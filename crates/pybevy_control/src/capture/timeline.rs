use std::collections::{HashMap, VecDeque};

use bevy::{
    camera::RenderTarget, ecs::world::World, prelude::*,
    render::view::window::screenshot::Screenshot, window::PrimaryWindow,
};
use image::RgbImage;
use tokio::sync::oneshot;

use super::{
    camera::setup_debug_camera,
    compose::composite_contact_sheet,
    headless::{headless_frame_sequence, read_headless_frame},
    hold::CaptureHold,
    isolation::EntityCaptureIsolationActive,
    lease::capture_in_progress,
    screenshot::{
        CAPTURE_CANCELLED_ERROR, CAPTURE_DEADLINE_ERROR, MAX_CAPTURE_WAIT_FRAMES,
        STALE_HEADLESS_FRAME_ERROR,
    },
};
use crate::{capture::queue::DebugCameraRequest, protocol::ControlError};

/// Resource tracking active timelines (multi-frame capture sequences).
#[derive(Resource, Default)]
pub struct PendingTimelines {
    pub active: HashMap<u64, ActiveTimeline>,
    pub next_id: u64,
}

/// A single active timeline capturing N frames over time.
pub struct ActiveTimeline {
    pub response_tx: Option<oneshot::Sender<Result<serde_json::Value, ControlError>>>,
    pub max_width: Option<u32>,
    pub columns: u32,
    /// Camera pose applied when the timeline starts.
    pub debug_camera: Option<DebugCameraRequest>,
    pub schedule: VecDeque<u32>,
    pub total_captures: u32,
    pub next_capture_index: u32,
    pub collected: Vec<(u32, RgbImage)>,
    pub hide_ui: bool,
    pub with_gizmos: bool,
    /// The capture state taken when the timeline starts, released when it ends.
    pub hold: Option<CaptureHold>,
    /// Last headless readback sequence observed by this timeline.
    pub headless_sequence: Option<u64>,
    /// Frames spent waiting on spawned captures after the schedule drained.
    pub stall_frames: u32,
}

/// Resource mapping screenshot Entity → (timeline_id, capture_index).
#[derive(Resource, Default)]
pub struct TimelineCaptures {
    pub map: HashMap<Entity, (u64, u32)>,
}

/// Compute capture schedule as frame deltas.
///
/// For count=6, total=60: targets [0,12,24,36,48,60], deltas [0,12,12,12,12,12]
/// Upper bound on timeline captures. Beyond this a contact sheet is unwieldy and
/// the capture cost balloons; below 1 the schedule is a single frame while
/// total_captures stays 0, so the completion check never matches and the request
/// hangs until it times out.
pub const MAX_TIMELINE_CAPTURES: u32 = 20;

pub fn compute_schedule(total_frames: u32, capture_count: u32) -> VecDeque<u32> {
    if capture_count <= 1 {
        return VecDeque::from([0]);
    }
    let interval = total_frames as f64 / (capture_count - 1) as f64;
    let mut schedule = VecDeque::new();
    let mut prev = 0u32;
    for i in 0..capture_count {
        let target = (i as f64 * interval).round() as u32;
        schedule.push_back(target - prev);
        prev = target;
    }
    schedule
}

/// Upper bound on `capture_timeline`'s `total_frames`.
pub const MAX_TIMELINE_FRAMES: u32 = 1200;

impl ActiveTimeline {
    pub(super) fn is_started(&self) -> bool {
        self.hold.is_some() && self.response_tx.is_some()
    }
}

/// Fail an active timeline, restoring the capture state it held.
fn fail_timeline(
    world: &mut World,
    timeline_id: u64,
    mut timeline: ActiveTimeline,
    message: String,
) {
    let mut stale_captures = Vec::new();
    if let Some(mut captures) = world.get_resource_mut::<TimelineCaptures>() {
        captures.map.retain(|&entity, (id, _)| {
            if *id == timeline_id {
                stale_captures.push(entity);
                false
            } else {
                true
            }
        });
    }
    for entity in stale_captures {
        world.despawn(entity);
    }
    if let Some(hold) = timeline.hold.take() {
        hold.release(world);
    }
    if let Some(tx) = timeline.response_tx.take() {
        let _ = tx.send(Err(ControlError::internal(message)));
    }
}

