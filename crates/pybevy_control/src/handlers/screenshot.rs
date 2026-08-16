use std::{
    any::TypeId,
    collections::{HashMap, HashSet, VecDeque},
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use base64::Engine;
use bevy::{
    camera::visibility::{RenderLayers, VisibilityClass},
    ecs::world::World,
    gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore},
    light::cluster::ClusterVisibilityClass,
    prelude::*,
    render::view::window::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};
use image::{ImageFormat, Rgb, RgbImage};
use pybevy_ecs::shared::system_runtime::HotReloadGeneration;
use tokio::sync::oneshot;

use crate::{
    bridge::{
        CaptureResponseKind, ControlError, DebugCameraRequest, EntityRef, InternalOverlayUi,
        OverlaySuppression, PendingScreenshot, PendingScreenshots,
    },
    handlers::{
        entity::resolve_entity,
        frame_analysis::{
            CapturedFrameMetadata, CapturedFrames, analyze_frame, resize_rgb_image_linear,
        },
    },
};

const ENTITY_CAPTURE_LAYER: usize = 63;

#[derive(Resource)]
pub struct EntityCaptureIsolationActive;

pub struct EntityCaptureIsolation {
    original_layers: Vec<(Entity, Option<RenderLayers>)>,
    gizmo_configs: Vec<(TypeId, bool, RenderLayers)>,
    scope: HashSet<Entity>,
}

impl EntityCaptureIsolation {
    fn restore_world(self, world: &mut World) {
        for (entity, original) in self.original_layers {
            let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
                continue;
            };
            if let Some(layers) = original {
                entity_mut.insert(layers);
            } else {
                entity_mut.remove::<RenderLayers>();
            }
        }
        if let Some(mut store) = world.get_resource_mut::<GizmoConfigStore>() {
            for (group, enabled, layers) in self.gizmo_configs {
                if let Some((config, _)) = store.get_config_mut_dyn(&group) {
                    config.enabled = enabled;
                    config.render_layers = layers;
                }
            }
        }
        world.remove_resource::<EntityCaptureIsolationActive>();
    }
}

fn collect_entity_capture_scope(world: &World, root: Entity) -> HashSet<Entity> {
    let mut scope = HashSet::from([root]);
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        let Some(children) = world.get::<Children>(entity) else {
            continue;
        };
        for child in children.iter() {
            if scope.insert(child) {
                pending.push(child);
            }
        }
    }
    scope
}

fn begin_entity_capture_isolation(
    world: &mut World,
    entity_ref: &EntityRef,
    with_gizmos: bool,
) -> Result<EntityCaptureIsolation, ControlError> {
    let root = resolve_entity(world, entity_ref)?;
    let scope = collect_entity_capture_scope(world, root);
    let support_class = TypeId::of::<ClusterVisibilityClass>();
    let snapshots = {
        let mut query = world.query::<(
            Entity,
            Option<&RenderLayers>,
            Has<Camera>,
            Option<&VisibilityClass>,
        )>();
        query
            .iter(world)
            .map(|(entity, layers, is_camera, visibility_class)| {
                let is_support =
                    visibility_class.is_some_and(|classes| classes.contains(&support_class));
                (entity, layers.cloned(), is_camera, is_support)
            })
            .collect::<Vec<_>>()
    };

    let mut original_layers = Vec::new();
    for (entity, original, is_camera, is_support) in snapshots {
        let included = scope.contains(&entity) || is_camera || is_support;
        let replacement = if included {
            Some(RenderLayers::layer(ENTITY_CAPTURE_LAYER))
        } else {
            original
                .as_ref()
                .filter(|layers| layers.iter().any(|layer| layer == ENTITY_CAPTURE_LAYER))
                .cloned()
                .map(|layers| layers.without(ENTITY_CAPTURE_LAYER))
        };
        let Some(replacement) = replacement else {
            continue;
        };
        if original.as_ref() == Some(&replacement) {
            continue;
        }
        world.entity_mut(entity).insert(replacement);
        original_layers.push((entity, original));
    }
    let mut gizmo_configs = Vec::new();
    if with_gizmos && let Some(mut store) = world.get_resource_mut::<GizmoConfigStore>() {
        for (group, config, _) in store.iter_mut() {
            gizmo_configs.push((*group, config.enabled, config.render_layers.clone()));
            config.enabled = true;
            config.render_layers = RenderLayers::layer(ENTITY_CAPTURE_LAYER);
        }
    }
    world.insert_resource(EntityCaptureIsolationActive);
    Ok(EntityCaptureIsolation {
        original_layers,
        gizmo_configs,
        scope,
    })
}

/// Resource storing the latest GPU readback frame for headless screenshots.
///
/// Updated each frame by a system registered by `ImageCopyPlugin`.
/// Read by screenshot/timeline/turnaround handlers when no primary window exists.
#[derive(Resource, Default)]
pub struct HeadlessFrameBuffer {
    pub latest: Option<(Vec<u8>, u32, u32)>,
    pub sequence: u64,
}

/// Cross-world render completion state for captures queued after live edits.
#[derive(Resource, Clone, Default)]
pub struct RenderFrameReadiness {
    shared: Arc<RenderFrameReadinessState>,
}

#[derive(Default)]
struct RenderFrameReadinessState {
    requested_epoch: AtomicU64,
    completed_epoch: AtomicU64,
}

