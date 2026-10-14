use std::sync::{Arc, Mutex};

use bevy::prelude::Resource;
use serde::Serialize;

use super::runner::ActionResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleStatus {
    Running,
    Completed,
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedScheduleState {
    pub schedule_id: String,
    pub status: ScheduleStatus,
    pub total_actions: usize,
    /// Number of actions with a terminal result, including actions that did not run.
    pub completed_actions: usize,
    /// Number of actions that were dispatched, including partial and error results.
    pub executed_actions: usize,
    pub skipped_actions: usize,
    pub aborted_actions: usize,
    pub cancelled_actions: usize,
    pub results: Vec<ActionResult>,
    pub cancelled: bool,
    #[serde(skip)]
    pub completed_at: Option<std::time::Instant>,
}

impl SharedScheduleState {
    pub fn new(schedule_id: &str, total: usize) -> Self {
        Self {
            schedule_id: schedule_id.to_string(),
            status: ScheduleStatus::Running,
            total_actions: total,
            completed_actions: 0,
            executed_actions: 0,
            skipped_actions: 0,
            aborted_actions: 0,
            cancelled_actions: 0,
            results: Vec::new(),
            cancelled: false,
            completed_at: None,
        }
    }
}

/// How long a completed schedule stays pollable before eviction.
pub const COMPLETED_SCHEDULE_RETENTION: std::time::Duration = std::time::Duration::from_secs(300);
/// Completed schedules kept regardless of age; oldest beyond this are evicted.
pub const MAX_COMPLETED_SCHEDULES: usize = 32;

#[derive(Clone, Default)]
pub struct SharedScheduleRegistry {
    inner: Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<SharedScheduleState>>>>>,
}

impl SharedScheduleRegistry {
    pub fn get(&self, id: &str) -> Option<SharedScheduleState> {
        let guard = self.inner.lock().ok()?;
        let arc = guard.get(id)?;
        arc.lock().ok().map(|g| g.clone())
    }

    pub fn insert(&self, id: String, state: Arc<Mutex<SharedScheduleState>>) {
        if let Ok(mut guard) = self.inner.lock() {
            Self::evict_completed(&mut guard);
            guard.insert(id, state);
        }
    }

    fn evict_completed(
        entries: &mut std::collections::HashMap<String, Arc<Mutex<SharedScheduleState>>>,
    ) {
        let now = std::time::Instant::now();
        let mut completed: Vec<(String, std::time::Instant)> = entries
            .iter()
            .filter_map(|(id, state)| {
                let completed_at = state.lock().ok()?.completed_at?;
                Some((id.clone(), completed_at))
            })
            .collect();

        completed.retain(|(id, completed_at)| {
            if now.duration_since(*completed_at) > COMPLETED_SCHEDULE_RETENTION {
                entries.remove(id);
                false
            } else {
                true
            }
        });

        if completed.len() > MAX_COMPLETED_SCHEDULES {
            completed.sort_by_key(|(_, completed_at)| *completed_at);
            for (id, _) in &completed[..completed.len() - MAX_COMPLETED_SCHEDULES] {
                entries.remove(id);
            }
        }
    }

    pub fn cancel(&self, id: &str) -> bool {
        if let Ok(guard) = self.inner.lock()
            && let Some(arc) = guard.get(id)
            && let Ok(mut state) = arc.lock()
        {
            state.cancelled = true;
            return true;
        }
        false
    }
}

#[derive(Resource, Clone)]
pub struct SharedScheduleRegistryResource(pub SharedScheduleRegistry);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::runner::ActionStatus;

    fn completed_state(
        id: &str,
        completed_at: std::time::Instant,
    ) -> Arc<Mutex<SharedScheduleState>> {
        let mut state = SharedScheduleState::new(id, 1);
        state.status = ScheduleStatus::Completed;
        state.completed_at = Some(completed_at);
        Arc::new(Mutex::new(state))
    }

    #[test]
    fn test_shared_schedule_registry() {
        let registry = SharedScheduleRegistry::default();
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-1", 3)));
        registry.insert("s-1".to_string(), shared.clone());

        let state = registry.get("s-1").unwrap();
        assert_eq!(state.schedule_id, "s-1");
        assert_eq!(state.status, ScheduleStatus::Running);
        assert_eq!(state.total_actions, 3);

        // Update via the Arc directly (same as schedule state machine does)
        shared.lock().unwrap().completed_actions = 2;
        let state = registry.get("s-1").unwrap();
        assert_eq!(state.completed_actions, 2);

        assert!(registry.cancel("s-1"));
        let state = registry.get("s-1").unwrap();
        assert!(state.cancelled);

        assert!(!registry.cancel("nonexistent"));
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn registry_evicts_expired_completed_schedules_on_insert() {
        let registry = SharedScheduleRegistry::default();
        let expired = std::time::Instant::now() - COMPLETED_SCHEDULE_RETENTION * 2;
        registry.insert("old".to_string(), completed_state("old", expired));
        registry.insert(
            "fresh".to_string(),
            completed_state("fresh", std::time::Instant::now()),
        );
        registry.insert(
            "running".to_string(),
            Arc::new(Mutex::new(SharedScheduleState::new("running", 1))),
        );

        assert!(
            registry.get("old").is_none(),
            "expired entry must be evicted"
        );
        assert!(registry.get("fresh").is_some());
        assert!(registry.get("running").is_some());
    }

    #[test]
    fn registry_caps_completed_schedules_evicting_oldest() {
        let registry = SharedScheduleRegistry::default();
        let base = std::time::Instant::now();
        for index in 0..=MAX_COMPLETED_SCHEDULES {
            let id = format!("s-{index}");
            registry.insert(
                id.clone(),
                completed_state(&id, base + std::time::Duration::from_millis(index as u64)),
            );
        }
        registry.insert(
            "trigger".to_string(),
            Arc::new(Mutex::new(SharedScheduleState::new("trigger", 1))),
        );

        assert!(
            registry.get("s-0").is_none(),
            "oldest completed entry beyond the cap must be evicted"
        );
        assert!(
            registry
                .get(&format!("s-{MAX_COMPLETED_SCHEDULES}"))
                .is_some()
        );
        assert!(registry.get("trigger").is_some());
    }

    #[test]
    fn registry_never_evicts_running_schedules() {
        let registry = SharedScheduleRegistry::default();
        for index in 0..MAX_COMPLETED_SCHEDULES * 2 {
            let id = format!("run-{index}");
            registry.insert(
                id,
                Arc::new(Mutex::new(SharedScheduleState::new("running", 1))),
            );
        }
        assert!(registry.get("run-0").is_some());
    }

    /// Verifies that the registry and ActiveSchedule share the same Arc,
    /// so updates from the schedule state machine are visible via registry.get().
    /// The registry previously stored a separate copy.
    #[test]
    fn test_registry_shares_arc_with_schedule() {
        let registry = SharedScheduleRegistry::default();
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-1", 2)));
        registry.insert("s-1".to_string(), shared.clone());

        // Simulate what finalize_schedule does
        {
            let mut guard = shared.lock().unwrap();
            guard.status = ScheduleStatus::Completed;
            guard.completed_actions = 2;
            guard.results.push(ActionResult {
                index: 0,
                label: Some("a".to_string()),
                tool: "pause_time".to_string(),
                at: 0.0,
                at_frame: None,
                fired_at_game_time: 1.0,
                status: ActionStatus::Ok,
                result: Some(serde_json::json!({"paused": true})),
                error: None,
                ..Default::default()
            });
        }

        // Registry should see the updated state via the shared Arc
        let state = registry.get("s-1").unwrap();
        assert_eq!(state.status, ScheduleStatus::Completed);
        assert_eq!(state.completed_actions, 2);
        assert_eq!(state.results.len(), 1);
        assert_eq!(state.results[0].tool, "pause_time");
    }

    /// Verifies cancel propagates through the shared Arc so the schedule
    /// state machine sees it on the next frame poll.
    #[test]
    fn test_registry_cancel_visible_through_arc() {
        let registry = SharedScheduleRegistry::default();
        let shared = Arc::new(Mutex::new(SharedScheduleState::new("s-2", 1)));
        registry.insert("s-2".to_string(), shared.clone());

        // Cancel via registry (HTTP handler path)
        assert!(registry.cancel("s-2"));

        // Schedule state machine reads from its own Arc clone
        let guard = shared.lock().unwrap();
        assert!(guard.cancelled);
    }

    #[test]
    fn test_shared_schedule_state_new() {
        let state = SharedScheduleState::new("test-id", 5);
        assert_eq!(state.schedule_id, "test-id");
        assert_eq!(state.status, ScheduleStatus::Running);
        assert_eq!(state.total_actions, 5);
        assert_eq!(state.completed_actions, 0);
        assert_eq!(state.executed_actions, 0);
        assert_eq!(state.skipped_actions, 0);
        assert_eq!(state.aborted_actions, 0);
        assert_eq!(state.cancelled_actions, 0);
        assert!(state.results.is_empty());
        assert!(!state.cancelled);
    }
}
