//! Shared record of uncaptured wgpu validation errors.
//!
//! The render crate's uncaptured-error callback writes it from whichever
//! thread wgpu reports on; MCP `get_last_error` reads it and a committed hot
//! reload clears it.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use bevy::prelude::Resource;

/// One recorded validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgpuValidationEntry {
    pub message: String,
    /// Pipeline label when the error names one, otherwise the first wgpu object label.
    pub label: Option<String>,
    /// True when `label` names a render or compute pipeline.
    pub is_pipeline: bool,
}

impl WgpuValidationEntry {
    pub fn from_message(message: String) -> Self {
        let (label, is_pipeline) = match extract_label(&message) {
            Some((label, is_pipeline)) => (Some(label), is_pipeline),
            None => (None, false),
        };
        Self {
            message,
            label,
            is_pipeline,
        }
    }
}

/// Snapshot of the validation errors recorded since the last clear.
#[derive(Debug, Clone, PartialEq)]
pub struct WgpuValidationSummary {
    pub count: u64,
    pub first: WgpuValidationEntry,
    pub last: WgpuValidationEntry,
    pub last_seen_ago: Duration,
}

#[derive(Default)]
struct WgpuValidationLog {
    count: u64,
    first: Option<WgpuValidationEntry>,
    last: Option<(WgpuValidationEntry, Instant)>,
    logged: HashSet<String>,
}

/// Shared signal and summary of uncaptured wgpu validation errors.
#[derive(Clone, Default, Resource)]
pub struct WgpuValidationErrorState(Arc<Mutex<WgpuValidationLog>>);

impl WgpuValidationErrorState {
    fn lock(&self) -> MutexGuard<'_, WgpuValidationLog> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn has_error(&self) -> bool {
        self.lock().count > 0
    }

    /// Forget recorded errors and the log deduplication set.
    pub fn clear(&self) {
        *self.lock() = WgpuValidationLog::default();
    }

    /// Record one error; returns true the first time this message is seen since the last clear.
    pub fn record(&self, entry: &WgpuValidationEntry) -> bool {
        let mut log = self.lock();
        log.count = log.count.saturating_add(1);
        let is_new = !log.logged.contains(&entry.message);
        if is_new {
            log.logged.insert(entry.message.clone());
        }
        if log.first.is_none() {
            log.first = Some(entry.clone());
        }
        log.last = Some((entry.clone(), Instant::now()));
        is_new
    }

    pub fn summary(&self) -> Option<WgpuValidationSummary> {
        let log = self.lock();
        let first = log.first.clone()?;
        let (last, seen_at) = log.last.clone()?;
        Some(WgpuValidationSummary {
            count: log.count,
            first,
            last,
            last_seen_ago: seen_at.elapsed(),
        })
    }
}

/// Clear the recorded errors if the app tracks them.
pub fn clear_wgpu_validation_errors(world: &bevy::ecs::world::World) {
    if let Some(state) = world.get_resource::<WgpuValidationErrorState>() {
        state.clear();
    }
}

fn quoted_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find('\'')?;
    Some(&rest[..end]).filter(|label| !label.is_empty())
}

/// Find the pipeline label in a wgpu error, falling back to the first object label.
fn extract_label(message: &str) -> Option<(String, bool)> {
    const PIPELINE_MARKERS: [&str; 4] = [
        "create_render_pipeline, label = '",
        "create_compute_pipeline, label = '",
        "RenderPipeline with '",
        "ComputePipeline with '",
    ];
    PIPELINE_MARKERS
        .iter()
        .filter_map(|marker| {
            let start = message.find(marker)?;
            quoted_after(message, marker).map(|label| (start, label))
        })
        .min_by_key(|(start, _)| *start)
        .map(|(_, label)| (label.to_string(), true))
        .or_else(|| {
            let label =
                quoted_after(message, "label = '").or_else(|| quoted_after(message, " with '"))?;
            Some((label.to_string(), false))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIPELINE_CREATE: &str = "Validation Error\n\nCaused by:\n  In Device::create_render_pipeline, label = 'pbr_prepass_pipeline'\n    Error matching ShaderStages(FRAGMENT) shader requirements against the pipeline\n      Shader global ResourceBinding { group: 0, binding: 11 } is not available in the pipeline layout\n";

    #[test]
    fn pipeline_creation_label_is_extracted() {
        let entry = WgpuValidationEntry::from_message(PIPELINE_CREATE.to_string());
        assert_eq!(entry.label.as_deref(), Some("pbr_prepass_pipeline"));
        assert!(entry.is_pipeline);
    }

    #[test]
    fn invalid_pipeline_use_label_is_extracted() {
        let message = "Validation Error\n\nCaused by:\n  In a CommandEncoder, label = 'main_opaque_pass_3d_command_encoder'\n    In a set_pipeline command\n      RenderPipeline with 'pbr_prepass_pipeline' label is invalid\n";
        let entry = WgpuValidationEntry::from_message(message.to_string());
        assert_eq!(entry.label.as_deref(), Some("pbr_prepass_pipeline"));
        assert!(entry.is_pipeline);
    }

    #[test]
    fn non_pipeline_label_falls_back_to_object_label() {
        let message = "Validation Error\n\nCaused by:\n  In Device::create_bind_group, label = 'downsample depth bind group'\n    Usage flags are missing\n";
        let entry = WgpuValidationEntry::from_message(message.to_string());
        assert_eq!(entry.label.as_deref(), Some("downsample depth bind group"));
        assert!(!entry.is_pipeline);
    }

    #[test]
    fn unlabeled_error_has_no_label() {
        let entry = WgpuValidationEntry::from_message("Validation Error".to_string());
        assert_eq!(entry.label, None);
        assert!(!entry.is_pipeline);
    }

    #[test]
    fn record_counts_every_error_and_keeps_first_and_last() {
        let state = WgpuValidationErrorState::default();
        assert!(!state.has_error());
        assert!(state.summary().is_none());

        assert!(state.record(&WgpuValidationEntry::from_message("a".into())));
        assert!(state.record(&WgpuValidationEntry::from_message("b".into())));
        assert!(!state.record(&WgpuValidationEntry::from_message("b".into())));

        let summary = state.summary().unwrap();
        assert_eq!(summary.count, 3);
        assert_eq!(summary.first.message, "a");
        assert_eq!(summary.last.message, "b");
        assert!(state.has_error());
    }

    #[test]
    fn clear_forgets_errors_and_log_deduplication() {
        let state = WgpuValidationErrorState::default();
        state.record(&WgpuValidationEntry::from_message("dup".into()));
        state.clear();
        assert!(!state.has_error());
        assert!(state.summary().is_none());
        assert!(state.record(&WgpuValidationEntry::from_message("dup".into())));
        assert_eq!(state.summary().unwrap().count, 1);
    }
}
