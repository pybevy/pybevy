use bevy::{ecs::world::World, prelude::Resource};
use tokio::sync::mpsc;

use super::{
    channel::{ControlReceiver, ControlRequest},
    deferred::{
        handle_submit_schedule, push_pending_depth, push_pending_reload,
        push_pending_reload_and_capture, push_pending_screenshot, push_pending_stats,
        push_pending_timeline, push_pending_turnaround,
    },
    runtime::ControlRuntime,
};
use crate::{
    capture::{
        queue::{PendingScreenshot, PendingScreenshots},
        screenshot::prepare_pending_screenshot_gizmos,
    },
    catalog::feature_gate::authorize,
    plugin::ControlConfig,
    protocol::{ControlOperation, SseEvent},
    transport::sse::SseEventBroadcaster,
};

/// Maximum requests processed per frame to prevent frame spikes
const MAX_REQUESTS_PER_FRAME: usize = 32;

/// Tracks the timestamp of the error we last broadcast via SSE, so each
/// drained error is pushed to clients exactly once. Compared by equality,
/// not ordering: a full reload resets the clock, so a genuinely new
/// post-reload error can carry a smaller timestamp than the previous one.
#[derive(Resource, Default)]
pub struct LastBroadcastedErrorTimestamp {
    pub timestamp_secs: Option<f64>,
}

