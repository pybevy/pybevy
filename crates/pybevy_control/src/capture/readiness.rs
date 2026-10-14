use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use bevy::{prelude::*, render::render_resource::PipelineCache};

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

    /// Marks every epoch up to `extracted` complete. Requests made after that
    /// extraction stay pending.
    pub(super) fn complete_extracted_frame(&self, extracted: u64) {
        self.shared
            .completed_epoch
            .fetch_max(extracted, Ordering::AcqRel);
    }

    pub(super) fn is_complete(&self, epoch: u64) -> bool {
        self.shared.completed_epoch.load(Ordering::Acquire) >= epoch
    }
}

/// The requested epoch as of the render world's last extraction. With
/// pipelined rendering the main world requests the next epoch while this
/// frame still renders, so completion must not read the live request count.
#[derive(Resource, Default)]
pub struct ExtractedRenderEpoch(u64);

pub fn extract_render_frame_epoch(
    readiness: Res<RenderFrameReadiness>,
    mut extracted: ResMut<ExtractedRenderEpoch>,
) {
    extracted.0 = readiness.requested_epoch();
}

/// Publish completion only after the render pipeline queue is empty at the
/// end of a render-world frame.
pub fn update_render_frame_readiness(
    pipeline_cache: Res<PipelineCache>,
    readiness: Res<RenderFrameReadiness>,
    extracted: Res<ExtractedRenderEpoch>,
) {
    if pipeline_cache.waiting_pipelines().next().is_none() {
        readiness.complete_extracted_frame(extracted.0);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    fn extract(world: &mut World) -> u64 {
        world.run_system_once(extract_render_frame_epoch).unwrap();
        world.resource::<ExtractedRenderEpoch>().0
    }

    #[test]
    fn a_frame_completes_only_the_requests_it_extracted() {
        let readiness = RenderFrameReadiness::default();
        let mut render_world = World::new();
        render_world.insert_resource(readiness.clone());
        render_world.init_resource::<ExtractedRenderEpoch>();

        let extracted_edit = readiness.request_frame();
        let previous_frame = extract(&mut render_world);
        // Pipelined rendering: the next main-world frame requests an epoch
        // while the previous extraction is still rendering.
        let concurrent_edit = readiness.request_frame();
        readiness.complete_extracted_frame(previous_frame);

        assert!(readiness.is_complete(extracted_edit));
        assert!(!readiness.is_complete(concurrent_edit));

        let next_frame = extract(&mut render_world);
        readiness.complete_extracted_frame(next_frame);
        assert!(readiness.is_complete(concurrent_edit));
    }

    #[test]
    fn an_older_extraction_never_rolls_completion_back() {
        let readiness = RenderFrameReadiness::default();
        let first = readiness.request_frame();
        let second = readiness.request_frame();
        readiness.complete_extracted_frame(second);
        readiness.complete_extracted_frame(first);
        assert!(readiness.is_complete(second));
    }
}
