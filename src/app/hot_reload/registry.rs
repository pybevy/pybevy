use bevy::prelude::Resource;
use pyo3::prelude::*;

use crate::ecs::dynamic_system::DynamicSystemHandle;

/// Registry tracking DynamicSystem handles by generation.
/// Used to "gut" old-generation systems, releasing their Python references
/// even though the DynamicSystem structs remain in Bevy's schedule.
#[derive(Resource, Default)]
pub(crate) struct DynamicSystemRegistry {
    pub(crate) generations: std::collections::HashMap<u32, Vec<DynamicSystemHandle>>,
    /// System names from the most recent generation, for detecting renames/removals
    pub(crate) known_systems: std::collections::HashSet<String>,
}

impl DynamicSystemRegistry {
    /// Register a system handle for a generation
    pub(crate) fn register(&mut self, generation: u32, handle: DynamicSystemHandle) {
        self.generations.entry(generation).or_default().push(handle);
    }

    /// Gut all systems from generations older than the threshold.
    /// Acquires GIL first, then per-system Mutexes, ensuring consistent
    /// lock ordering with DynamicSystem::run_unsafe (GIL -> Mutex).
    pub(crate) fn cleanup_old_generations(&mut self, keep_after: u32) {
        let old_gens: Vec<u32> = self
            .generations
            .keys()
            .filter(|&&g| g < keep_after)
            .copied()
            .collect();

        if old_gens.is_empty() {
            return;
        }

        // Hold GIL for the entire gut loop so dropping Py<PyAny> refs is safe,
        // and lock ordering is GIL -> per-system Mutex (matching run_unsafe).
        Python::attach(|_py| {
            for old_gen in old_gens {
                if let Some(handles) = self.generations.remove(&old_gen) {
                    for handle in handles {
                        let mut inner = handle.lock().unwrap_or_else(|p| p.into_inner());
                        inner.gut();
                    }
                }
            }
        });
    }

    /// Compare new system names against known set, update tracker, return removed names.
    pub(crate) fn detect_system_delta(
        &mut self,
        new_systems: std::collections::HashSet<String>,
    ) -> Vec<String> {
        if self.known_systems.is_empty() {
            // First reload: record the initial set
            self.known_systems = new_systems;
            return Vec::new();
        }

        let removed: Vec<_> = self
            .known_systems
            .difference(&new_systems)
            .cloned()
            .collect();
        self.known_systems = new_systems;
        removed
    }
}