/// Exclusive Bevy system that drains the control request channel.
/// Runs in First schedule, before Python systems.
pub fn control_poll_system(world: &mut World) {
    // Take the receiver out of the resource temporarily
    let Some(mut receiver) = world.remove_resource::<ControlReceiver>() else {
        return;
    };

    let mut processed = 0;
    let mut deferred_screenshots: Vec<PendingScreenshot> = Vec::new();
    let config = world.get_resource::<ControlConfig>().cloned();

    // Two-phase processing: drain all requests first (no GIL needed), then
    // execute sync handlers in one batched Python::attach. This prevents both
    // rapid GIL acquire/release cycling (which corrupts GIL state after
    // py.detach()) AND holding the GIL for deferred ops that don't need it
    // (which increases contention with the file watcher thread and can
    // contribute to deadlocks during hot reload).

    // Phase 1: Drain channel and classify requests (no GIL needed)
    let mut sync_requests: Vec<ControlRequest> = Vec::new();

    while processed < MAX_REQUESTS_PER_FRAME {
        match receiver.rx.try_recv() {
            Ok(request) => {
                processed += 1;

                if let Err(error) = authorize(&request.operation, config.as_ref()) {
                    let _ = request.response_tx.send(Err(error));
                    continue;
                }

                // Handle ScheduleActions (no GIL needed - just stores in World resource)
                if let ControlOperation::ScheduleActions(sched) = request.operation {
                    handle_submit_schedule(sched, request.response_tx, world);
                    continue;
                }

                // Classify: deferred ops go directly to their queues,
                // sync ops are collected for batched GIL processing
                match request.operation {
                    ControlOperation::CaptureScreenshot(p) => {
                        let with_gizmos = p.gizmos;
                        push_pending_screenshot(
                            p,
                            with_gizmos,
                            request.response_tx,
                            &mut deferred_screenshots,
                        );
                    }
                    ControlOperation::CaptureStats(p) => {
                        push_pending_stats(p, request.response_tx, &mut deferred_screenshots);
                    }
                    ControlOperation::CaptureWithGizmos(p) => {
                        push_pending_screenshot(
                            p,
                            true,
                            request.response_tx,
                            &mut deferred_screenshots,
                        );
                    }
                    ControlOperation::CaptureTimeline(p) => {
                        push_pending_timeline(p, request.response_tx, world);
                    }
                    ControlOperation::Reload(p) => {
                        push_pending_reload(p, request.response_tx, world);
                    }
                    ControlOperation::ReloadAndCapture(p) => {
                        push_pending_reload_and_capture(p, request.response_tx, world);
                    }
                    ControlOperation::CaptureTurnaround(p) => {
                        push_pending_turnaround(p, request.response_tx, world);
                    }
                    ControlOperation::CaptureDepth(p) => {
                        push_pending_depth(
                            p,
                            request.response_tx,
                            &mut deferred_screenshots,
                            world,
                        );
                    }
                    other => {
                        sync_requests.push(ControlRequest {
                            operation: other,
                            response_tx: request.response_tx,
                        });
                    }
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }

    // Phase 2: Dispatch sync requests through the runtime.
    // The runtime impl batches GIL acquisition (PyO3) or enters interpreter
    // scope as appropriate for the backend.
    if !sync_requests.is_empty() {
        let mut runtime = world
            .remove_non_send::<Box<dyn ControlRuntime>>()
            .expect("ControlRuntime resource missing");

        runtime.dispatch_batch(world, sync_requests);

        world.insert_non_send(runtime);
    }

    // Store deferred screenshots
    if !deferred_screenshots.is_empty() {
        for screenshot in &mut deferred_screenshots {
            prepare_pending_screenshot_gizmos(world, screenshot);
        }
        let mut pending = world.get_resource_or_insert_with(PendingScreenshots::default);
        pending.pending.extend(deferred_screenshots);
    }

    // Put the receiver back
    world.insert_resource(receiver);

    // Broadcast new system errors via SSE
    if let Some(last_error) = world.get_resource::<pybevy_core::LastSystemError>()
        && let Some(ref msg) = last_error.error
    {
        let error_ts = last_error.timestamp_secs;
        let msg = msg.clone();
        let traceback = last_error.traceback.clone();

        let mut tracker = world.get_resource_or_insert_with(LastBroadcastedErrorTimestamp::default);

        if tracker.timestamp_secs != Some(error_ts) {
            tracker.timestamp_secs = Some(error_ts);

            // SSE broadcast (for clients that subscribe to /api/v1/sse)
            if let Some(broadcaster) = world.get_resource::<SseEventBroadcaster>() {
                broadcaster.send(&SseEvent::Error {
                    message: msg,
                    traceback,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{any::TypeId, sync::Once};

    use bevy::{
        ecs::entity::Entity,
        gizmos::config::{DefaultGizmoConfigGroup, GizmoConfig, GizmoConfigStore},
        prelude::Transform,
    };
    use pybevy_core::bridge_inventory::collect_all;
    use pyo3::Python;
    use tokio::sync::oneshot;

    use super::*;
    use crate::{
        engine::channel::create_channel,
        protocol::{EntityRef, GetComponentParams},
        pyo3::runtime::Pyo3ControlRuntime,
    };

    fn all_features_enabled() -> ControlConfig {
        ControlConfig {
            port: 0,
            host: "127.0.0.1".into(),
            screenshot: true,
            manipulation: true,
            execute_python: true,
            api_discovery: true,
        }
    }
    #[test]
    fn control_poll_suppresses_gizmos_before_queuing_capture() {
        let mut world = World::new();
        world.insert_resource(all_features_enabled());
        let mut store = GizmoConfigStore::default();
        store.insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
        world.insert_resource(store);

        let (sender, receiver) = create_channel();
        world.insert_resource(receiver);
        let (response_tx, _response_rx) = oneshot::channel();
        sender
            .tx
            .send(ControlRequest {
                operation: ControlOperation::CaptureScreenshot(serde_json::from_str("{}").unwrap()),
                response_tx,
            })
            .unwrap();

        control_poll_system(&mut world);

        let store = world.resource::<GizmoConfigStore>();
        let (config, _) = store.config::<DefaultGizmoConfigGroup>();
        assert!(!config.enabled);
        let pending = world.resource::<PendingScreenshots>();
        assert_eq!(
            pending.pending[0].gizmo_restore,
            vec![(TypeId::of::<DefaultGizmoConfigGroup>(), true)]
        );
    }

    #[test]
    fn capture_screenshot_gizmos_flag_survives_direct_dispatch() {
        let mut world = World::new();
        world.insert_resource(all_features_enabled());
        let mut store = GizmoConfigStore::default();
        store.insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
        world.insert_resource(store);

        let (sender, receiver) = create_channel();
        world.insert_resource(receiver);
        let (response_tx, _response_rx) = oneshot::channel();
        sender
            .tx
            .send(ControlRequest {
                operation: ControlOperation::CaptureScreenshot(
                    serde_json::from_str(r#"{"gizmos": true}"#).unwrap(),
                ),
                response_tx,
            })
            .unwrap();

        control_poll_system(&mut world);

        let pending = world.resource::<PendingScreenshots>();
        assert!(pending.pending[0].with_gizmos);
        let store = world.resource::<GizmoConfigStore>();
        let (config, _) = store.config::<DefaultGizmoConfigGroup>();
        assert!(config.enabled, "gizmos must stay enabled for the capture");
    }
    #[test]
    fn max_requests_per_frame_is_reasonable() {
        const _: () = assert!((MAX_REQUESTS_PER_FRAME > 0) && (MAX_REQUESTS_PER_FRAME <= 1000));
    }
    /// Regression test: multiple get_component requests processed in a single
    /// control_poll_system frame must not crash.
    ///
    /// Without the Python::attach() wrapper around the dispatch loop, rapid
    /// GIL acquire/release cycling after py.detach() can corrupt GIL state
    /// and cause a silent segfault (the original bug).
    #[test]
    fn parallel_get_component_does_not_crash() {
        // Force linker to include pybevy_transform (its inventory entries register Transform bridge)
        extern crate pybevy_transform;

        static INIT: Once = Once::new();
        INIT.call_once(|| {
            Python::initialize();
            collect_all();
        });

        Python::attach(|py| {
            // Release the GIL to simulate the production environment where
            // app.run() calls py.detach() before entering the Bevy main loop.
            py.detach(|| {
                let mut world = World::new();
                let (sender, receiver) = create_channel();
                world.insert_resource(receiver);

                // Insert the runtime resource
                world.insert_non_send(Box::new(Pyo3ControlRuntime) as Box<dyn ControlRuntime>);

                // Spawn 6 entities with Transform
                let entities: Vec<Entity> = (0..6)
                    .map(|i| world.spawn(Transform::from_xyz(i as f32, 0.0, 0.0)).id())
                    .collect();

                // Queue 6 get_component requests (all in one batch, like parallel HTTP)
                let mut response_rxs = Vec::new();
                for entity in &entities {
                    let (tx, rx) = oneshot::channel();
                    sender
                        .tx
                        .send(ControlRequest {
                            operation: ControlOperation::GetComponent(GetComponentParams {
                                entity: EntityRef::Id(entity.to_bits()),
                                component: "Transform".into(),
                            }),
                            response_tx: tx,
                        })
                        .unwrap();
                    response_rxs.push(rx);
                }

                // Process all 6 in a single control_poll_system call
                control_poll_system(&mut world);

                // Verify all 6 responses arrived successfully
                for (i, rx) in response_rxs.into_iter().enumerate() {
                    let result = rx
                        .blocking_recv()
                        .unwrap_or_else(|_| panic!("Response {i} channel closed"));
                    let value =
                        result.unwrap_or_else(|e| panic!("Response {i} error: {}", e.message));
                    assert_eq!(
                        value["component"], "Transform",
                        "Response {i}: expected Transform component"
                    );
                    assert!(
                        value["fields"].is_object(),
                        "Response {i}: expected fields object"
                    );
                }
            });
        });
    }
}