impl RenderFrameReadiness {
    pub fn request_frame(&self) -> u64 {
        self.shared.requested_epoch.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn requested_epoch(&self) -> u64 {
        self.shared.requested_epoch.load(Ordering::Acquire)
    }

    fn complete_requested_frame(&self) {
        let requested = self.requested_epoch();
        self.shared
            .completed_epoch
            .store(requested, Ordering::Release);
    }

    fn is_complete(&self, epoch: u64) -> bool {
        self.shared.completed_epoch.load(Ordering::Acquire) >= epoch
    }
}

/// Publish completion only after the render pipeline queue is empty at the
/// end of a render-world frame.
pub fn update_render_frame_readiness(
    pipeline_cache: Res<bevy::render::render_resource::PipelineCache>,
    readiness: Res<RenderFrameReadiness>,
) {
    if pipeline_cache.waiting_pipelines().next().is_none() {
        readiness.complete_requested_frame();
    }
}

/// Cleanup info for a debug camera set up for a screenshot.
pub struct DebugCameraCleanup {
    pub debug_entity: Entity,
    /// If we reused an existing camera: (original_transform, original_global_transform, original_is_active).
    /// If None, this was a freshly spawned camera that should be despawned on cleanup.
    pub reused_state: Option<(Transform, GlobalTransform, bool)>,
    /// Other cameras and their original `is_active` state before we disabled them.
    pub original_cameras: Vec<(Entity, bool)>,
}

/// Frames a queued capture may wait for its readback before it is failed and
/// its visibility/camera state restored, so a capture that never completes
/// cannot wedge `another_capture_is_active` or hang its request forever.
pub(crate) const MAX_CAPTURE_WAIT_FRAMES: u32 = 600;

pub(crate) const CAPTURE_DEADLINE_ERROR: &str =
    "capture did not complete within the deadline (render readback never arrived)";
pub(crate) const STALE_HEADLESS_FRAME_ERROR: &str = "capture did not receive a fresh headless frame before the deadline; the renderer may have stopped";

/// Per-screenshot responder stored until the observer fires.
pub struct ScreenshotResponder {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub max_width: Option<u32>,
    pub debug_cleanup: Option<DebugCameraCleanup>,
    pub ui_restore: Option<Vec<(Entity, Visibility)>>,
    pub entity_isolation: Option<EntityCaptureIsolation>,
    /// If gizmos were toggled for this screenshot, the original enabled state to restore.
    pub gizmo_restore: Option<bool>,
    /// Extra JSON fields to merge into the screenshot response.
    pub extra_response: Option<serde_json::Value>,
    pub response_kind: CaptureResponseKind,
    pub frames_waited: u32,
}

/// Resource mapping screenshot Entity → responder info.
#[derive(Resource, Default)]
pub struct PendingScreenshotResponders {
    pub map: HashMap<Entity, ScreenshotResponder>,
}

/// Staged debug screenshots waiting for the debug camera to render before capture.
#[derive(Resource, Default)]
struct StagedScreenshots {
    pending: Vec<StagedScreenshot>,
}

struct StagedScreenshot {
    response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    frames_remaining: u32,
    with_gizmos: bool,
    gizmo_restore: Option<bool>,
    max_width: Option<u32>,
    debug_cleanup: Option<DebugCameraCleanup>,
    ui_restore: Option<Vec<(Entity, Visibility)>>,
    entity_isolation: Option<EntityCaptureIsolation>,
    extra_response: Option<serde_json::Value>,
    response_kind: CaptureResponseKind,
    baseline_headless_sequence: Option<u64>,
    frames_waited: u32,
}

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
    pub debug_cleanup: Option<DebugCameraCleanup>,
    pub schedule: VecDeque<u32>,
    pub total_captures: u32,
    pub next_capture_index: u32,
    pub collected: Vec<(u32, RgbImage)>,
    /// Whether this timeline holds an `OverlaySuppression` refcount
    /// (taken at first capture, released on completion).
    pub overlay_suppressed: bool,
    pub hide_ui: bool,
    pub with_gizmos: bool,
    pub ui_restore: Option<Vec<(Entity, Visibility)>>,
    pub gizmo_restore: Option<bool>,
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

fn another_capture_is_active(world: &World) -> bool {
    world
        .get_resource::<PendingScreenshotResponders>()
        .is_some_and(|responders| !responders.map.is_empty())
        || world
            .get_resource::<StagedScreenshots>()
            .is_some_and(|staged| !staged.pending.is_empty())
        || world
            .get_resource::<PendingTimelines>()
            .is_some_and(|timelines| !timelines.active.is_empty())
        || world
            .get_resource::<super::turnaround::PendingTurnarounds>()
            .is_some_and(|turnarounds| !turnarounds.active.is_empty())
}

/// Process pending screenshot requests (called each frame in Last schedule).
///
/// Flow:
/// 1. Count down `frames_remaining` on normal pending screenshots
/// 2. When ready: if `debug_camera` is set, set up the debug camera and stage with extra delay
/// 3. Count down staged debug screenshots
/// 4. When staged screenshot is ready, spawn the Screenshot entity for capture
pub fn process_pending_screenshots(world: &mut World) {
    expire_stuck_screenshot_responders(world);

    let Some(mut pending) = world.remove_resource::<PendingScreenshots>() else {
        return;
    };

    if pending.pending.is_empty()
        && world
            .get_resource::<StagedScreenshots>()
            .is_none_or(|s| s.pending.is_empty())
    {
        world.insert_resource(pending);
        return;
    }

    let mut remaining = Vec::new();
    let mut ready = Vec::new();

    for mut screenshot in pending.pending.drain(..) {
        prepare_pending_screenshot_gizmos(world, &mut screenshot);
        if screenshot.required_render_epoch.is_some_and(|epoch| {
            world
                .get_resource::<RenderFrameReadiness>()
                .is_some_and(|readiness| !readiness.is_complete(epoch))
        }) {
            remaining.push(screenshot);
            continue;
        }
        if screenshot.frames_remaining > 0 {
            screenshot.frames_remaining -= 1;
            remaining.push(screenshot);
        } else {
            ready.push(screenshot);
        }
    }

    pending.pending = remaining;
    world.insert_resource(pending);

    // Process ready screenshots
    for mut screenshot in ready {
        if world.contains_resource::<EntityCaptureIsolationActive>()
            || (screenshot.entity.is_some() && another_capture_is_active(world))
        {
            world
                .resource_mut::<PendingScreenshots>()
                .pending
                .push(screenshot);
            continue;
        }

        let debug_cleanup = if let Some(debug_req) = screenshot.debug_camera.take() {
            match setup_debug_camera(world, &debug_req) {
                Ok(cleanup) => cleanup,
                Err(error) => {
                    let _ = screenshot.response_tx.send(Err(error));
                    if let Some(was_enabled) = screenshot.gizmo_restore {
                        set_gizmos_enabled(world, was_enabled);
                    }
                    continue;
                }
            }
            .into()
        } else {
            None
        };

        let entity_isolation = if let Some(entity_ref) = screenshot.entity.as_ref() {
            match begin_entity_capture_isolation(world, entity_ref, screenshot.with_gizmos) {
                Ok(isolation) => Some(isolation),
                Err(error) => {
                    let _ = screenshot.response_tx.send(Err(error));
                    if let Some(cleanup) = debug_cleanup {
                        cleanup_debug_camera_world(cleanup, world);
                    }
                    if let Some(was_enabled) = screenshot.gizmo_restore {
                        set_gizmos_enabled(world, was_enabled);
                    }
                    continue;
                }
            }
        } else {
            None
        };

        suppress_internal_overlay(world);
        let ui_restore = screenshot.hide_ui.then(|| {
            hide_ui_nodes(
                world,
                entity_isolation.as_ref().map(|isolation| &isolation.scope),
            )
        });

        if debug_cleanup.is_some() {
            let baseline_headless_sequence = headless_frame_sequence(world);
            let mut staged = world.get_resource_or_insert_with(StagedScreenshots::default);
            staged.pending.push(StagedScreenshot {
                response_tx: screenshot.response_tx,
                frames_remaining: if entity_isolation.is_some() { 4 } else { 2 },
                with_gizmos: screenshot.with_gizmos,
                gizmo_restore: screenshot.gizmo_restore,
                max_width: screenshot.max_width,
                debug_cleanup,
                ui_restore,
                entity_isolation,
                extra_response: screenshot.extra_response,
                response_kind: screenshot.response_kind,
                baseline_headless_sequence,
                frames_waited: 0,
            });
        } else {
            // Normal path: spawn Screenshot entity immediately
            let has_window = world
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .iter(world)
                .next()
                .is_some();

            if has_window {
                let gizmo_restore = screenshot.gizmo_restore.take().or_else(|| {
                    (!screenshot.with_gizmos)
                        .then(|| set_gizmos_enabled(world, false))
                        .flatten()
                });

                let entity = world.spawn(Screenshot::primary_window()).id();

                let mut responders =
                    world.get_resource_or_insert_with(PendingScreenshotResponders::default);
                responders.map.insert(
                    entity,
                    ScreenshotResponder {
                        response_tx: screenshot.response_tx,
                        max_width: screenshot.max_width,
                        debug_cleanup: None,
                        ui_restore,
                        entity_isolation,
                        gizmo_restore,
                        extra_response: screenshot.extra_response,
                        response_kind: screenshot.response_kind,
                        frames_waited: 0,
                    },
                );
            } else {
                let baseline_headless_sequence = headless_frame_sequence(world);
                let mut staged = world.get_resource_or_insert_with(StagedScreenshots::default);
                staged.pending.push(StagedScreenshot {
                    response_tx: screenshot.response_tx,
                    frames_remaining: if entity_isolation.is_some() { 4 } else { 2 },
                    with_gizmos: screenshot.with_gizmos,
                    gizmo_restore: screenshot.gizmo_restore,
                    max_width: screenshot.max_width,
                    debug_cleanup: None,
                    ui_restore,
                    entity_isolation,
                    extra_response: screenshot.extra_response,
                    response_kind: screenshot.response_kind,
                    baseline_headless_sequence,
                    frames_waited: 0,
                });
            }
        }
    }

    // Process staged debug screenshots
    if let Some(mut staged) = world.remove_resource::<StagedScreenshots>() {
        let mut still_waiting = Vec::new();

        for mut s in staged.pending.drain(..) {
            if s.frames_remaining > 0 {
                s.frames_remaining -= 1;
                still_waiting.push(s);
            } else {
                // Debug camera has rendered — capture now
                let has_window = world
                    .query_filtered::<Entity, With<PrimaryWindow>>()
                    .iter(world)
                    .next()
                    .is_some();

                if !has_window
                    && s.baseline_headless_sequence.is_some_and(|baseline| {
                        headless_frame_sequence(world).is_none_or(|current| current <= baseline)
                    })
                {
                    s.frames_waited += 1;
                    if s.frames_waited > MAX_CAPTURE_WAIT_FRAMES {
                        fail_staged_screenshot(world, s, STALE_HEADLESS_FRAME_ERROR);
                    } else {
                        still_waiting.push(s);
                    }
                    continue;
                }

                if has_window {
                    let gizmo_restore = s.gizmo_restore.take().or_else(|| {
                        (!s.with_gizmos)
                            .then(|| set_gizmos_enabled(world, false))
                            .flatten()
                    });

                    let entity = world.spawn(Screenshot::primary_window()).id();

                    let mut responders =
                        world.get_resource_or_insert_with(PendingScreenshotResponders::default);
                    responders.map.insert(
                        entity,
                        ScreenshotResponder {
                            response_tx: s.response_tx,
                            max_width: s.max_width,
                            debug_cleanup: s.debug_cleanup,
                            ui_restore: s.ui_restore,
                            entity_isolation: s.entity_isolation,
                            gizmo_restore,
                            extra_response: s.extra_response,
                            response_kind: s.response_kind,
                            frames_waited: 0,
                        },
                    );
                } else {
                    // Headless fallback
                    let result = capture_headless_frame(world, s.max_width, s.response_kind);
                    let result = merge_extra_response(result, s.extra_response);
                    let _ = s.response_tx.send(result);
                    if let Some(cleanup) = s.debug_cleanup {
                        cleanup_debug_camera_world(cleanup, world);
                    }
                    release_internal_overlay(world);
                    if let Some(restore) = s.ui_restore {
                        restore_ui_nodes(world, restore);
                    }
                    if let Some(isolation) = s.entity_isolation {
                        isolation.restore_world(world);
                    }
                    if let Some(was_enabled) = s.gizmo_restore {
                        set_gizmos_enabled(world, was_enabled);
                    }
                }
            }
        }

        staged.pending = still_waiting;
        world.insert_resource(staged);
    }
}

pub(crate) fn headless_frame_sequence(world: &World) -> Option<u64> {
    world
        .get_resource::<HeadlessFrameBuffer>()
        .and_then(|buffer| buffer.latest.as_ref().map(|_| buffer.sequence))
}

fn fail_staged_screenshot(world: &mut World, staged: StagedScreenshot, message: &str) {
    let _ = staged
        .response_tx
        .send(Err(ControlError::internal(message.to_string())));
    if let Some(cleanup) = staged.debug_cleanup {
        cleanup_debug_camera_world(cleanup, world);
    }
    release_internal_overlay(world);
    if let Some(restore) = staged.ui_restore {
        restore_ui_nodes(world, restore);
    }
    if let Some(isolation) = staged.entity_isolation {
        isolation.restore_world(world);
    }
    if let Some(was_enabled) = staged.gizmo_restore {
        set_gizmos_enabled(world, was_enabled);
    }
}

/// Fail responders whose capture readback never arrived, restoring the
/// visibility and camera state they held.
fn expire_stuck_screenshot_responders(world: &mut World) {
    let expired: Vec<(Entity, ScreenshotResponder)> = {
        let Some(mut responders) = world.get_resource_mut::<PendingScreenshotResponders>() else {
            return;
        };
        let stuck: Vec<Entity> = responders
            .map
            .iter_mut()
            .filter_map(|(entity, responder)| {
                responder.frames_waited += 1;
                (responder.frames_waited > MAX_CAPTURE_WAIT_FRAMES).then_some(*entity)
            })
            .collect();
        stuck
            .into_iter()
            .filter_map(|entity| {
                responders
                    .map
                    .remove(&entity)
                    .map(|responder| (entity, responder))
            })
            .collect()
    };

    for (entity, responder) in expired {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
        let _ = responder
            .response_tx
            .send(Err(ControlError::internal(CAPTURE_DEADLINE_ERROR)));
        release_internal_overlay(world);
        if let Some(restore) = responder.ui_restore {
            restore_ui_nodes(world, restore);
        }
        if let Some(was_enabled) = responder.gizmo_restore {
            set_gizmos_enabled(world, was_enabled);
        }
        if let Some(isolation) = responder.entity_isolation {
            isolation.restore_world(world);
        }
        if let Some(cleanup) = responder.debug_cleanup {
            cleanup_debug_camera_world(cleanup, world);
        }
    }
}

/// Fail an active timeline, restoring the capture state it held.
fn fail_timeline(
    world: &mut World,
    timeline_id: u64,
    mut timeline: ActiveTimeline,
    message: String,
) {
    if let Some(mut captures) = world.get_resource_mut::<TimelineCaptures>() {
        captures.map.retain(|_, (id, _)| *id != timeline_id);
    }
    if let Some(cleanup) = timeline.debug_cleanup.take() {
        cleanup_debug_camera_world(cleanup, world);
    }
    if timeline.overlay_suppressed {
        release_internal_overlay(world);
    }
    if let Some(restore) = timeline.ui_restore.take() {
        restore_ui_nodes(world, restore);
    }
    if let Some(was_enabled) = timeline.gizmo_restore.take() {
        set_gizmos_enabled(world, was_enabled);
    }
    if let Some(tx) = timeline.response_tx.take() {
        let _ = tx.send(Err(ControlError::internal(message)));
    }
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

    // Collect timeline IDs that need a capture this frame
    let mut captures_to_spawn: Vec<(u64, u32)> = Vec::new();
    let mut completed_ids: Vec<u64> = Vec::new();
    let mut stalled_ids: Vec<u64> = Vec::new();
    let mut stale_headless_ids: Vec<u64> = Vec::new();

    for (&id, timeline) in timelines.active.iter_mut() {
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
                if !has_window
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

    // Apply capture visibility for the duration of each timeline when its
    // first capture becomes ready. Restore it after the contact sheet is
    // complete.
    for (id, _) in &captures_to_spawn {
        let capture_options = timelines.active.get(id).and_then(|timeline| {
            (!timeline.overlay_suppressed).then_some((timeline.hide_ui, timeline.with_gizmos))
        });
        if let Some((hide_ui, with_gizmos)) = capture_options {
            let (ui_restore, gizmo_restore) =
                prepare_capture_visibility(world, hide_ui, with_gizmos);
            if let Some(timeline) = timelines.active.get_mut(id) {
                timeline.overlay_suppressed = true;
                timeline.ui_restore = ui_restore;
                timeline.gizmo_restore = gizmo_restore;
            }
        }
    }

    world.insert_resource(timelines);

    if has_window {
        // Spawn screenshot entities for captures
        for (timeline_id, capture_index) in captures_to_spawn {
            let entity = world.spawn(Screenshot::primary_window()).id();

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
                let mut overlay_releases: u32 = 0;
                let mut camera_cleanups = Vec::new();
                let mut ui_restores = Vec::new();
                let mut gizmo_restores = Vec::new();
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
                                if timeline.overlay_suppressed {
                                    timeline.overlay_suppressed = false;
                                    overlay_releases += 1;
                                }
                                if let Some(cleanup) = timeline.debug_cleanup.take() {
                                    camera_cleanups.push(cleanup);
                                }
                                if let Some(restore) = timeline.ui_restore.take() {
                                    ui_restores.push(restore);
                                }
                                if let Some(was_enabled) = timeline.gizmo_restore.take() {
                                    gizmo_restores.push(was_enabled);
                                }
                            }
                        }
                    }
                }
                for cleanup in camera_cleanups {
                    cleanup_debug_camera_world(cleanup, world);
                }
                for _ in 0..overlay_releases {
                    release_internal_overlay(world);
                }
                for restore in ui_restores {
                    restore_ui_nodes(world, restore);
                }
                for was_enabled in gizmo_restores {
                    set_gizmos_enabled(world, was_enabled);
                }
            }
        }
    }
}

