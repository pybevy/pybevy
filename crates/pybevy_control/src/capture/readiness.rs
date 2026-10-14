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

    pub(super) fn complete_requested_frame(&self) {
        let requested = self.requested_epoch();
        self.shared
            .completed_epoch
            .store(requested, Ordering::Release);
    }

    pub(super) fn is_complete(&self, epoch: u64) -> bool {
        self.shared.completed_epoch.load(Ordering::Acquire) >= epoch
    }
}

/// Publish completion only after the render pipeline queue is empty at the
/// end of a render-world frame.
pub fn update_render_frame_readiness(
    pipeline_cache: Res<PipelineCache>,
    readiness: Res<RenderFrameReadiness>,
) {
    if pipeline_cache.waiting_pipelines().next().is_none() {
        readiness.complete_requested_frame();
    }
}
