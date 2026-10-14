use bevy::prelude::Resource;
use tokio::sync::oneshot;

use super::{analysis::FrameStatsOptions, depth::PendingDepthAnalysis, hold::GizmoEnabledRestore};
use crate::protocol::{ControlError, EntityRef};

/// Default screenshot width when the caller doesn't pass `max_width`.
/// Keep in sync with the "(default 768)" schema docs on the capture params.
pub const DEFAULT_SCREENSHOT_MAX_WIDTH: u32 = 768;
/// Default contact-sheet width for `capture_timeline`, matching the value
/// `capture_turnaround` uses for the same composition.
pub const DEFAULT_TIMELINE_MAX_WIDTH: u32 = 1200;

/// Bevy resource for pending screenshot responses (deferred until after render)
#[derive(Resource, Default)]
pub struct PendingScreenshots {
    pub pending: Vec<PendingScreenshot>,
}

pub struct PendingScreenshot {
    pub response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
    pub frames_remaining: u32,
    /// A scheduled scene mutation may require a render-world pipeline update
    /// before its capture delay begins.
    pub required_render_epoch: Option<u64>,
    pub with_gizmos: bool,
    /// Original gizmo group states captured before Update draws for this request.
    pub gizmo_restore: GizmoEnabledRestore,
    pub max_width: Option<u32>,
    pub debug_camera: Option<DebugCameraRequest>,
    pub hide_ui: bool,
    pub entity: Option<EntityRef>,
    /// Deferred depth analysis tied to one selected camera identity.
    pub depth_analysis: Option<PendingDepthAnalysis>,
    pub response_kind: CaptureResponseKind,
    /// Extra JSON fields to merge into the screenshot response.
    /// Used by `capture_depth` and `reload_and_capture` to avoid spawning
    /// a thread for the merge.
    pub extra_response: Option<serde_json::Value>,
}

impl PendingScreenshot {
    /// A capture of the current view with no delay, UI hidden, gizmos off and
    /// the default width.
    pub fn new(
        response_tx: oneshot::Sender<Result<serde_json::Value, ControlError>>,
        response_kind: CaptureResponseKind,
    ) -> Self {
        Self {
            response_tx,
            frames_remaining: 0,
            required_render_epoch: None,
            with_gizmos: false,
            gizmo_restore: GizmoEnabledRestore::new(),
            max_width: Some(DEFAULT_SCREENSHOT_MAX_WIDTH),
            debug_camera: None,
            hide_ui: true,
            entity: None,
            depth_analysis: None,
            response_kind,
            extra_response: None,
        }
    }
}

#[derive(Debug)]
pub enum CaptureResponseKind {
    Screenshot,
    ReloadAndCapture,
    UnretainedScreenshot,
    Stats(FrameStatsOptions),
}

impl CaptureResponseKind {
    pub(crate) fn operation_name(&self) -> &'static str {
        match self {
            Self::Screenshot => "capture_screenshot",
            Self::ReloadAndCapture => "reload_and_capture",
            Self::UnretainedScreenshot => "capture_depth",
            Self::Stats(_) => "capture_stats",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DebugCameraRequest {
    pub position: [f32; 3],
    pub look_at: [f32; 3],
}

impl DebugCameraRequest {
    /// A camera pose when `position` is given, looking at the origin by default.
    pub fn from_pose(position: Option<[f32; 3]>, look_at: Option<[f32; 3]>) -> Option<Self> {
        position.map(|position| Self {
            position,
            look_at: look_at.unwrap_or([0.0, 0.0, 0.0]),
        })
    }
}