/// Set gizmo visibility for the default gizmo group, returning the original enabled state.
fn set_gizmos_enabled(world: &mut World, enabled: bool) -> Option<bool> {
    let mut store = world.get_resource_mut::<GizmoConfigStore>()?;
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    let was_enabled = config.enabled;
    if was_enabled != enabled {
        config.enabled = enabled;
        Some(was_enabled)
    } else {
        None
    }
}

/// Apply screenshot gizmo visibility before Python Update systems draw this frame.
pub(crate) fn prepare_pending_screenshot_gizmos(
    world: &mut World,
    screenshot: &mut PendingScreenshot,
) {
    if !screenshot.with_gizmos && screenshot.gizmo_restore.is_none() {
        screenshot.gizmo_restore = set_gizmos_enabled(world, false);
    }
}

/// Hide authored UI Node entities by setting their visibility to Hidden.
/// Returns a list of (entity, original_visibility) for restoration. Internal
/// overlay entities are excluded: they are owned by `OverlaySuppression`.
fn hide_ui_nodes(
    world: &mut World,
    keep_visible: Option<&HashSet<Entity>>,
) -> Vec<(Entity, Visibility)> {
    let mut ui_entities: Vec<(Entity, Visibility)> = Vec::new();
    let mut query = world
        .query_filtered::<(Entity, &Visibility, &bevy::ui::Node), Without<InternalOverlayUi>>();
    for (entity, vis, _) in query.iter(world) {
        if keep_visible.is_some_and(|keep| keep.contains(&entity)) {
            continue;
        }
        ui_entities.push((entity, *vis));
    }
    for (entity, _) in &ui_entities {
        if let Some(mut vis) = world.get_mut::<Visibility>(*entity) {
            *vis = Visibility::Hidden;
        }
        // Force immediate render-pipeline visibility update
        // (bypass PostUpdate VisibilityPropagate which has already run)
        if let Some(mut inherited) = world.get_mut::<InheritedVisibility>(*entity) {
            *inherited = InheritedVisibility::HIDDEN;
        }
        if let Some(mut view_vis) = world.get_mut::<ViewVisibility>(*entity) {
            *view_vis = ViewVisibility::HIDDEN;
        }
    }
    ui_entities
}

/// Restore UI nodes hidden by `hide_ui_nodes` to their recorded visibility.
pub(crate) fn restore_ui_nodes(world: &mut World, restore: Vec<(Entity, Visibility)>) {
    for (entity, original_vis) in restore {
        if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
            *vis = original_vis;
        }
    }
}

/// Suppress the internal overlay (hot reload status, error text): bump the
/// `OverlaySuppression` refcount and force-hide the overlay entities so a
/// capture triggered later this same frame is already clean (visibility
/// propagation has run by now). The overlay's own render system re-applies
/// visibility from the refcount every frame afterwards, and restores the
/// overlay once `release_internal_overlay` brings the count back to zero.
pub(crate) fn suppress_internal_overlay(world: &mut World) {
    world
        .get_resource_or_insert_with(OverlaySuppression::default)
        .0 += 1;

    let mut overlay_entities: Vec<Entity> = Vec::new();
    let mut query = world.query_filtered::<Entity, With<InternalOverlayUi>>();
    for entity in query.iter(world) {
        overlay_entities.push(entity);
    }
    for entity in overlay_entities {
        if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
            *vis = Visibility::Hidden;
        }
        // Force immediate render-pipeline visibility update
        // (bypass PostUpdate VisibilityPropagate which has already run)
        if let Some(mut inherited) = world.get_mut::<InheritedVisibility>(entity) {
            *inherited = InheritedVisibility::HIDDEN;
        }
        if let Some(mut view_vis) = world.get_mut::<ViewVisibility>(entity) {
            *view_vis = ViewVisibility::HIDDEN;
        }
    }
}

pub(crate) fn prepare_capture_visibility(
    world: &mut World,
    hide_ui: bool,
    with_gizmos: bool,
) -> (Option<Vec<(Entity, Visibility)>>, Option<bool>) {
    suppress_internal_overlay(world);
    let ui_restore = hide_ui.then(|| hide_ui_nodes(world, None));
    let gizmo_restore = (!with_gizmos)
        .then(|| set_gizmos_enabled(world, false))
        .flatten();
    (ui_restore, gizmo_restore)
}

/// Release one `suppress_internal_overlay` hold.
pub(crate) fn release_internal_overlay(world: &mut World) {
    if let Some(mut suppression) = world.get_resource_mut::<OverlaySuppression>() {
        suppression.0 = suppression.0.saturating_sub(1);
    }
}

/// Set up a debug camera for a screenshot.
///
/// Reuses an existing Camera3d when possible to avoid a crash in
/// `prepare_lights` — a newly spawned Camera3d won't have cascade shadow
/// data populated by `build_directional_light_cascades` (runs in PostUpdate,
/// before Last where we spawn), causing an `unwrap()` panic on the cascade
/// lookup.  Reusing an existing camera preserves the cascade data.
///
/// Falls back to spawning a new camera only if no Camera3d exists.
pub(crate) fn setup_debug_camera(
    world: &mut World,
    req: &DebugCameraRequest,
) -> Result<DebugCameraCleanup, ControlError> {
    setup_debug_camera_with_up(world, req, Vec3::Y)
}

pub(crate) fn setup_debug_camera_with_up(
    world: &mut World,
    req: &DebugCameraRequest,
    up: Vec3,
) -> Result<DebugCameraCleanup, ControlError> {
    let position = Vec3::from_array(req.position);
    let look_at = Vec3::from_array(req.look_at);
    let target_transform = Transform::from_translation(position).looking_at(look_at, up);

    // Collect all cameras and their active state
    let all_cameras: Vec<(Entity, bool)> = {
        let mut query = world.query::<(Entity, &Camera)>();
        query.iter(world).map(|(e, c)| (e, c.is_active)).collect()
    };

    // Find an existing Camera3d to reuse (prefer an active one)
    let reuse_entity: Option<Entity> = {
        let mut query = world.query_filtered::<(Entity, &Camera), With<Camera3d>>();
        let candidates: Vec<(Entity, bool)> =
            query.iter(world).map(|(e, c)| (e, c.is_active)).collect();
        candidates
            .iter()
            .find(|(_, active)| *active)
            .or(candidates.first())
            .map(|(e, _)| *e)
    };

    if let Some(reuse) = reuse_entity {
        let original_transform = world.get::<Transform>(reuse).copied().unwrap_or_default();
        let original_global_transform = world
            .get::<GlobalTransform>(reuse)
            .copied()
            .unwrap_or_default();
        let original_active = all_cameras
            .iter()
            .find(|(e, _)| *e == reuse)
            .map(|(_, a)| *a)
            .unwrap_or(true);

        // Move camera to debug position and ensure it is active.
        // Set both Transform and GlobalTransform so that view uniforms
        // (view.world_position) are correct even before propagate_transforms
        // runs in the next PostUpdate — this function runs in Last.
        if let Some(mut t) = world.get_mut::<Transform>(reuse) {
            *t = target_transform;
        }
        if let Some(mut gt) = world.get_mut::<GlobalTransform>(reuse) {
            *gt = GlobalTransform::from(target_transform);
        }
        if let Some(mut cam) = world.get_mut::<Camera>(reuse) {
            cam.is_active = true;
        }

        // Disable all other cameras
        let other_cameras: Vec<(Entity, bool)> = all_cameras
            .into_iter()
            .filter(|(e, _)| *e != reuse)
            .collect();
        for (entity, _) in &other_cameras {
            if let Some(mut cam) = world.get_mut::<Camera>(*entity) {
                cam.is_active = false;
            }
        }

        Ok(DebugCameraCleanup {
            debug_entity: reuse,
            reused_state: Some((
                original_transform,
                original_global_transform,
                original_active,
            )),
            original_cameras: other_cameras,
        })
    } else {
        let has_camera2d = world
            .query_filtered::<Entity, With<Camera2d>>()
            .iter(world)
            .next()
            .is_some();
        if has_camera2d {
            return Err(ControlError::invalid_params(
                "position/look_at screenshot overrides require a Camera3d; omit them when capturing a Camera2d scene",
            ));
        }

        // No Camera3d exists — spawn a new one.
        // Cascade crash is unlikely here since the scene probably has no directional lights.
        let original_cameras = all_cameras.clone();
        for (entity, _) in &all_cameras {
            if let Some(mut cam) = world.get_mut::<Camera>(*entity) {
                cam.is_active = false;
            }
        }

        let debug_entity = world
            .spawn((
                Camera3d::default(),
                target_transform,
                GlobalTransform::from(target_transform),
            ))
            .id();

        Ok(DebugCameraCleanup {
            debug_entity,
            reused_state: None,
            original_cameras,
        })
    }
}

pub(crate) fn cleanup_debug_camera_world(cleanup: DebugCameraCleanup, world: &mut World) {
    if let Some((original_transform, original_global_transform, was_active)) = cleanup.reused_state
    {
        if let Some(mut transform) = world.get_mut::<Transform>(cleanup.debug_entity) {
            *transform = original_transform;
        }
        if let Some(mut global_transform) = world.get_mut::<GlobalTransform>(cleanup.debug_entity) {
            *global_transform = original_global_transform;
        }
        if let Some(mut camera) = world.get_mut::<Camera>(cleanup.debug_entity) {
            camera.is_active = was_active;
        }
    } else if world.get_entity(cleanup.debug_entity).is_ok() {
        world.despawn(cleanup.debug_entity);
    }

    for (entity, was_active) in cleanup.original_cameras {
        if let Some(mut camera) = world.get_mut::<Camera>(entity) {
            camera.is_active = was_active;
        }
    }
}

