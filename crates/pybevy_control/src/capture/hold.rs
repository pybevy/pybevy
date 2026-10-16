use std::{any::TypeId, collections::HashSet};

use bevy::{ecs::world::World, gizmos::config::GizmoConfigStore, prelude::*};

use crate::capture::queue::PendingScreenshot;

/// Marker component for internal overlay UI that should always be hidden in screenshots.
/// Applied to hot reload overlay entities so the screenshot handler can find and hide them.
#[derive(Component)]
pub struct InternalOverlayUi;

/// Refcount of in-flight captures (screenshots, timelines, turnarounds) that
/// want the internal overlay off-screen. Captures only increment/decrement;
/// the overlay's render system drives `Visibility` from this every frame, so
/// restoration is automatic once the count returns to zero (and survives the
/// overlay entities being respawned mid-capture by a Full hot reload).
#[derive(Resource, Default)]
pub struct OverlaySuppression(pub u32);

/// Original `enabled` state of every gizmo config group a capture turned off.
pub type GizmoEnabledRestore = Vec<(TypeId, bool)>;

/// Set gizmo visibility for every registered gizmo config group, returning the
/// original enabled state of each group that changed. Component-driven markers
/// (`ShowAabbGizmo`, `ShowLightGizmo`) draw through their own groups, so hiding
/// only the default group would leave them in the capture.
pub(super) fn set_gizmos_enabled(world: &mut World, enabled: bool) -> GizmoEnabledRestore {
    let mut restore = GizmoEnabledRestore::new();
    let Some(mut store) = world.get_resource_mut::<GizmoConfigStore>() else {
        return restore;
    };
    for (group, config, _) in store.iter_mut() {
        if config.enabled == enabled {
            continue;
        }
        restore.push((*group, config.enabled));
        config.enabled = enabled;
    }
    restore
}

/// Replay the group states recorded by `set_gizmos_enabled`.
pub(super) fn restore_gizmo_configs(store: &mut GizmoConfigStore, restore: GizmoEnabledRestore) {
    for (group, was_enabled) in restore {
        if let Some((config, _)) = store.get_config_mut_dyn(&group) {
            config.enabled = was_enabled;
        }
    }
}

pub(super) fn restore_gizmos_enabled(world: &mut World, restore: GizmoEnabledRestore) {
    if restore.is_empty() {
        return;
    }
    if let Some(mut store) = world.get_resource_mut::<GizmoConfigStore>() {
        restore_gizmo_configs(&mut store, restore);
    }
}

/// Apply screenshot gizmo visibility before Python Update systems draw this frame.
pub(crate) fn prepare_pending_screenshot_gizmos(
    world: &mut World,
    screenshot: &mut PendingScreenshot,
) {
    if !screenshot.with_gizmos && screenshot.gizmo_restore.is_empty() {
        screenshot.gizmo_restore = set_gizmos_enabled(world, false);
    }
}

/// Hide authored UI Node entities by setting their visibility to Hidden.
/// Returns a list of (entity, original_visibility) for restoration. Internal
/// overlay entities are excluded: they are owned by `OverlaySuppression`.
pub(super) fn hide_ui_nodes(
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
) -> (Option<Vec<(Entity, Visibility)>>, GizmoEnabledRestore) {
    suppress_internal_overlay(world);
    let ui_restore = hide_ui.then(|| hide_ui_nodes(world, None));
    let gizmo_restore = if with_gizmos {
        GizmoEnabledRestore::new()
    } else {
        set_gizmos_enabled(world, false)
    };
    (ui_restore, gizmo_restore)
}

