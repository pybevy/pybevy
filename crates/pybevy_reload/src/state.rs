use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use bevy::prelude::Resource;

/// Mode for hot reload operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadMode {
    /// Full reload: despawn entities, clear custom resources, reload all systems including Startup
    Full,
    /// Partial reload: keep entities and resources, only update Update/Last systems
    Partial,
}

/// Bevy resource that tracks which generation of systems should be active
/// Systems added with a specific generation will only run when this matches
#[derive(Resource, Clone)]
pub struct HotReloadGeneration {
    /// Current active generation
    pub current: u32,
    /// Atomic counter shared with HotReloadState
    generation_counter: Arc<AtomicU32>,
    /// Track which generations have already run their Startup schedule
    /// This prevents Startup from running multiple times per generation
    pub(crate) startup_run_for_generations: Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,
}

impl HotReloadGeneration {
    pub fn new(generation_counter: Arc<AtomicU32>) -> Self {
        Self {
            current: generation_counter.load(Ordering::SeqCst),
            generation_counter,
            startup_run_for_generations: Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }

    /// Update to the latest generation from the atomic counter
    pub fn update(&mut self) {
        self.current = self.generation_counter.load(Ordering::SeqCst);
    }

    /// Mark that Startup has run for the current generation
    pub fn mark_startup_run(&self) {
        if let Ok(mut set) = self.startup_run_for_generations.lock() {
            set.insert(self.current);
        }
    }

    /// Check if Startup has already run for a given generation
    pub fn has_startup_run(&self, generation: u32) -> bool {
        self.startup_run_for_generations
            .lock()
            .map(|set| set.contains(&generation))
            .unwrap_or(false)
    }
}

/// Run condition function that checks if the current generation matches the expected one
/// This is used to enable/disable Update and Last systems based on hot reload generation
/// If HotReloadGeneration resource doesn't exist (hot reload not enabled), always returns true
pub fn generation_matches(
    expected_generation: u32,
) -> impl FnMut(Option<bevy::ecs::system::Res<HotReloadGeneration>>) -> bool + Clone {
    move |generation_res: Option<bevy::ecs::system::Res<HotReloadGeneration>>| {
        match generation_res {
            Some(res) => res.current == expected_generation,
            None => true, // No hot reload, all systems run
        }
    }
}

/// Run condition for Startup systems - only runs when generation matches AND hasn't run yet
/// Startup systems should run once during their generation (at app start or after reload)
/// If HotReloadGeneration resource doesn't exist (hot reload not enabled), always returns true
pub fn startup_or_reload(
    expected_generation: u32,
) -> impl FnMut(Option<bevy::ecs::system::Res<HotReloadGeneration>>) -> bool + Clone {
    move |generation_res: Option<bevy::ecs::system::Res<HotReloadGeneration>>| {
        match generation_res {
            Some(res) => {
                // Only run if:
                // 1. Current generation matches expected generation
                // 2. Startup hasn't run yet for this generation
                res.current == expected_generation && !res.has_startup_run(expected_generation)
            }
            None => true, // No hot reload, all systems run
        }
    }
}