/// Clean up a debug camera: restore or despawn it, then restore original cameras.
fn cleanup_debug_camera(
    cleanup: DebugCameraCleanup,
    commands: &mut Commands,
    cameras: &mut Query<&mut Camera>,
    transforms: &mut Query<&mut Transform>,
    global_transforms: &mut Query<&mut GlobalTransform>,
) {
    if let Some((orig_transform, orig_global_transform, was_active)) = cleanup.reused_state {
        if let Ok(mut t) = transforms.get_mut(cleanup.debug_entity) {
            *t = orig_transform;
        }
        // Restore GlobalTransform so propagate_transforms doesn't need an extra frame
        if let Ok(mut gt) = global_transforms.get_mut(cleanup.debug_entity) {
            *gt = orig_global_transform;
        }
        if let Ok(mut cam) = cameras.get_mut(cleanup.debug_entity) {
            cam.is_active = was_active;
        }
    } else {
        commands.entity(cleanup.debug_entity).despawn();
    }
    for (cam_entity, was_active) in cleanup.original_cameras {
        if let Ok(mut cam) = cameras.get_mut(cam_entity) {
            cam.is_active = was_active;
        }
    }
}

/// Global observer triggered by Bevy when a screenshot is captured.
/// Handles normal screenshots, timeline captures, and turnaround captures.
#[allow(clippy::too_many_arguments)]
pub fn screenshot_captured_observer(
    trigger: On<ScreenshotCaptured>,
    mut responders: ResMut<PendingScreenshotResponders>,
    mut timeline_captures: ResMut<TimelineCaptures>,
    mut timelines: ResMut<PendingTimelines>,
    mut turnarounds: ResMut<super::turnaround::PendingTurnarounds>,
    mut turnaround_captures: ResMut<super::turnaround::TurnaroundCaptures>,
    mut captured_frames: ResMut<CapturedFrames>,
    hot_reload_generation: Option<Res<HotReloadGeneration>>,
    mut gizmo_store: Option<ResMut<GizmoConfigStore>>,
    mut overlay_suppression: Option<ResMut<OverlaySuppression>>,
    mut commands: Commands,
    mut cameras: Query<&mut Camera>,
    mut transforms: Query<&mut Transform>,
    mut global_transforms: Query<&mut GlobalTransform>,
    mut visibility_query: Query<&mut Visibility>,
) {
    let entity = trigger.entity;

    // Check if this is a turnaround capture
    if super::turnaround::handle_turnaround_capture(
        entity,
        &trigger.image,
        &mut turnarounds,
        &mut turnaround_captures,
    ) {
        return;
    }

    // Check if this is a timeline capture
    if let Some((timeline_id, capture_index)) = timeline_captures.map.remove(&entity) {
        let img = trigger.image.clone();
        let dyn_img = match img.try_into_dynamic() {
            Ok(d) => d,
            Err(e) => {
                bevy::log::error!("[MCP] Timeline capture failed to convert image: {e:?}");
                return;
            }
        };
        let rgb = dyn_img.to_rgb8();

        if let Some(timeline) = timelines.active.get_mut(&timeline_id) {
            timeline.collected.push((capture_index, rgb));
            timeline.stall_frames = 0;

            // Check if all captures collected
            if timeline.collected.len() as u32 == timeline.total_captures {
                let result = composite_contact_sheet(timeline);
                if let Some(tx) = timeline.response_tx.take() {
                    let _ = tx.send(result);
                }

                // Release the overlay suppression held by this timeline
                if timeline.overlay_suppressed {
                    timeline.overlay_suppressed = false;
                    if let Some(suppression) = overlay_suppression.as_deref_mut() {
                        suppression.0 = suppression.0.saturating_sub(1);
                    }
                }

                if let Some(restore) = timeline.ui_restore.take() {
                    for (ui_entity, original_vis) in restore {
                        if let Ok(mut vis) = visibility_query.get_mut(ui_entity) {
                            *vis = original_vis;
                        }
                    }
                }

                if let Some(was_enabled) = timeline.gizmo_restore.take()
                    && let Some(store) = gizmo_store.as_deref_mut()
                {
                    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
                    config.enabled = was_enabled;
                }

                // Clean up debug camera if present
                if let Some(cleanup) = timeline.debug_cleanup.take() {
                    cleanup_debug_camera(
                        cleanup,
                        &mut commands,
                        &mut cameras,
                        &mut transforms,
                        &mut global_transforms,
                    );
                }
            }
        }

        return;
    }

    // Normal screenshot handling
    let Some(responder) = responders.map.remove(&entity) else {
        return;
    };

    let img = trigger.image.clone();
    let result = complete_screenshot_capture(
        img,
        responder.max_width,
        responder.response_kind,
        &mut captured_frames,
        hot_reload_generation.map(|generation| generation.current),
    );
    let result = merge_extra_response(result, responder.extra_response);
    let _ = responder.response_tx.send(result);

    // Release this screenshot's overlay suppression
    if let Some(suppression) = overlay_suppression.as_deref_mut() {
        suppression.0 = suppression.0.saturating_sub(1);
    }

    // Restore hidden UI nodes
    if let Some(restore) = responder.ui_restore {
        for (ui_entity, original_vis) in restore {
            if let Ok(mut vis) = visibility_query.get_mut(ui_entity) {
                *vis = original_vis;
            }
        }
    }

    // Restore gizmo visibility
    if let Some(was_enabled) = responder.gizmo_restore
        && let Some(mut store) = gizmo_store
    {
        let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
        config.enabled = was_enabled;
    }

    if let Some(isolation) = responder.entity_isolation {
        commands.queue(move |world: &mut World| isolation.restore_world(world));
    }

    // Clean up debug camera if present
    if let Some(cleanup) = responder.debug_cleanup {
        cleanup_debug_camera(
            cleanup,
            &mut commands,
            &mut cameras,
            &mut transforms,
            &mut global_transforms,
        );
    }
}

/// Composite timeline captures into a contact sheet grid image.
fn composite_contact_sheet(
    timeline: &mut ActiveTimeline,
) -> Result<serde_json::Value, ControlError> {
    // Sort by capture index
    timeline.collected.sort_by_key(|(idx, _)| *idx);

    let cols = timeline.columns.max(1) as usize;
    let count = timeline.collected.len();
    let rows = count.div_ceil(cols);

    if count == 0 {
        return Err(ControlError::internal("No captures collected"));
    }

    // Use the first image's dimensions as cell size
    let cell_w = timeline.collected[0].1.width();
    let cell_h = timeline.collected[0].1.height();
    let bar_height = 4u32;

    let canvas_w = cell_w * cols as u32;
    let canvas_h = (cell_h + bar_height) * rows as u32;

    // Dark background
    let mut canvas = RgbImage::from_pixel(canvas_w, canvas_h, Rgb([40, 40, 40]));

    for (i, (_, frame)) in timeline.collected.iter().enumerate() {
        let col = (i % cols) as u32;
        let row = (i / cols) as u32;
        let x = col * cell_w;
        let y = row * (cell_h + bar_height);

        // Resize frame to cell size if needed
        let resized;
        let src = if frame.width() != cell_w || frame.height() != cell_h {
            resized = image::imageops::resize(
                frame,
                cell_w,
                cell_h,
                image::imageops::FilterType::Triangle,
            );
            &resized
        } else {
            frame
        };

        image::imageops::overlay(&mut canvas, src, x as i64, y as i64);

        // Colored bar at bottom (hue cycling for visual ordering)
        let hue = (i as f64 / count.max(1) as f64) * 300.0; // 0..300 degrees
        let color = hsv_to_rgb(hue, 0.8, 0.9);
        let bar_y = y + cell_h;
        for bx in x..x + cell_w {
            for by in bar_y..bar_y + bar_height {
                if bx < canvas_w && by < canvas_h {
                    canvas.put_pixel(bx, by, color);
                }
            }
        }
    }

    // Resize if max_width set
    if let Some(max_w) = timeline.max_width
        && canvas.width() > max_w
    {
        let scale = max_w as f64 / canvas.width() as f64;
        let new_height = (canvas.height() as f64 * scale).round() as u32;
        canvas = image::imageops::resize(
            &canvas,
            max_w,
            new_height,
            image::imageops::FilterType::Triangle,
        );
    }

    let width = canvas.width();
    let height = canvas.height();

    let mut buf = Vec::new();
    canvas
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| ControlError::internal(format!("Failed to encode timeline PNG: {e}")))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);

    Ok(serde_json::json!({
        "image": b64,
        "width": width,
        "height": height,
        "format": "png",
        "encoding": "base64"
    }))
}

/// Convert HSV (h: 0..360, s: 0..1, v: 0..1) to RGB.
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb<u8> {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Rgb([
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ])
}

/// Encode a Bevy Image as a base64 PNG string, optionally resizing.
/// If `extra` is `Some`, merges its fields into the screenshot result alongside
/// screenshot/screenshot_width/screenshot_height. On screenshot error, the extra
/// fields are still returned with a null screenshot.
fn merge_extra_response(
    result: Result<serde_json::Value, ControlError>,
    extra: Option<serde_json::Value>,
) -> Result<serde_json::Value, ControlError> {
    let Some(extra) = extra else {
        return result;
    };
    let mut merged = match result {
        Ok(screenshot) => {
            let mut response = serde_json::json!({
                "screenshot": screenshot.get("image"),
                "screenshot_width": screenshot.get("width"),
                "screenshot_height": screenshot.get("height"),
            });
            if let Some(response) = response.as_object_mut() {
                for key in ["frame_id", "retained", "retention_reason"] {
                    if let Some(value) = screenshot.get(key) {
                        response.insert(key.to_string(), value.clone());
                    }
                }
            }
            response
        }
        Err(error) => serde_json::json!({
            "screenshot": null,
            "screenshot_error": error.message,
        }),
    };
    if let (Some(base), serde_json::Value::Object(extra_map)) = (merged.as_object_mut(), extra) {
        for (k, v) in extra_map {
            base.insert(k, v);
        }
    }
    Ok(merged)
}

#[cfg(test)]
fn encode_screenshot(
    img: bevy::image::Image,
    max_width: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let dyn_img = img.try_into_dynamic().map_err(|e| {
        ControlError::internal(format!("Failed to convert screenshot image: {e:?}"))
    })?;

    // Discard alpha (stores HDR brightness) to get a clean RGB image
    let rgb = resize_rgb_image_linear(dyn_img.to_rgb8(), max_width);
    encode_rgb_screenshot(&rgb)
}

fn complete_screenshot_capture(
    img: bevy::image::Image,
    max_width: Option<u32>,
    response_kind: CaptureResponseKind,
    captured_frames: &mut CapturedFrames,
    hot_reload_generation: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let dyn_img = img.try_into_dynamic().map_err(|error| {
        ControlError::internal(format!("Failed to convert screenshot image: {error:?}"))
    })?;
    complete_rgb_capture(
        dyn_img.to_rgb8(),
        max_width,
        response_kind,
        captured_frames,
        hot_reload_generation,
    )
}