/// Release one `suppress_internal_overlay` hold.
pub(crate) fn release_internal_overlay(world: &mut World) {
    if let Some(mut suppression) = world.get_resource_mut::<OverlaySuppression>() {
        suppression.0 = suppression.0.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use std::{any::TypeId, collections::VecDeque};

    use bevy::{
        gizmos::config::DefaultGizmoConfigGroup, light::gizmos::LightGizmoConfigGroup,
        render::view::window::screenshot::Screenshot, ui::Node,
    };
    use tokio::sync::oneshot;

    use super::*;
    use crate::capture::{
        hold::{InternalOverlayUi, OverlaySuppression},
        lease::capture_in_progress,
        queue::{CaptureResponseKind, PendingScreenshots},
        screenshot::{
            MAX_CAPTURE_WAIT_FRAMES, PendingScreenshotResponders, ScreenshotResponder,
            process_pending_screenshots,
        },
        timeline::{ActiveTimeline, PendingTimelines, TimelineCaptures, process_pending_timelines},
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

    #[test]
    fn set_gizmos_enabled_disables_when_enabled() {
        let mut world = world_with_gizmos();
        assert!(gizmos_enabled(&world));

        let restore = set_gizmos_enabled(&mut world, false);
        assert!(!gizmos_enabled(&world));
        assert_eq!(restore, default_group_restore(true));
    }

    #[test]
    fn set_gizmos_enabled_noop_when_already_matches() {
        let mut world = world_with_gizmos();
        // Gizmos default to enabled; setting true should be a no-op
        let restore = set_gizmos_enabled(&mut world, true);
        assert!(gizmos_enabled(&world));
        assert!(restore.is_empty());
    }

    #[test]
    fn set_gizmos_enabled_returns_nothing_without_resource() {
        let mut world = World::new();
        let restore = set_gizmos_enabled(&mut world, false);
        assert!(restore.is_empty());
    }

    #[test]
    fn set_gizmos_enabled_roundtrip_restores_state() {
        let mut world = world_with_gizmos();
        assert!(gizmos_enabled(&world));

        // Disable gizmos
        let restore = set_gizmos_enabled(&mut world, false);
        assert!(!gizmos_enabled(&world));

        // Restore original state
        restore_gizmos_enabled(&mut world, restore);
        assert!(gizmos_enabled(&world));
    }

    #[test]
    fn set_gizmos_enabled_double_disable_second_is_noop() {
        let mut world = world_with_gizmos();

        let first = set_gizmos_enabled(&mut world, false);
        assert_eq!(first, default_group_restore(true));

        let second = set_gizmos_enabled(&mut world, false);
        assert!(second.is_empty());
    }

    #[test]
    fn set_gizmos_enabled_covers_every_registered_group() {
        let mut world = world_with_gizmos();
        world
            .resource_mut::<GizmoConfigStore>()
            .insert(GizmoConfig::default(), LightGizmoConfigGroup::default());

        let restore = set_gizmos_enabled(&mut world, false);

        {
            let store = world.resource::<GizmoConfigStore>();
            assert!(!store.config::<DefaultGizmoConfigGroup>().0.enabled);
            assert!(!store.config::<LightGizmoConfigGroup>().0.enabled);
        }
        assert_eq!(restore.len(), 2);

        restore_gizmos_enabled(&mut world, restore);

        let store = world.resource::<GizmoConfigStore>();
        assert!(store.config::<DefaultGizmoConfigGroup>().0.enabled);
        assert!(store.config::<LightGizmoConfigGroup>().0.enabled);
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
                gizmo_restore: default_group_restore(true),
                extra_response: None,
                capture_viewport: None,
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
            !capture_in_progress(&world),
            "expired responder must unwedge entity-isolated captures"
        );
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert!(gizmos_enabled(&world));
        assert!(world.get_entity(screenshot_entity).is_err());
    }

    #[test]
    fn abandoned_responder_releases_capture_state() {
        let mut world = world_with_gizmos();
        world.insert_resource(OverlaySuppression(1));
        let ui = world.spawn((Visibility::Hidden, Node::default())).id();
        let screenshot_entity = world.spawn_empty().id();
        let (tx, rx) = oneshot::channel();
        let mut responders = PendingScreenshotResponders::default();
        responders.map.insert(
            screenshot_entity,
            ScreenshotResponder {
                response_tx: tx,
                max_width: None,
                debug_cleanup: None,
                ui_restore: Some(vec![(ui, Visibility::Visible)]),
                entity_isolation: None,
                gizmo_restore: default_group_restore(true),
                extra_response: None,
                capture_viewport: None,
                response_kind: CaptureResponseKind::Screenshot,
                frames_waited: 0,
            },
        );
        world.insert_resource(responders);
        world.insert_resource(PendingScreenshots::default());
        set_gizmos_enabled(&mut world, false);
        drop(rx);

        process_pending_screenshots(&mut world);

        assert!(!capture_in_progress(&world));
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert!(gizmos_enabled(&world));
        assert!(world.get_entity(screenshot_entity).is_err());
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
                debug_camera: None,
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
                gizmo_restore: default_group_restore(true),
                headless_sequence: None,
                stall_frames: MAX_CAPTURE_WAIT_FRAMES,
            },
        );
        world.insert_resource(timelines);
        let mut captures = TimelineCaptures::default();
        let stale = world.spawn(Screenshot::primary_window()).id();
        captures.map.insert(stale, (3, 0));
        world.insert_resource(captures);
        set_gizmos_enabled(&mut world, false);

        process_pending_timelines(&mut world);

        let error = rx.try_recv().unwrap().unwrap_err();
        assert!(error.message.contains("deadline"));
        assert!(world.resource::<PendingTimelines>().active.is_empty());
        assert!(world.resource::<TimelineCaptures>().map.is_empty());
        assert!(world.get_entity(stale).is_err());
        assert_eq!(world.resource::<OverlaySuppression>().0, 0);
        assert_eq!(*world.get::<Visibility>(ui).unwrap(), Visibility::Visible);
        assert!(gizmos_enabled(&world));
    }
}