/// Take the capture state a timeline holds until it completes.
fn start_timeline(world: &mut World, timeline: &mut ActiveTimeline) -> Result<(), ControlError> {
    let debug_cleanup = timeline
        .debug_camera
        .take()
        .map(|request| setup_debug_camera(world, &request))
        .transpose()?;
    let mut hold = CaptureHold::begin(world, timeline.hide_ui, timeline.with_gizmos);
    hold.debug_cleanup = debug_cleanup;
    timeline.hold = Some(hold);
    timeline.headless_sequence = headless_frame_sequence(world);
    Ok(())
}

/// Process pending timelines: decrement schedule, spawn captures when ready.
pub fn process_pending_timelines(world: &mut World) {
    let Some(mut timelines) = world.remove_resource::<PendingTimelines>() else {
        return;
    };

    if timelines.active.is_empty() || world.contains_resource::<EntityCaptureIsolationActive>() {
        world.insert_resource(timelines);
        return;
    }

    let has_window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .is_some();
    let current_headless_sequence = (!has_window)
        .then(|| headless_frame_sequence(world))
        .flatten();
    let screenshot_target = if has_window {
        Some(Screenshot::primary_window())
    } else {
        world
            .query::<(&Camera, &RenderTarget)>()
            .iter(world)
            .find_map(|(camera, target)| {
                (camera.is_active && matches!(target, RenderTarget::Image(_)))
                    .then(|| Screenshot(target.clone()))
            })
    };

    let abandoned: Vec<u64> = timelines
        .active
        .iter()
        .filter(|(_, timeline)| {
            timeline
                .response_tx
                .as_ref()
                .is_some_and(oneshot::Sender::is_closed)
        })
        .map(|(&id, _)| id)
        .collect();
    for id in abandoned {
        if let Some(timeline) = timelines.active.remove(&id) {
            fail_timeline(world, id, timeline, CAPTURE_CANCELLED_ERROR.to_string());
        }
    }

    let next_to_start = (!timelines.active.values().any(ActiveTimeline::is_started)
        && !capture_in_progress(world))
    .then(|| {
        timelines
            .active
            .iter()
            .filter(|(_, timeline)| timeline.response_tx.is_some())
            .map(|(&id, _)| id)
            .min()
    })
    .flatten();
    if let Some(id) = next_to_start
        && let Some(mut timeline) = timelines.active.remove(&id)
    {
        match start_timeline(world, &mut timeline) {
            Ok(()) => {
                timelines.active.insert(id, timeline);
            }
            Err(error) => {
                if let Some(tx) = timeline.response_tx.take() {
                    let _ = tx.send(Err(error));
                }
            }
        }
    }

    // Collect timeline IDs that need a capture this frame
    let mut captures_to_spawn: Vec<(u64, u32)> = Vec::new();
    let mut completed_ids: Vec<u64> = Vec::new();
    let mut stalled_ids: Vec<u64> = Vec::new();
    let mut stale_headless_ids: Vec<u64> = Vec::new();

    for (&id, timeline) in timelines.active.iter_mut() {
        if timeline.response_tx.is_some() && timeline.hold.is_none() {
            continue;
        }
        if timeline.schedule.is_empty() {
            // All captures scheduled, waiting for collection
            if timeline.response_tx.is_none() {
                completed_ids.push(id);
            } else {
                timeline.stall_frames += 1;
                if timeline.stall_frames > MAX_CAPTURE_WAIT_FRAMES {
                    stalled_ids.push(id);
                }
            }
            continue;
        }

        // Decrement front of schedule
        if let Some(front) = timeline.schedule.front_mut() {
            if *front > 0 {
                *front -= 1;
            } else {
                if screenshot_target.is_none()
                    && timeline.headless_sequence.is_some_and(|baseline| {
                        current_headless_sequence.is_none_or(|current| current <= baseline)
                    })
                {
                    timeline.stall_frames += 1;
                    if timeline.stall_frames > MAX_CAPTURE_WAIT_FRAMES {
                        stale_headless_ids.push(id);
                    }
                    continue;
                }

                // Time to capture
                let capture_index = timeline.next_capture_index;
                timeline.next_capture_index += 1;
                timeline.schedule.pop_front();
                timeline.headless_sequence = current_headless_sequence;
                timeline.stall_frames = 0;
                captures_to_spawn.push((id, capture_index));
            }
        }
    }

    // Remove fully completed timelines (response already sent)
    for id in completed_ids {
        timelines.active.remove(&id);
    }

    for id in stalled_ids {
        if let Some(timeline) = timelines.active.remove(&id) {
            fail_timeline(world, id, timeline, CAPTURE_DEADLINE_ERROR.to_string());
        }
    }

    for id in stale_headless_ids {
        if let Some(timeline) = timelines.active.remove(&id) {
            fail_timeline(world, id, timeline, STALE_HEADLESS_FRAME_ERROR.to_string());
        }
    }

    world.insert_resource(timelines);

    if let Some(screenshot) = screenshot_target {
        // Capture the scheduled render, not an earlier frame still in readback.
        for (timeline_id, capture_index) in captures_to_spawn {
            let entity = world.spawn(Screenshot(screenshot.0.clone())).id();

            let mut timeline_captures =
                world.get_resource_or_insert_with(TimelineCaptures::default);
            timeline_captures
                .map
                .insert(entity, (timeline_id, capture_index));
        }
    } else {
        // Headless fallback: read from HeadlessFrameBuffer
        match read_headless_frame(world) {
            Err(error) => {
                // The capture slots were already consumed from the schedule,
                // so these timelines cannot complete: fail them now.
                for (timeline_id, _) in captures_to_spawn {
                    let timeline = world
                        .resource_mut::<PendingTimelines>()
                        .active
                        .remove(&timeline_id);
                    if let Some(timeline) = timeline {
                        fail_timeline(world, timeline_id, timeline, error.message.clone());
                    }
                }
            }
            Ok(rgb) => {
                let mut holds = Vec::new();
                {
                    let mut timelines = world.resource_mut::<PendingTimelines>();
                    for (timeline_id, capture_index) in captures_to_spawn {
                        if let Some(timeline) = timelines.active.get_mut(&timeline_id) {
                            timeline.collected.push((capture_index, rgb.clone()));
                            if timeline.collected.len() as u32 == timeline.total_captures {
                                let result = composite_contact_sheet(timeline);
                                if let Some(tx) = timeline.response_tx.take() {
                                    let _ = tx.send(result);
                                }
                                holds.extend(timeline.hold.take());
                            }
                        }
                    }
                }
                for hold in holds {
                    hold.release(world);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{any::TypeId, collections::VecDeque};

    use base64::Engine;
    use bevy::{
        asset::RenderAssetUsages,
        camera::RenderTarget,
        gizmos::config::DefaultGizmoConfigGroup,
        image::Image,
        render::{
            render_resource::{Extent3d, TextureDimension, TextureFormat},
            view::window::screenshot::{Screenshot, ScreenshotCaptured},
        },
        ui::Node,
        window::PrimaryWindow,
    };
    use image::Rgb;
    use tokio::sync::oneshot;

    use super::*;
    use crate::{
        capture::{
            analysis::CapturedFrames,
            headless::HeadlessFrameBuffer,
            hold::{GizmoEnabledRestore, InternalOverlayUi, OverlaySuppression},
            queue::CaptureResponseKind,
            screenshot::{
                PendingScreenshotResponders, ScreenshotResponder, screenshot_captured_observer,
            },
        },
        handlers::turnaround::{PendingTurnarounds, TurnaroundCaptures},
    };

    fn world_with_gizmos() -> World {
        let mut world = World::new();
        let mut store = GizmoConfigStore::default();
        store.insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
        world.insert_resource(store);
        world
    }

    fn gizmos_enabled(world: &World) -> bool {
        let store = world.resource::<GizmoConfigStore>();
        let (config, _) = store.config::<DefaultGizmoConfigGroup>();
        config.enabled
    }

    fn default_group_restore(was_enabled: bool) -> GizmoEnabledRestore {
        vec![(TypeId::of::<DefaultGizmoConfigGroup>(), was_enabled)]
    }

    fn queued_timeline(
        hide_ui: bool,
    ) -> (
        ActiveTimeline,
        oneshot::Receiver<Result<serde_json::Value, ControlError>>,
    ) {
        let (tx, rx) = oneshot::channel();
        let timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: None,
            columns: 2,
            debug_camera: None,
            hold: None,
            schedule: VecDeque::from([3, 5]),
            total_captures: 2,
            next_capture_index: 0,
            collected: vec![],
            hide_ui,
            with_gizmos: false,
            headless_sequence: None,
            stall_frames: 0,
        };
        (timeline, rx)
    }

    fn make_test_image(width: u32, height: u32) -> Image {
        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let data = vec![255u8; (width * height * 4) as usize];
        Image::new(
            size,
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    #[test]
    fn compute_schedule_even_distribution() {
        let schedule = compute_schedule(60, 4);
        // 4 captures across 60 frames: targets [0, 20, 40, 60], deltas [0, 20, 20, 20]
        assert_eq!(schedule.len(), 4);
        assert_eq!(schedule[0], 0);
        assert_eq!(schedule[1], 20);
        assert_eq!(schedule[2], 20);
        assert_eq!(schedule[3], 20);
    }

    #[test]
    fn compute_schedule_single_capture() {
        let schedule = compute_schedule(60, 1);
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0], 0);
    }

    #[test]
    fn compute_schedule_two_captures() {
        let schedule = compute_schedule(60, 2);
        assert_eq!(schedule.len(), 2);
        assert_eq!(schedule[0], 0);
        assert_eq!(schedule[1], 60);
    }

    #[test]
    fn compute_schedule_six_captures() {
        let schedule = compute_schedule(60, 6);
        assert_eq!(schedule.len(), 6);
        // First capture at frame 0
        assert_eq!(schedule[0], 0);
        // Sum of all deltas should equal total_frames
        let total: u32 = schedule.iter().sum();
        assert_eq!(total, 60);
    }

    #[test]
    fn compute_schedule_zero_total_frames() {
        let schedule = compute_schedule(0, 3);
        assert_eq!(schedule.len(), 3);
        // All deltas should be 0
        for delta in &schedule {
            assert_eq!(*delta, 0);
        }
    }

    /// The overlay is hidden on the first frame the timeline is seen, before
    /// any capture spawns.
    #[test]
    fn timeline_suppresses_the_overlay_before_the_first_capture() {
        let mut world = world_with_gizmos();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 255, 255, 255], 1, 1)),
            sequence: 1,
        });
        let overlay = world.spawn((Visibility::Visible, InternalOverlayUi)).id();

        let (tx, _rx) = oneshot::channel();
        let mut pending = PendingTimelines::default();
        pending.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 2,
                hold: None,
                // Nothing captures on this frame: the front of the schedule
                // only counts down.
                schedule: VecDeque::from([3, 5]),
                total_captures: 2,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: true,
                with_gizmos: false,
                headless_sequence: None,
                stall_frames: 0,
            },
        );
        world.insert_resource(pending);

        process_pending_timelines(&mut world);

        assert_eq!(
            *world.get::<Visibility>(overlay).unwrap(),
            Visibility::Hidden,
            "the overlay must already be hidden on the frame before the first capture"
        );
        assert!(
            world
                .resource::<PendingTimelines>()
                .active
                .get(&0)
                .is_some_and(|timeline| timeline.hold.is_some()),
            "suppression must be recorded so it is released once"
        );
    }

    /// Timeline captures hold visibility suppression for the contact sheet.
    #[test]
    fn timeline_capture_applies_capture_visibility() {
        let mut world = world_with_gizmos();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 255, 255, 255], 1, 1)),
            sequence: 1,
        });
        let overlay = world.spawn((Visibility::Visible, InternalOverlayUi)).id();
        let authored_ui = world.spawn((Node::default(), Visibility::Visible)).id();

        let (tx, _rx) = oneshot::channel();
        let mut pending = PendingTimelines::default();
        pending.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 2,
                hold: None,
                // capture on this frame, then later
                schedule: VecDeque::from([0, 5]),
                total_captures: 2,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: true,
                with_gizmos: false,
                headless_sequence: None,
                stall_frames: 0,
            },
        );
        world.insert_resource(pending);

        process_pending_timelines(&mut world);

        assert_eq!(
            *world.get::<Visibility>(overlay).unwrap(),
            Visibility::Hidden,
            "overlay must be hidden while the timeline captures"
        );
        assert_eq!(
            *world.get::<Visibility>(authored_ui).unwrap(),
            Visibility::Hidden,
            "authored UI must follow the timeline hide_ui option"
        );
        assert!(!gizmos_enabled(&world));
        assert_eq!(
            world.resource::<OverlaySuppression>().0,
            1,
            "timeline must hold one suppression refcount"
        );
        assert!(
            world.resource::<PendingTimelines>().active[&0]
                .hold
                .is_some(),
            "timeline must record its hold so completion releases exactly once"
        );
        assert!(
            world.resource::<PendingTimelines>().active[&0]
                .hold
                .as_ref()
                .is_some_and(|hold| hold.ui_restore.is_some())
        );
        assert_eq!(
            world.resource::<PendingTimelines>().active[&0]
                .hold
                .as_ref()
                .unwrap()
                .gizmo_restore,
            default_group_restore(true)
        );

        // A second tick of the same timeline must not double-count
        process_pending_timelines(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
    }

    #[test]
    fn queued_timeline_starts_after_the_active_capture_ends() {
        let mut world = world_with_gizmos();
        let ui = world.spawn((Visibility::Visible, Node::default())).id();
        let screenshot_entity = world.spawn_empty().id();
        let (tx, _responder_rx) = oneshot::channel();
        let mut hold = CaptureHold::default();
        hold.begin_capture(&mut world);
        let mut responders = PendingScreenshotResponders::default();
        responders.map.insert(
            screenshot_entity,
            ScreenshotResponder {
                response_tx: tx,
                max_width: None,
                hold,
                extra_response: None,
                capture_viewport: None,
                response_kind: CaptureResponseKind::Screenshot,
                frames_waited: 0,
            },
        );
        world.insert_resource(responders);
        let (timeline, _rx) = queued_timeline(true);
        let mut pending = PendingTimelines::default();
        pending.active.insert(0, timeline);
        world.insert_resource(pending);

        process_pending_timelines(&mut world);
        let timeline = &world.resource::<PendingTimelines>().active[&0];
        assert!(timeline.hold.is_none());
        assert_eq!(timeline.schedule, VecDeque::from([3, 5]));
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);

        let responder = world
            .resource_mut::<PendingScreenshotResponders>()
            .map
            .remove(&screenshot_entity)
            .unwrap();
        responder.hold.release(&mut world);
        process_pending_timelines(&mut world);
        let timeline = &world.resource::<PendingTimelines>().active[&0];
        assert!(timeline.hold.is_some());
        assert_eq!(timeline.schedule, VecDeque::from([2, 5]));
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Hidden);
    }

    #[test]
    fn abandoned_timeline_releases_capture_state() {
        let mut world = world_with_gizmos();
        let ui = world.spawn((Visibility::Visible, Node::default())).id();
        let (timeline, rx) = queued_timeline(true);
        let mut pending = PendingTimelines::default();
        pending.active.insert(0, timeline);
        world.insert_resource(pending);
        process_pending_timelines(&mut world);
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Hidden);

        drop(rx);
        process_pending_timelines(&mut world);

        assert!(world.resource::<PendingTimelines>().active.is_empty());
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert!(gizmos_enabled(&world));
    }

    #[test]
    fn headless_timeline_without_frame_buffer_fails_instead_of_hanging() {
        let mut world = world_with_gizmos();
        let (tx, mut rx) = oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 2,
                hold: None,
                schedule: VecDeque::from([0]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: false,
                with_gizmos: true,
                headless_sequence: None,
                stall_frames: 0,
            },
        );
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("HeadlessFrameBuffer"));
        assert!(world.resource::<PendingTimelines>().active.is_empty());
    }

    #[test]
    fn process_pending_timelines_no_resource() {
        let mut world = World::new();
        // Should not panic when PendingTimelines resource is absent
        process_pending_timelines(&mut world);
    }

    #[test]
    fn process_pending_timelines_empty() {
        let mut world = World::new();
        world.insert_resource(PendingTimelines::default());

        process_pending_timelines(&mut world);

        let timelines = world.resource::<PendingTimelines>();
        assert!(timelines.active.is_empty());
    }

    #[test]
    fn process_pending_timelines_countdown() {
        let mut world = World::new();
        let (tx, _rx) = oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                hold: None,
                schedule: VecDeque::from([2]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: true,
                with_gizmos: false,
                headless_sequence: None,
                stall_frames: 0,
            },
        );
        timelines.next_id = 1;
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        let timelines = world.resource::<PendingTimelines>();
        let timeline = timelines.active.get(&0).unwrap();
        // Front should be decremented from 2 to 1
        assert_eq!(*timeline.schedule.front().unwrap(), 1);
        // No screenshot should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn process_pending_timelines_captures_at_zero_delay() {
        let mut world = World::new();
        world.spawn(PrimaryWindow);
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                hold: None,
                schedule: VecDeque::from([0]),
                // ready immediately
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: true,
                with_gizmos: false,
                headless_sequence: None,
                stall_frames: 0,
            },
        );
        timelines.next_id = 1;
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        // A Screenshot entity should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 1);

        // next_capture_index should have advanced
        let tl = world.resource::<PendingTimelines>();
        let timeline = tl.active.get(&0).unwrap();
        assert_eq!(timeline.next_capture_index, 1);
        assert!(timeline.schedule.is_empty());
    }

    #[test]
    fn process_pending_timelines_multiple_delays() {
        let mut world = World::new();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 255, 255, 255], 1, 1)),
            sequence: 1,
        });
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                hold: None,
                // first ready, then 2 more
                schedule: VecDeque::from([0, 10, 10]),
                total_captures: 3,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: true,
                with_gizmos: false,
                headless_sequence: None,
                stall_frames: 0,
            },
        );
        timelines.next_id = 1;
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);
        assert_eq!(
            world.resource::<PendingTimelines>().active[&0].next_capture_index,
            0,
            "the first capture waits for a frame rendered after the timeline started"
        );
        world.resource_mut::<HeadlessFrameBuffer>().sequence = 2;
        process_pending_timelines(&mut world);

        // First capture should be spawned
        let tl = world.resource::<PendingTimelines>();
        let timeline = tl.active.get(&0).unwrap();
        assert_eq!(timeline.next_capture_index, 1);
        // schedule should now be [10, 10] (first 0 popped)
        assert_eq!(timeline.schedule.len(), 2);
        assert_eq!(timeline.schedule[0], 10);
    }

    #[test]
    fn headless_timeline_captures_active_image_and_does_not_resuppress_after_completion() {
        let mut world = world_with_gizmos();
        world.init_resource::<PendingScreenshotResponders>();
        world.init_resource::<TimelineCaptures>();
        world.init_resource::<PendingTurnarounds>();
        world.init_resource::<TurnaroundCaptures>();
        world.init_resource::<CapturedFrames>();
        world.add_observer(screenshot_captured_observer);
        let mut images = Assets::<Image>::default();
        let inactive_image = images.add(make_test_image(1, 1));
        let active_image = images.add(make_test_image(1, 1));
        world.spawn((
            Camera {
                is_active: false,
                ..default()
            },
            RenderTarget::Image(inactive_image.into()),
        ));
        world.spawn((
            Camera::default(),
            RenderTarget::Image(active_image.clone().into()),
        ));
        let ui = world.spawn((Node::default(), Visibility::Visible)).id();
        let overlay = world.spawn((Visibility::Visible, InternalOverlayUi)).id();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255], 1, 1)),
            sequence: 6,
        });
        let (tx, mut rx) = oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 1,
                hold: None,
                schedule: VecDeque::from([0]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: true,
                with_gizmos: false,
                headless_sequence: Some(5),
                stall_frames: 0,
            },
        );
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        assert!(rx.try_recv().is_err());
        assert!(
            world.resource::<PendingTimelines>().active[&0]
                .collected
                .is_empty()
        );
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Hidden);
        assert_eq!(
            *world.get::<Visibility>(overlay).unwrap(),
            Visibility::Hidden
        );
        assert!(!gizmos_enabled(&world));
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
        let mut screenshots = world.query::<(Entity, &Screenshot)>();
        let (entity, screenshot) = screenshots.single(&world).unwrap();
        let RenderTarget::Image(target) = &screenshot.0 else {
            panic!("headless timeline must capture its active image target");
        };
        assert_eq!(target.handle, active_image);
        assert_eq!(world.resource::<TimelineCaptures>().map[&entity], (0, 0));

        world.trigger(ScreenshotCaptured {
            entity,
            image: make_test_image(1, 1),
        });
        world.flush();

        let result = rx.try_recv().unwrap().unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(result["image"].as_str().unwrap())
            .unwrap();
        let captured = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(*captured.get_pixel(0, 0), Rgb([255, 255, 255]));
        assert!(world.resource::<TimelineCaptures>().map.is_empty());
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert!(gizmos_enabled(&world));

        process_pending_timelines(&mut world);

        assert!(world.resource::<PendingTimelines>().active.is_empty());
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert!(gizmos_enabled(&world));
    }

    #[test]
    fn headless_timeline_waits_for_a_new_readback() {
        let mut world = World::new();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255], 1, 1)),
            sequence: 5,
        });
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            0,
            ActiveTimeline {
                debug_camera: None,
                response_tx: Some(tx),
                max_width: None,
                columns: 1,
                hold: None,
                schedule: VecDeque::from([0]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                hide_ui: false,
                with_gizmos: true,
                headless_sequence: Some(5),
                stall_frames: 0,
            },
        );
        world.insert_resource(timelines);

        process_pending_timelines(&mut world);

        let timeline = &world.resource::<PendingTimelines>().active[&0];
        assert_eq!(timeline.schedule, VecDeque::from([0]));
        assert_eq!(timeline.next_capture_index, 0);
        assert!(rx.try_recv().is_err());

        {
            let mut frame = world.resource_mut::<HeadlessFrameBuffer>();
            frame.latest = Some((vec![0, 255, 0, 255], 1, 1));
            frame.sequence = 6;
        }

        process_pending_timelines(&mut world);

        let result = rx.try_recv().unwrap().unwrap();
        assert_eq!(result["width"], 1);
        assert_eq!(result["height"], 5);
    }

    #[test]
    fn compute_schedule_large_frame_count() {
        let schedule = compute_schedule(1000, 11);
        assert_eq!(schedule.len(), 11);
        let total: u32 = schedule.iter().sum();
        assert_eq!(total, 1000);
    }

    #[test]
    fn compute_schedule_zero_captures_treated_as_one() {
        // capture_count of 0 would cause issues, but the function
        // handles capture_count <= 1 by returning [0]
        let schedule = compute_schedule(100, 0);
        assert_eq!(schedule.len(), 1);
        assert_eq!(schedule[0], 0);
    }
}