fn complete_rgb_capture(
    rgb: RgbImage,
    max_width: Option<u32>,
    response_kind: CaptureResponseKind,
    captured_frames: &mut CapturedFrames,
    hot_reload_generation: Option<u32>,
) -> Result<serde_json::Value, ControlError> {
    let rgb = Arc::new(resize_rgb_image_linear(rgb, max_width));
    let (mut result, kind) = match response_kind {
        CaptureResponseKind::Screenshot => (encode_rgb_screenshot(&rgb)?, "screenshot"),
        CaptureResponseKind::UnretainedScreenshot => return encode_rgb_screenshot(&rgb),
        CaptureResponseKind::Stats(options) => (analyze_frame(&rgb, &options)?, "stats"),
    };
    let retention = captured_frames.retain(
        rgb,
        CapturedFrameMetadata {
            kind,
            max_width,
            hot_reload_generation,
        },
    );
    retention.insert_into(
        result
            .as_object_mut()
            .ok_or_else(|| ControlError::internal("Capture response was not an object"))?,
    );
    Ok(result)
}

/// Encode an RgbImage as a base64 PNG string.
fn encode_rgb_screenshot(rgb: &RgbImage) -> Result<serde_json::Value, ControlError> {
    let width = rgb.width();
    let height = rgb.height();

    let mut buf = Vec::new();
    rgb.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| ControlError::internal(format!("Failed to encode PNG: {e}")))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);

    Ok(serde_json::json!({
        "image": b64,
        "width": width,
        "height": height,
        "format": "png",
        "encoding": "base64"
    }))
}

/// Read the latest frame from `HeadlessFrameBuffer` and convert to an RgbImage.
pub fn read_headless_frame(world: &World) -> Result<RgbImage, ControlError> {
    let buffer = world.get_resource::<HeadlessFrameBuffer>().ok_or_else(|| {
        ControlError::internal(
            "No primary window and no HeadlessFrameBuffer registered. \
             Add ImageCopyPlugin and a camera with RenderTarget.Image(ImageRenderTarget(...)) for headless screenshots."
                .to_string(),
        )
    })?;

    let (rgba_bytes, w, h) = buffer.latest.as_ref().ok_or_else(|| {
        ControlError::internal(
            "HeadlessFrameBuffer has no frame yet. \
             The render pipeline may not have produced a frame yet."
                .to_string(),
        )
    })?;

    let mut rgb = RgbImage::new(*w, *h);
    for y in 0..*h {
        for x in 0..*w {
            let i = ((y * *w + x) * 4) as usize;
            if i + 2 < rgba_bytes.len() {
                rgb.put_pixel(
                    x,
                    y,
                    Rgb([rgba_bytes[i], rgba_bytes[i + 1], rgba_bytes[i + 2]]),
                );
            }
        }
    }

    Ok(rgb)
}

