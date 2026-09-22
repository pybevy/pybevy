use std::collections::{HashMap, HashSet};

use bevy::{ecs::schedule::InternedScheduleLabel, prelude::Resource};
use pybevy_ecs::shared::system_ticks::{
    SystemRegistrationIdentity, SystemRunHistory, SystemTickRegistration,
};

type TickRegistrations =
    HashMap<(InternedScheduleLabel, SystemRegistrationIdentity), Vec<Vec<SystemRunHistory>>>;

/// Backend-neutral bookkeeping for reloadable system handles.
///
/// The registry owns one handle list per reload generation and the most recent
/// set of system names. Interpreter-specific retirement remains a callback so
/// Python references are always released under the originating runtime's rules.
#[derive(Resource)]
pub struct SystemGenerationRegistry<H: Send + Sync + 'static> {
    generations: HashMap<u32, Vec<H>>,
    known_systems: HashSet<String>,
    ticks: HashMap<u32, TickRegistrations>,
}

impl<H: Send + Sync + 'static> Default for SystemGenerationRegistry<H> {
    fn default() -> Self {
        Self {
            generations: HashMap::new(),
            known_systems: HashSet::new(),
            ticks: HashMap::new(),
        }
    }
}

impl<H: Send + Sync + 'static> SystemGenerationRegistry<H> {
    /// Begin a fresh candidate without changing the committed generation's history.
    pub fn begin_tick_reload(&mut self, generation: u32) {
        self.ticks
            .retain(|old, _| *old >= generation.saturating_sub(2) && *old != generation);
    }

    pub fn register_ticks(
        &mut self,
        generation: u32,
        previous: Option<u32>,
        schedule: InternedScheduleLabel,
        registration: SystemTickRegistration,
    ) {
        let key = (schedule, registration.identity);
        let occurrence = self
            .ticks
            .get(&generation)
            .and_then(|entries| entries.get(&key))
            .map_or(0, Vec::len);
        if let Some(previous) = previous
            && let Some(previous) = self
                .ticks
                .get(&previous)
                .and_then(|entries| entries.get(&key))
                .and_then(|occurrences| occurrences.get(occurrence))
            && previous.len() == registration.leaves.len()
        {
            for (new, old) in registration.leaves.iter().zip(previous) {
                new.inherit_from(old);
            }
        }
        self.ticks
            .entry(generation)
            .or_default()
            .entry(key)
            .or_default()
            .push(registration.leaves);
    }

    pub fn track_generation(&mut self, generation: u32) {
        self.generations.entry(generation).or_default();
    }

    /// Record a system handle under its reload generation.
    pub fn register(&mut self, generation: u32, handle: H) {
        self.generations.entry(generation).or_default().push(handle);
    }

    /// Retire and remove every handle older than `keep_after`.
    ///
    /// The callback borrows each handle before the registry drops its owning
    /// reference. Backends use it to clear interpreter references with their
    /// required lock/attachment ordering.
    pub fn cleanup_old_generations(
        &mut self,
        keep_after: u32,
        mut retire: impl FnMut(&H),
    ) -> Vec<u32> {
        let old_generations: Vec<u32> = self
            .generations
            .keys()
            .filter(|&&generation| generation < keep_after)
            .copied()
            .collect();

        for &generation in &old_generations {
            self.ticks.remove(&generation);
            if let Some(handles) = self.generations.remove(&generation) {
                for handle in &handles {
                    retire(handle);
                }
            }
        }

        old_generations
    }

    /// Replace the latest system-name snapshot and return removed names.
    pub fn detect_system_delta(&mut self, new_systems: HashSet<String>) -> Vec<String> {
        let removed = self
            .known_systems
            .difference(&new_systems)
            .cloned()
            .collect();
        self.known_systems = new_systems;
        removed
    }

    pub fn set_system_baseline(&mut self, systems: HashSet<String>) {
        self.known_systems = systems;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use bevy::{
        app::{Last, Update},
        ecs::schedule::ScheduleLabel,
    };
    use pybevy_ecs::shared::schedule::ConditionExpr;

    use super::*;

    #[derive(Clone)]
    struct TestHandle {
        id: usize,
        retire_count: Arc<AtomicUsize>,
    }

    impl TestHandle {
        fn new(id: usize) -> Self {
            Self {
                id,
                retire_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn retire(&self) {
            self.retire_count.fetch_add(1, Ordering::SeqCst);
        }

        fn retired(&self) -> usize {
            self.retire_count.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn default_registry_is_empty() {
        let registry = SystemGenerationRegistry::<TestHandle>::default();
        assert!(registry.generations.is_empty());
        assert!(registry.known_systems.is_empty());
    }

    #[test]
    fn tick_registrations_are_scoped_by_schedule_structure_and_occurrence() {
        let mut registry = SystemGenerationRegistry::<()>::default();
        let identity = SystemRegistrationIdentity {
            callable: "scene.observe".to_owned(),
            ..Default::default()
        };
        for schedule in [Update.intern(), Update.intern(), Last.intern()] {
            registry.register_ticks(
                0,
                None,
                schedule,
                SystemTickRegistration {
                    identity: identity.clone(),
                    leaves: vec![SystemRunHistory::default()],
                },
            );
        }
        assert_eq!(
            registry.ticks[&0][&(Update.intern(), identity.clone())].len(),
            2
        );
        assert_eq!(
            registry.ticks[&0][&(Last.intern(), identity.clone())].len(),
            1
        );
        let separate = SystemRegistrationIdentity {
            conditions: vec![
                ConditionExpr::Leaf("scene.a".to_owned()),
                ConditionExpr::Leaf("scene.b".to_owned()),
            ],
            ..identity.clone()
        };
        let combined = SystemRegistrationIdentity {
            conditions: vec![ConditionExpr::And(
                Box::new(ConditionExpr::Leaf("scene.a".to_owned())),
                Box::new(ConditionExpr::Leaf("scene.b".to_owned())),
            )],
            ..identity.clone()
        };
        assert_ne!(separate, combined);
        registry.begin_tick_reload(1);
        registry.register_ticks(
            1,
            Some(0),
            Update.intern(),
            SystemTickRegistration {
                identity: identity.clone(),
                leaves: vec![SystemRunHistory::default()],
            },
        );
        registry.begin_tick_reload(1);
        assert!(!registry.ticks.contains_key(&1));
        assert_eq!(registry.ticks[&0][&(Update.intern(), identity)].len(), 2);
        registry.begin_tick_reload(4);
        assert!(registry.ticks.is_empty());
    }

    #[test]
    fn cleanup_retires_only_generations_below_threshold() {
        let mut registry = SystemGenerationRegistry::default();
        let old_a = TestHandle::new(1);
        let old_b = TestHandle::new(2);
        let current = TestHandle::new(3);
        registry.register(0, old_a.clone());
        registry.register(0, old_b.clone());
        registry.register(1, current.clone());

        let mut retired_ids = Vec::new();
        registry.cleanup_old_generations(1, |handle| {
            retired_ids.push(handle.id);
            handle.retire();
        });
        retired_ids.sort_unstable();

        assert_eq!(retired_ids, vec![1, 2]);
        assert_eq!(old_a.retired(), 1);
        assert_eq!(old_b.retired(), 1);
        assert_eq!(current.retired(), 0);
        assert!(!registry.generations.contains_key(&0));
        assert!(registry.generations.contains_key(&1));
    }

    #[test]
    fn cleanup_is_idempotent() {
        let mut registry = SystemGenerationRegistry::default();
        let handle = TestHandle::new(1);
        registry.register(0, handle.clone());

        registry.cleanup_old_generations(1, TestHandle::retire);
        registry.cleanup_old_generations(1, TestHandle::retire);

        assert_eq!(handle.retired(), 1);
    }

    #[test]
    fn cleanup_returns_tracked_generation_without_handles() {
        let mut registry = SystemGenerationRegistry::<TestHandle>::default();
        registry.track_generation(0);

        assert_eq!(registry.cleanup_old_generations(1, |_| {}), vec![0]);
    }

    #[test]
    fn system_delta_records_baseline_then_reports_removals() {
        let mut registry = SystemGenerationRegistry::<TestHandle>::default();
        let first = HashSet::from(["setup".to_string(), "update_score".to_string()]);
        assert!(registry.detect_system_delta(first).is_empty());

        let second = HashSet::from(["setup".to_string(), "update_timer".to_string()]);
        let removed = registry.detect_system_delta(second);

        assert_eq!(removed, vec!["update_score"]);
        assert!(registry.known_systems.contains("update_timer"));
        assert!(!registry.known_systems.contains("update_score"));
    }

    #[test]
    fn explicit_system_baseline_reports_first_reload_removals() {
        let mut registry = SystemGenerationRegistry::<TestHandle>::default();
        registry.set_system_baseline(HashSet::from(["old_name".to_string()]));

        assert_eq!(
            registry.detect_system_delta(HashSet::from(["new_name".to_string()])),
            vec!["old_name"]
        );
    }
}
