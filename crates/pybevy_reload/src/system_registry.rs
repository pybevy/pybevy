use std::collections::{HashMap, HashSet};

use bevy::prelude::Resource;

/// Backend-neutral bookkeeping for reloadable system handles.
///
/// The registry owns one handle list per reload generation and the most recent
/// set of system names. Interpreter-specific retirement remains a callback so
/// Python references are always released under the originating runtime's rules.
#[derive(Resource)]
pub struct SystemGenerationRegistry<H: Send + Sync + 'static> {
    generations: HashMap<u32, Vec<H>>,
    known_systems: HashSet<String>,
}

impl<H: Send + Sync + 'static> Default for SystemGenerationRegistry<H> {
    fn default() -> Self {
        Self {
            generations: HashMap::new(),
            known_systems: HashSet::new(),
        }
    }
}

impl<H: Send + Sync + 'static> SystemGenerationRegistry<H> {
    /// Record a system handle under its reload generation.
    pub fn register(&mut self, generation: u32, handle: H) {
        self.generations.entry(generation).or_default().push(handle);
    }

    /// Retire and remove every handle older than `keep_after`.
    ///
    /// The callback borrows each handle before the registry drops its owning
    /// reference. Backends use it to clear interpreter references with their
    /// required lock/attachment ordering.
    pub fn cleanup_old_generations(&mut self, keep_after: u32, mut retire: impl FnMut(&H)) {
        let old_generations: Vec<u32> = self
            .generations
            .keys()
            .filter(|&&generation| generation < keep_after)
            .copied()
            .collect();

        for generation in old_generations {
            if let Some(handles) = self.generations.remove(&generation) {
                for handle in &handles {
                    retire(handle);
                }
            }
        }
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
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

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
}