/// Capture a frame from the headless readback buffer and encode it as a screenshot.
fn capture_headless_frame(
    world: &mut World,
    max_width: Option<u32>,
    response_kind: CaptureResponseKind,
) -> Result<serde_json::Value, ControlError> {
    let rgb = read_headless_frame(world)?;
    let hot_reload_generation = world
        .get_resource::<HotReloadGeneration>()
        .map(|generation| generation.current);
    let mut captured_frames = world.get_resource_or_insert_with(CapturedFrames::default);
    complete_rgb_capture(
        rgb,
        max_width,
        response_kind,
        &mut captured_frames,
        hot_reload_generation,
    )
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::RenderAssetUsages,
        image::Image,
        render::{
            render_resource::{Extent3d, TextureDimension, TextureFormat},
            view::window::screenshot::Screenshot,
        },
        ui::Node,
    };

    use super::*;
    use crate::{bridge::PendingScreenshot, handlers::frame_analysis::FrameStatsOptions};

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
    fn setup_debug_camera_reuses_existing_camera3d() {
        let mut world = World::new();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let cam_entity = world.spawn((Camera3d::default(), original_transform)).id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Should reuse the existing entity, not spawn a new one
        assert_eq!(cleanup.debug_entity, cam_entity);
        assert!(cleanup.reused_state.is_some());

        let (saved_transform, _saved_global_transform, saved_active) =
            cleanup.reused_state.unwrap();
        assert_eq!(saved_transform.translation, original_transform.translation);
        assert!(saved_active); // Camera3d defaults to active

        // Camera should be moved to the debug position
        let current = world.get::<Transform>(cam_entity).unwrap();
        assert!(
            (current.translation.x - 10.0).abs() < 0.01,
            "Camera should be at debug position"
        );
    }

    #[test]
    fn setup_debug_camera_supports_a_vertical_top_view() {
        let mut world = World::new();
        let camera = world.spawn(Camera3d::default()).id();
        let request = DebugCameraRequest {
            position: [0.0, 10.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };

        setup_debug_camera_with_up(&mut world, &request, Vec3::Z).unwrap();

        let transform = world.get::<Transform>(camera).unwrap();
        assert!(transform.rotation.is_finite());
        assert!(transform.forward().as_vec3().dot(Vec3::NEG_Y) > 0.999);
    }

    #[test]
    fn setup_debug_camera_disables_other_cameras() {
        let mut world = World::new();
        let cam_a = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();
        let cam_b = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // One camera is reused (active), the other is disabled
        let reused = cleanup.debug_entity;
        let other = if reused == cam_a { cam_b } else { cam_a };

        assert!(world.get::<Camera>(reused).unwrap().is_active);
        assert!(!world.get::<Camera>(other).unwrap().is_active);

        // original_cameras should contain only the non-reused camera
        assert_eq!(cleanup.original_cameras.len(), 1);
        assert_eq!(cleanup.original_cameras[0].0, other);
    }

    #[test]
    fn setup_debug_camera_spawns_when_no_camera3d() {
        let mut world = World::new();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Should have spawned a new camera (reused_state is None)
        assert!(cleanup.reused_state.is_none());
        assert!(world.get::<Camera3d>(cleanup.debug_entity).is_some());
    }

    #[test]
    fn setup_debug_camera_rejects_camera2d_only_scene() {
        let mut world = World::new();
        let camera = world.spawn(Camera2d::default()).id();
        let req = DebugCameraRequest {
            position: [0.0, 0.0, 500.0],
            look_at: [0.0, 0.0, 0.0],
        };

        let error = setup_debug_camera(&mut world, &req).err().unwrap();
        assert!(error.message.contains("require a Camera3d"));
        assert!(world.get::<Camera>(camera).unwrap().is_active);
        assert_eq!(
            world
                .query_filtered::<Entity, With<Camera3d>>()
                .iter(&world)
                .count(),
            0
        );
    }

    #[test]
    fn rejected_camera2d_override_restores_capture_state() {
        let mut world = world_with_gizmos();
        let camera = world.spawn(Camera2d::default()).id();
        let ui = world.spawn((Visibility::Visible, Node::default())).id();
        let (response_tx, mut response_rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![crate::bridge::PendingScreenshot {
                response_tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos: false,
                gizmo_restore: None,
                max_width: None,
                debug_camera: Some(DebugCameraRequest {
                    position: [0.0, 0.0, 500.0],
                    look_at: [0.0, 0.0, 0.0],
                }),
                hide_ui: true,
                entity: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        let error = response_rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("require a Camera3d"));
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert!(world.get::<Camera>(camera).unwrap().is_active);
        assert!(
            world
                .get_resource::<OverlaySuppression>()
                .is_none_or(|suppression| suppression.0 == 0)
        );
    }

    #[test]
    fn cleanup_debug_camera_world_despawns_temporary_camera() {
        let mut world = World::new();
        let cleanup = setup_debug_camera(
            &mut world,
            &DebugCameraRequest {
                position: [10.0, 5.0, 0.0],
                look_at: [0.0, 0.0, 0.0],
            },
        )
        .unwrap();
        let debug_entity = cleanup.debug_entity;

        cleanup_debug_camera_world(cleanup, &mut world);

        assert!(world.get_entity(debug_entity).is_err());
    }

    #[test]
    fn setup_debug_camera_prefers_active_camera() {
        let mut world = World::new();
        let inactive = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();
        // Deactivate the first camera
        world.get_mut::<Camera>(inactive).unwrap().is_active = false;

        let active = world
            .spawn((Camera3d::default(), Transform::default()))
            .id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Should prefer the active camera
        assert_eq!(cleanup.debug_entity, active);
    }

    #[test]
    fn setup_debug_camera_cleanup_restores_reused_camera() {
        let mut world = World::new();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let cam = world.spawn((Camera3d::default(), original_transform)).id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        cleanup_debug_camera_world(cleanup, &mut world);

        // Camera should be restored to original position
        let restored = world.get::<Transform>(cam).unwrap();
        assert_eq!(restored.translation, original_transform.translation);
        assert!(world.get::<Camera>(cam).unwrap().is_active);
    }

    #[test]
    fn headless_debug_screenshot_restores_reused_camera() {
        let mut world = world_with_gizmos();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let camera = world
            .spawn((
                Camera3d::default(),
                original_transform,
                GlobalTransform::from(original_transform),
            ))
            .id();
        let cleanup = setup_debug_camera(
            &mut world,
            &DebugCameraRequest {
                position: [10.0, 5.0, 0.0],
                look_at: [0.0, 0.0, 0.0],
            },
        )
        .unwrap();
        let (response_tx, _response_rx) = oneshot::channel();
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx,
                frames_remaining: 0,
                with_gizmos: false,
                gizmo_restore: None,
                max_width: None,
                debug_cleanup: Some(cleanup),
                ui_restore: None,
                entity_isolation: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: None,
                frames_waited: 0,
            }],
        });
        world.insert_resource(PendingScreenshots::default());

        process_pending_screenshots(&mut world);

        assert_eq!(
            world.get::<Transform>(camera).unwrap().translation,
            original_transform.translation
        );
        assert!(world.get::<Camera>(camera).unwrap().is_active);
    }

    #[test]
    fn setup_debug_camera_sets_global_transform() {
        let mut world = World::new();
        let original_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let original_global = GlobalTransform::from(original_transform);
        let cam = world
            .spawn((Camera3d::default(), original_transform, original_global))
            .id();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // GlobalTransform should be updated to match the debug position
        let gt = world.get::<GlobalTransform>(cam).unwrap();
        let debug_pos = gt.translation();
        assert!(
            (debug_pos.x - 10.0).abs() < 0.01,
            "GlobalTransform should be at debug position, got {debug_pos:?}"
        );

        // Saved state should contain the original GlobalTransform
        let (_saved_t, saved_gt, _saved_active) = cleanup.reused_state.unwrap();
        assert_eq!(
            saved_gt.translation(),
            original_global.translation(),
            "Should save original GlobalTransform for cleanup"
        );
    }

    #[test]
    fn setup_debug_camera_spawns_with_global_transform() {
        let mut world = World::new();

        let req = DebugCameraRequest {
            position: [10.0, 5.0, 0.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let cleanup = setup_debug_camera(&mut world, &req).unwrap();

        // Spawned camera should have GlobalTransform set
        let gt = world.get::<GlobalTransform>(cleanup.debug_entity);
        assert!(
            gt.is_some(),
            "Spawned debug camera should have GlobalTransform"
        );
        let debug_pos = gt.unwrap().translation();
        assert!(
            (debug_pos.x - 10.0).abs() < 0.01,
            "Spawned GlobalTransform should match debug position, got {debug_pos:?}"
        );
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

    #[test]
    fn set_gizmos_enabled_disables_when_enabled() {
        let mut world = world_with_gizmos();
        assert!(gizmos_enabled(&world));

        let restore = set_gizmos_enabled(&mut world, false);
        assert!(!gizmos_enabled(&world));
        assert_eq!(restore, Some(true));
    }

    #[test]
    fn set_gizmos_enabled_noop_when_already_matches() {
        let mut world = world_with_gizmos();
        // Gizmos default to enabled; setting true should be a no-op
        let restore = set_gizmos_enabled(&mut world, true);
        assert!(gizmos_enabled(&world));
        assert_eq!(restore, None);
    }

    #[test]
    fn set_gizmos_enabled_returns_none_without_resource() {
        let mut world = World::new();
        let restore = set_gizmos_enabled(&mut world, false);
        assert_eq!(restore, None);
    }

    #[test]
    fn set_gizmos_enabled_roundtrip_restores_state() {
        let mut world = world_with_gizmos();
        assert!(gizmos_enabled(&world));

        // Disable gizmos
        let restore = set_gizmos_enabled(&mut world, false);
        assert!(!gizmos_enabled(&world));

        // Restore original state
        if let Some(was_enabled) = restore {
            set_gizmos_enabled(&mut world, was_enabled);
        }
        assert!(gizmos_enabled(&world));
    }

    #[test]
    fn set_gizmos_enabled_double_disable_second_is_noop() {
        let mut world = world_with_gizmos();

        let first = set_gizmos_enabled(&mut world, false);
        assert_eq!(first, Some(true));

        // Already disabled — should be a no-op
        let second = set_gizmos_enabled(&mut world, false);
        assert_eq!(second, None);
    }

    /// Set up a World with gizmos + PendingScreenshots + a PrimaryWindow entity.
    fn world_with_pending_screenshot(
        with_gizmos: bool,
    ) -> (
        World,
        oneshot::Receiver<Result<serde_json::Value, ControlError>>,
    ) {
        let mut world = world_with_gizmos();
        // Spawn a PrimaryWindow entity so the windowed screenshot path is taken
        world.spawn(PrimaryWindow);
        let (tx, rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos,
                gizmo_restore: None,
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                entity: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });
        (world, rx)
    }

    #[test]
    fn process_screenshot_disables_gizmos_for_normal_capture() {
        let (mut world, _rx) = world_with_pending_screenshot(false);
        assert!(gizmos_enabled(&world));

        process_pending_screenshots(&mut world);

        // Gizmos should be disabled after processing a non-gizmo screenshot
        assert!(!gizmos_enabled(&world));
    }

    #[test]
    fn process_screenshot_leaves_gizmos_for_gizmo_capture() {
        let (mut world, _rx) = world_with_pending_screenshot(true);
        assert!(gizmos_enabled(&world));

        process_pending_screenshots(&mut world);

        // Gizmos should remain enabled for a gizmo screenshot
        assert!(gizmos_enabled(&world));
    }

    #[test]
    fn process_screenshot_spawns_screenshot_entity() {
        let (mut world, _rx) = world_with_pending_screenshot(false);

        process_pending_screenshots(&mut world);

        // A Screenshot entity should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 1);
    }

    #[test]
    fn process_screenshot_stores_gizmo_restore_in_responder() {
        let (mut world, _rx) = world_with_pending_screenshot(false);

        process_pending_screenshots(&mut world);

        let responders = world.resource::<PendingScreenshotResponders>();
        assert_eq!(responders.map.len(), 1);
        let responder = responders.map.values().next().unwrap();
        // Should store the original enabled state for restoration
        assert_eq!(responder.gizmo_restore, Some(true));
    }

    #[test]
    fn process_screenshot_gizmo_capture_has_no_restore() {
        let (mut world, _rx) = world_with_pending_screenshot(true);

        process_pending_screenshots(&mut world);

        let responders = world.resource::<PendingScreenshotResponders>();
        let responder = responders.map.values().next().unwrap();
        // Gizmo screenshot should not need restoration
        assert_eq!(responder.gizmo_restore, None);
    }

    #[test]
    fn process_screenshot_with_delay_does_not_capture_yet() {
        let mut world = world_with_gizmos();
        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 2,
                required_render_epoch: None,
                with_gizmos: false,
                gizmo_restore: None,
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                entity: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        // Should not have spawned a Screenshot yet
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
        // Suppression begins while the capture is still delayed, before the
        // next Update systems can submit global gizmos.
        assert!(!gizmos_enabled(&world));
        // Request should still be pending with decremented count
        let pending = world.resource::<PendingScreenshots>();
        assert_eq!(pending.pending.len(), 1);
        assert_eq!(pending.pending[0].frames_remaining, 1);
        assert_eq!(pending.pending[0].gizmo_restore, Some(true));
    }

    #[test]
    fn pending_capture_waits_for_its_required_render_epoch() {
        let mut world = world_with_gizmos();
        let readiness = RenderFrameReadiness::default();
        let required_render_epoch = readiness.request_frame();
        world.insert_resource(readiness.clone());
        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 2,
                required_render_epoch: Some(required_render_epoch),
                with_gizmos: false,
                gizmo_restore: None,
                max_width: None,
                debug_camera: None,
                hide_ui: false,
                entity: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);
        assert_eq!(
            world.resource::<PendingScreenshots>().pending[0].frames_remaining,
            2
        );

        readiness.complete_requested_frame();
        process_pending_screenshots(&mut world);
        assert_eq!(
            world.resource::<PendingScreenshots>().pending[0].frames_remaining,
            1
        );
    }

    #[test]
    fn headless_ui_capture_waits_for_hidden_frame() {
        let mut world = world_with_gizmos();
        let ui = world.spawn((Visibility::Visible, Node::default())).id();
        let (response_tx, _response_rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos: false,
                gizmo_restore: None,
                max_width: None,
                debug_camera: None,
                hide_ui: true,
                entity: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Hidden);
        let staged = world.resource::<StagedScreenshots>();
        assert_eq!(staged.pending.len(), 1);
        assert_eq!(staged.pending[0].frames_remaining, 1);
        assert!(staged.pending[0].debug_cleanup.is_none());
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
    }

    #[test]
    fn process_screenshot_debug_camera_stages_with_gizmo_flag() {
        let mut world = world_with_gizmos();
        // Need a Camera3d for setup_debug_camera to reuse
        world.spawn((Camera3d::default(), Transform::default()));

        let (tx, _rx) = oneshot::channel();
        world.insert_resource(PendingScreenshots {
            pending: vec![PendingScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                required_render_epoch: None,
                with_gizmos: true,
                gizmo_restore: None,
                max_width: Some(512),
                debug_camera: Some(DebugCameraRequest {
                    position: [10.0, 5.0, 0.0],
                    look_at: [0.0, 0.0, 0.0],
                }),
                hide_ui: false,
                entity: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
            }],
        });

        process_pending_screenshots(&mut world);

        // Debug camera path stages the screenshot (extra 2 frame delay)
        // so no Screenshot entity yet, and gizmos untouched
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 0);
        assert!(gizmos_enabled(&world));

        // Staged screenshot should exist with with_gizmos preserved
        let staged = world.resource::<StagedScreenshots>();
        assert_eq!(staged.pending.len(), 1);
        assert!(staged.pending[0].with_gizmos);
        assert_eq!(staged.pending[0].max_width, Some(512));
    }

    #[test]
    fn process_staged_debug_screenshot_toggles_gizmos_on_capture() {
        let mut world = world_with_gizmos();
        // Spawn a PrimaryWindow entity so the windowed screenshot path is taken
        world.spawn(PrimaryWindow);

        // Directly insert a staged screenshot (simulating debug camera already set up)
        let (tx, _rx) = oneshot::channel();
        let cleanup = DebugCameraCleanup {
            debug_entity: world
                .spawn((Camera3d::default(), Transform::default()))
                .id(),
            reused_state: None,
            original_cameras: vec![],
        };
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: false, // Normal screenshot via debug camera
                gizmo_restore: None,
                max_width: None,
                debug_cleanup: Some(cleanup),
                ui_restore: None,
                entity_isolation: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: None,
                frames_waited: 0,
            }],
        });
        // Need PendingScreenshots resource for the function to proceed
        world.insert_resource(PendingScreenshots::default());

        process_pending_screenshots(&mut world);

        // Gizmos should be disabled (non-gizmo screenshot)
        assert!(!gizmos_enabled(&world));
        // Screenshot entity should have been spawned
        let mut query = world.query::<&Screenshot>();
        assert_eq!(query.iter(&world).count(), 1);
    }

    #[test]
    fn hsv_to_rgb_red() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), Rgb([255, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_green() {
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), Rgb([0, 255, 0]));
    }

    #[test]
    fn hsv_to_rgb_blue() {
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), Rgb([0, 0, 255]));
    }

    #[test]
    fn hsv_to_rgb_white() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), Rgb([255, 255, 255]));
    }

    #[test]
    fn hsv_to_rgb_black() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), Rgb([0, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_half_saturation() {
        let color = hsv_to_rgb(0.0, 0.5, 1.0);
        assert_eq!(color.0[0], 255);
        assert!((color.0[1] as i16 - 128).abs() <= 1);
        assert!((color.0[2] as i16 - 128).abs() <= 1);
    }

    #[test]
    fn composite_contact_sheet_single_frame() {
        let img = RgbImage::from_pixel(4, 4, Rgb([100, 100, 100]));
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: None,
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: None,
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline);
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.get("image").is_some());
        assert!(val["image"].as_str().unwrap().len() > 0);
        assert!(val.get("width").is_some());
        assert!(val.get("height").is_some());
        assert_eq!(val["format"], "png");
    }

    #[test]
    fn composite_contact_sheet_grid_layout() {
        let mut collected = Vec::new();
        for i in 0..4u32 {
            collected.push((i, RgbImage::from_pixel(4, 4, Rgb([100, 100, 100]))));
        }
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: None,
            columns: 2,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 4,
            next_capture_index: 0,
            collected,
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: None,
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline).unwrap();
        let bar_height = 4u32;
        // 2 columns, 4 images → 2 rows
        assert_eq!(result["width"], 2 * 4); // cols * cell_w
        assert_eq!(result["height"], (4 + bar_height) * 2); // (cell_h + bar) * rows
    }

    #[test]
    fn composite_contact_sheet_empty_captures() {
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: None,
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 0,
            next_capture_index: 0,
            collected: vec![],
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: None,
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline);
        assert!(result.is_err());
    }

    #[test]
    fn composite_contact_sheet_max_width_resize() {
        let img = RgbImage::from_pixel(100, 100, Rgb([100, 100, 100]));
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            response_tx: Some(tx),
            max_width: Some(50),
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: None,
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline).unwrap();
        assert_eq!(result["width"], 50);
    }

    #[test]
    fn hide_ui_nodes_empty_world() {
        let mut world = World::new();
        let result = hide_ui_nodes(&mut world, None);
        assert!(result.is_empty());
    }

    #[test]
    fn hide_ui_nodes_hides_nodes() {
        let mut world = World::new();
        let entity = world.spawn((Visibility::Visible, Node::default())).id();

        let hidden = hide_ui_nodes(&mut world, None);
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].0, entity);
        assert_eq!(hidden[0].1, Visibility::Visible);

        let vis = world.get::<Visibility>(entity).unwrap();
        assert_eq!(*vis, Visibility::Hidden);
    }

    #[test]
    fn entity_capture_isolates_subtree_camera_and_lighting_then_restores() {
        let mut world = World::new();
        let root = world
            .spawn((Name::new("Target"), RenderLayers::layer(2)))
            .id();
        let child = world.spawn(ChildOf(root)).id();
        let camera = world.spawn(Camera::default()).id();
        let mut light_class = VisibilityClass::default();
        light_class.push(TypeId::of::<ClusterVisibilityClass>());
        let light = world.spawn(light_class).id();
        let unrelated = world
            .spawn(RenderLayers::layer(1).with(ENTITY_CAPTURE_LAYER).with(7))
            .id();
        let default_unrelated = world.spawn_empty().id();

        let isolation = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("Target".to_string()),
            false,
        )
        .unwrap();

        for entity in [root, child, camera, light] {
            assert_eq!(
                world.get::<RenderLayers>(entity),
                Some(&RenderLayers::layer(ENTITY_CAPTURE_LAYER))
            );
        }
        assert_eq!(
            world
                .get::<RenderLayers>(unrelated)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec![1, 7]
        );
        assert!(world.get::<RenderLayers>(default_unrelated).is_none());
        assert!(world.contains_resource::<EntityCaptureIsolationActive>());

        isolation.restore_world(&mut world);

        assert_eq!(
            world.get::<RenderLayers>(root),
            Some(&RenderLayers::layer(2))
        );
        assert!(world.get::<RenderLayers>(child).is_none());
        assert!(world.get::<RenderLayers>(camera).is_none());
        assert!(world.get::<RenderLayers>(light).is_none());
        assert_eq!(
            world
                .get::<RenderLayers>(unrelated)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec![1, 7, ENTITY_CAPTURE_LAYER]
        );
        assert!(!world.contains_resource::<EntityCaptureIsolationActive>());
    }

    #[test]
    fn entity_capture_keeps_targeted_ui_visible_when_hiding_authored_ui() {
        let mut world = World::new();
        let root = world
            .spawn((Name::new("TargetUi"), Visibility::Visible, Node::default()))
            .id();
        let child = world
            .spawn((ChildOf(root), Visibility::Visible, Node::default()))
            .id();
        let unrelated = world.spawn((Visibility::Visible, Node::default())).id();
        let isolation = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("TargetUi".to_string()),
            false,
        )
        .unwrap();

        let hidden = hide_ui_nodes(&mut world, Some(&isolation.scope));

        assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Visible);
        assert_eq!(
            *world.get::<Visibility>(child).unwrap(),
            Visibility::Visible
        );
        assert_eq!(
            *world.get::<Visibility>(unrelated).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(hidden, vec![(unrelated, Visibility::Visible)]);
        isolation.restore_world(&mut world);
    }

    #[test]
    fn missing_entity_capture_does_not_change_render_layers() {
        let mut world = World::new();
        let camera = world
            .spawn((Camera::default(), RenderLayers::layer(4)))
            .id();

        let result = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("Missing".to_string()),
            false,
        );

        assert!(result.is_err());
        assert_eq!(
            world.get::<RenderLayers>(camera),
            Some(&RenderLayers::layer(4))
        );
        assert!(!world.contains_resource::<EntityCaptureIsolationActive>());
    }

    #[test]
    fn entity_capture_with_gizmos_restores_group_render_layers() {
        let mut world = world_with_gizmos();
        world.spawn(Name::new("Target"));
        {
            let mut store = world.resource_mut::<GizmoConfigStore>();
            let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
            config.enabled = false;
            config.render_layers = RenderLayers::layer(5);
        }

        let isolation = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("Target".to_string()),
            true,
        )
        .unwrap();
        {
            let store = world.resource::<GizmoConfigStore>();
            let config = store.config::<DefaultGizmoConfigGroup>().0;
            assert!(config.enabled);
            assert_eq!(
                config.render_layers,
                RenderLayers::layer(ENTITY_CAPTURE_LAYER)
            );
        }

        isolation.restore_world(&mut world);
        let store = world.resource::<GizmoConfigStore>();
        let config = store.config::<DefaultGizmoConfigGroup>().0;
        assert!(!config.enabled);
        assert_eq!(config.render_layers, RenderLayers::layer(5));
    }

    #[test]
    fn suppress_internal_overlay_counts_without_entities() {
        let mut world = World::new();
        suppress_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        // Releasing below zero saturates instead of underflowing
        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
    }

    #[test]
    fn suppress_internal_overlay_force_hides_and_refcounts() {
        let mut world = World::new();
        let entity = world.spawn((Visibility::Visible, InternalOverlayUi)).id();

        // Two overlapping captures compose via the refcount
        suppress_internal_overlay(&mut world);
        suppress_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 2);
        assert_eq!(
            *world.get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );

        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
        release_internal_overlay(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
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
                response_tx: Some(tx),
                max_width: None,
                columns: 2,
                debug_cleanup: None,
                schedule: VecDeque::from([0, 5]), // capture on this frame, then later
                total_captures: 2,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
                hide_ui: true,
                with_gizmos: false,
                ui_restore: None,
                gizmo_restore: None,
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
            world.resource::<PendingTimelines>().active[&0].overlay_suppressed,
            "timeline must record its hold so completion releases exactly once"
        );
        assert!(
            world.resource::<PendingTimelines>().active[&0]
                .ui_restore
                .is_some()
        );
        assert_eq!(
            world.resource::<PendingTimelines>().active[&0].gizmo_restore,
            Some(true)
        );

        // A second tick of the same timeline must not double-count
        process_pending_timelines(&mut world);
        assert_eq!(world.resource::<OverlaySuppression>().0, 1);
    }

    #[test]
    fn suppress_internal_overlay_ignores_non_overlay() {
        let mut world = World::new();
        let plain = world.spawn(Visibility::Visible).id();

        suppress_internal_overlay(&mut world);
        assert_eq!(
            *world.get::<Visibility>(plain).unwrap(),
            Visibility::Visible
        );
    }

    #[test]
    fn stuck_responder_expires_with_error_and_restored_state() {
        let mut world = world_with_gizmos();
        world.insert_resource(OverlaySuppression(1));
        let ui = world.spawn((Visibility::Hidden, Node::default())).id();
        let screenshot_entity = world.spawn_empty().id();

        let (tx, mut rx) = oneshot::channel();
        let mut responders = PendingScreenshotResponders::default();
        responders.map.insert(
            screenshot_entity,
            ScreenshotResponder {
                response_tx: tx,
                max_width: None,
                debug_cleanup: None,
                ui_restore: Some(vec![(ui, Visibility::Visible)]),
                entity_isolation: None,
                gizmo_restore: Some(true),
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                frames_waited: MAX_CAPTURE_WAIT_FRAMES,
            },
        );
        world.insert_resource(responders);
        world.insert_resource(PendingScreenshots::default());
        set_gizmos_enabled(&mut world, false);

        process_pending_screenshots(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("deadline"));
        assert!(
            world
                .resource::<PendingScreenshotResponders>()
                .map
                .is_empty()
        );
        assert!(
            !another_capture_is_active(&world),
            "expired responder must unwedge entity-isolated captures"
        );
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert!(gizmos_enabled(&world));
        assert!(world.get_entity(screenshot_entity).is_err());
    }

    #[test]
    fn waiting_responder_below_deadline_is_kept() {
        let mut world = world_with_gizmos();
        let screenshot_entity = world.spawn_empty().id();
        let (tx, mut rx) = oneshot::channel();
        let mut responders = PendingScreenshotResponders::default();
        responders.map.insert(
            screenshot_entity,
            ScreenshotResponder {
                response_tx: tx,
                max_width: None,
                debug_cleanup: None,
                ui_restore: None,
                entity_isolation: None,
                gizmo_restore: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                frames_waited: 0,
            },
        );
        world.insert_resource(responders);
        world.insert_resource(PendingScreenshots::default());

        process_pending_screenshots(&mut world);

        assert!(rx.try_recv().is_err(), "response must remain deferred");
        let responders = world.resource::<PendingScreenshotResponders>();
        assert_eq!(responders.map[&screenshot_entity].frames_waited, 1);
    }

    #[test]
    fn stale_headless_frame_fails_instead_of_becoming_a_capture() {
        let mut world = world_with_gizmos();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255], 1, 1)),
            sequence: 5,
        });
        world.insert_resource(OverlaySuppression(1));
        world.insert_resource(PendingScreenshots::default());
        let (tx, mut rx) = oneshot::channel();
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: false,
                gizmo_restore: None,
                max_width: None,
                debug_cleanup: None,
                ui_restore: None,
                entity_isolation: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: Some(5),
                frames_waited: MAX_CAPTURE_WAIT_FRAMES,
            }],
        });

        process_pending_screenshots(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert_eq!(error.message, STALE_HEADLESS_FRAME_ERROR);
        assert!(world.resource::<StagedScreenshots>().pending.is_empty());
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
    }

    #[test]
    fn newer_headless_frame_completes_the_staged_capture() {
        let mut world = world_with_gizmos();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![0, 255, 0, 255], 1, 1)),
            sequence: 6,
        });
        world.insert_resource(OverlaySuppression(1));
        world.insert_resource(PendingScreenshots::default());
        let (tx, mut rx) = oneshot::channel();
        world.insert_resource(StagedScreenshots {
            pending: vec![StagedScreenshot {
                response_tx: tx,
                frames_remaining: 0,
                with_gizmos: false,
                gizmo_restore: None,
                max_width: None,
                debug_cleanup: None,
                ui_restore: None,
                entity_isolation: None,
                extra_response: None,
                response_kind: CaptureResponseKind::Screenshot,
                baseline_headless_sequence: Some(5),
                frames_waited: 0,
            }],
        });

        process_pending_screenshots(&mut world);

        let capture = rx.try_recv().unwrap().unwrap();
        assert_eq!(capture["width"], 1);
        assert!(world.resource::<StagedScreenshots>().pending.is_empty());
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
    }

    #[test]
    fn stalled_timeline_fails_with_error_and_restored_state() {
        let mut world = world_with_gizmos();
        world.insert_resource(OverlaySuppression(1));
        let ui = world.spawn((Visibility::Hidden, Node::default())).id();
        let (tx, mut rx) = oneshot::channel();
        let mut timelines = PendingTimelines::default();
        timelines.active.insert(
            3,
            ActiveTimeline {
                response_tx: Some(tx),
                max_width: None,
                columns: 2,
                debug_cleanup: None,
                schedule: VecDeque::new(),
                total_captures: 2,
                next_capture_index: 2,
                collected: vec![],
                overlay_suppressed: true,
                hide_ui: true,
                with_gizmos: false,
                ui_restore: Some(vec![(ui, Visibility::Visible)]),
                gizmo_restore: Some(true),
                headless_sequence: None,
                stall_frames: MAX_CAPTURE_WAIT_FRAMES,
            },
        );
        world.insert_resource(timelines);
        let mut captures = TimelineCaptures::default();
        let stale = world.spawn_empty().id();
        captures.map.insert(stale, (3, 0));
        world.insert_resource(captures);
        set_gizmos_enabled(&mut world, false);

        process_pending_timelines(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("deadline"));
        assert!(world.resource::<PendingTimelines>().active.is_empty());
        assert!(world.resource::<TimelineCaptures>().map.is_empty());
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
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
                response_tx: Some(tx),
                max_width: None,
                columns: 2,
                debug_cleanup: None,
                schedule: VecDeque::from([0]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
                hide_ui: false,
                with_gizmos: true,
                ui_restore: None,
                gizmo_restore: None,
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
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                debug_cleanup: None,
                schedule: VecDeque::from([2]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
                hide_ui: true,
                with_gizmos: false,
                ui_restore: None,
                gizmo_restore: None,
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
    fn encode_screenshot_basic() {
        let img = make_test_image(100, 80);
        let result = encode_screenshot(img, None).unwrap();
        assert_eq!(result["width"], 100);
        assert_eq!(result["height"], 80);
        assert_eq!(result["format"], "png");
        assert_eq!(result["encoding"], "base64");
        assert!(result["image"].as_str().unwrap().len() > 0);
    }

    #[test]
    fn encode_screenshot_max_width_no_resize_needed() {
        let img = make_test_image(100, 80);
        let result = encode_screenshot(img, Some(200)).unwrap();
        // Image is 100px wide, max is 200, so no resize
        assert_eq!(result["width"], 100);
        assert_eq!(result["height"], 80);
    }

    #[test]
    fn encode_screenshot_max_width_resize() {
        let img = make_test_image(200, 100);
        let result = encode_screenshot(img, Some(100)).unwrap();
        assert_eq!(result["width"], 100);
        assert_eq!(result["height"], 50); // proportional
    }

    #[test]
    fn encode_screenshot_produces_valid_base64() {
        let img = make_test_image(10, 10);
        let result = encode_screenshot(img, None).unwrap();
        let b64 = result["image"].as_str().unwrap();
        let decoded = base64::engine::general_purpose::STANDARD.decode(b64);
        assert!(decoded.is_ok());
        // Should be a valid PNG
        let bytes = decoded.unwrap();
        assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn encode_screenshot_small_1x1() {
        let img = make_test_image(1, 1);
        let result = encode_screenshot(img, None).unwrap();
        assert_eq!(result["width"], 1);
        assert_eq!(result["height"], 1);
    }

    #[test]
    fn window_image_completion_uses_the_numeric_analysis_path() {
        let image = make_test_image(2, 1);
        let mut frames = CapturedFrames::default();
        let result = complete_screenshot_capture(
            image,
            None,
            CaptureResponseKind::Stats(FrameStatsOptions {
                grid: 1,
                region: None,
                sample_points: Some(vec![[1, 0]]),
            }),
            &mut frames,
            Some(7),
        )
        .unwrap();

        assert_eq!(result["overall"]["luma_mean"], 1.0);
        assert_eq!(
            result["samples"][0]["rgb"],
            serde_json::json!([1.0, 1.0, 1.0])
        );
        assert!(result.get("image").is_none());
        assert_eq!(result["retained"], true);
    }

    #[test]
    fn depth_rgb_completion_does_not_enter_frame_retention() {
        let image = make_test_image(1, 1);
        let mut frames = CapturedFrames::default();
        let result = complete_screenshot_capture(
            image,
            None,
            CaptureResponseKind::UnretainedScreenshot,
            &mut frames,
            None,
        )
        .unwrap();

        assert!(result.get("frame_id").is_none());
        assert!(
            frames
                .compare("f_0000000000000000", "f_0000000000000000", 0.0)
                .is_err()
        );
    }

    #[test]
    fn merge_extra_response_none_passes_through() {
        let result = Ok(serde_json::json!({"image": "abc", "width": 10, "height": 20}));
        let merged = merge_extra_response(result, None).unwrap();
        assert_eq!(merged["image"], "abc");
        assert_eq!(merged["width"], 10);
    }

    #[test]
    fn merge_extra_response_ok_merges_fields() {
        let result = Ok(serde_json::json!({
            "image": "abc",
            "width": 10,
            "height": 20,
            "frame_id": "f_0001",
            "retained": true,
        }));
        let extra = Some(serde_json::json!({"depth_samples": {"hit_count": 3}, "custom": true}));
        let merged = merge_extra_response(result, extra).unwrap();
        assert_eq!(merged["screenshot"], "abc");
        assert_eq!(merged["screenshot_width"], 10);
        assert_eq!(merged["screenshot_height"], 20);
        assert_eq!(merged["frame_id"], "f_0001");
        assert_eq!(merged["retained"], true);
        assert!(merged.get("retention_reason").is_none());
        assert_eq!(merged["depth_samples"]["hit_count"], 3);
        assert_eq!(merged["custom"], true);
    }

    #[test]
    fn merge_extra_response_err_returns_extra_with_null_screenshot() {
        let result = Err(ControlError::not_found("fail"));
        let extra = Some(serde_json::json!({"reload": "ok", "entity_count": 42}));
        let merged = merge_extra_response(result, extra).unwrap();
        assert!(merged["screenshot"].is_null());
        assert_eq!(merged["screenshot_error"], "fail");
        assert_eq!(merged["reload"], "ok");
        assert_eq!(merged["entity_count"], 42);
    }

    #[test]
    fn merge_extra_response_ok_has_no_screenshot_error() {
        let result = Ok(serde_json::json!({"image": "abc", "width": 10, "height": 20}));
        let merged = merge_extra_response(result, Some(serde_json::json!({}))).unwrap();
        assert!(merged.get("screenshot_error").is_none());
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
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                debug_cleanup: None,
                schedule: VecDeque::from([0]), // ready immediately
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
                hide_ui: true,
                with_gizmos: false,
                ui_restore: None,
                gizmo_restore: None,
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
                response_tx: Some(tx),
                max_width: None,
                columns: 3,
                debug_cleanup: None,
                schedule: VecDeque::from([0, 10, 10]), // first ready, then 2 more
                total_captures: 3,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
                hide_ui: true,
                with_gizmos: false,
                ui_restore: None,
                gizmo_restore: None,
                headless_sequence: None,
                stall_frames: 0,
            },
        );
        timelines.next_id = 1;
        world.insert_resource(timelines);

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
                response_tx: Some(tx),
                max_width: None,
                columns: 1,
                debug_cleanup: None,
                schedule: VecDeque::from([0]),
                total_captures: 1,
                next_capture_index: 0,
                collected: vec![],
                overlay_suppressed: false,
                hide_ui: false,
                with_gizmos: true,
                ui_restore: None,
                gizmo_restore: None,
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

        let mut frame = world.resource_mut::<HeadlessFrameBuffer>();
        frame.latest = Some((vec![0, 255, 0, 255], 1, 1));
        frame.sequence = 6;
        drop(frame);

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

    #[test]
    fn headless_frame_buffer_default_is_none() {
        let buffer = HeadlessFrameBuffer::default();
        assert!(buffer.latest.is_none());
    }

    #[test]
    fn encode_rgb_screenshot_produces_valid_png() {
        let mut rgb = RgbImage::new(4, 4);
        for x in 0..4 {
            for y in 0..4 {
                rgb.put_pixel(x, y, Rgb([100, 150, 200]));
            }
        }
        let result = encode_rgb_screenshot(&rgb).unwrap();
        assert_eq!(result["format"], "png");
        assert_eq!(result["width"], 4);
        assert_eq!(result["height"], 4);
        assert!(result["image"].as_str().unwrap().len() > 10);
    }

    #[test]
    fn encode_rgb_screenshot_resizes() {
        let rgb = RgbImage::new(100, 50);
        let resized = resize_rgb_image_linear(rgb, Some(20));
        let result = encode_rgb_screenshot(&resized).unwrap();
        assert_eq!(result["width"], 20);
        assert_eq!(result["height"], 10);
    }

    #[test]
    fn capture_headless_frame_no_buffer() {
        let mut world = World::new();
        let result = capture_headless_frame(&mut world, None, CaptureResponseKind::Screenshot);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("HeadlessFrameBuffer"));
    }

    #[test]
    fn capture_headless_frame_no_frame_available() {
        let mut world = World::new();
        world.insert_resource(HeadlessFrameBuffer::default());
        let result = capture_headless_frame(&mut world, None, CaptureResponseKind::Screenshot);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no frame"));
    }

    #[test]
    fn capture_headless_frame_success() {
        let mut world = World::new();
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((rgba, 2, 2)),
            sequence: 1,
        });
        let result =
            capture_headless_frame(&mut world, None, CaptureResponseKind::Screenshot).unwrap();
        assert_eq!(result["width"], 2);
        assert_eq!(result["height"], 2);
        assert_eq!(result["format"], "png");
        assert!(result["frame_id"].as_str().is_some());
        assert_eq!(result["retained"], true);
    }

    #[test]
    fn capture_headless_stats_returns_numbers_without_png_and_retains_frame() {
        let mut world = World::new();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255, 0, 255, 0, 255], 2, 1)),
            sequence: 1,
        });
        let result = capture_headless_frame(
            &mut world,
            None,
            CaptureResponseKind::Stats(FrameStatsOptions {
                grid: 1,
                region: None,
                sample_points: Some(vec![[0, 0]]),
            }),
        )
        .unwrap();

        assert_eq!(result["width"], 2);
        assert_eq!(
            result["samples"][0]["rgb"],
            serde_json::json!([1.0, 0.0, 0.0])
        );
        assert!(result.get("image").is_none());
        assert!(result["frame_id"].as_str().is_some());
        assert_eq!(result["retained"], true);
    }

    #[test]
    fn read_headless_frame_converts_rgba_to_rgb() {
        let mut world = World::new();
        // 1x1 pixel: R=100, G=200, B=50, A=255
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![100, 200, 50, 255], 1, 1)),
            sequence: 1,
        });
        let rgb = read_headless_frame(&world).unwrap();
        assert_eq!(rgb.width(), 1);
        assert_eq!(rgb.height(), 1);
        let pixel = rgb.get_pixel(0, 0);
        assert_eq!(pixel.0, [100, 200, 50]);
    }
}
