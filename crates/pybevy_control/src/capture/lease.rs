use bevy::ecs::world::World;

use super::{
    isolation::EntityCaptureIsolationActive,
    screenshot::{PendingScreenshotResponders, StagedScreenshots},
    timeline::{ActiveTimeline, PendingTimelines, TimelineCaptures},
};

/// Captures start one at a time so each snapshots authored state, not another capture's.
pub(crate) fn capture_in_progress(world: &World) -> bool {
    world.contains_resource::<EntityCaptureIsolationActive>()
        || world
            .get_resource::<PendingScreenshotResponders>()
            .is_some_and(|responders| !responders.map.is_empty())
        || world
            .get_resource::<StagedScreenshots>()
            .is_some_and(|staged| !staged.pending.is_empty())
        || world
            .get_resource::<TimelineCaptures>()
            .is_some_and(|captures| !captures.map.is_empty())
        || world
            .get_resource::<PendingTimelines>()
            .is_some_and(|timelines| timelines.active.values().any(ActiveTimeline::is_started))
        || world
            .get_resource::<super::turnaround::PendingTurnarounds>()
            .is_some_and(|turnarounds| {
                turnarounds
                    .active
                    .iter()
                    .any(super::turnaround::ActiveTurnaround::is_started)
            })
}
