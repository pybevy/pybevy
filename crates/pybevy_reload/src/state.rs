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

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;

    use bevy::ecs::schedule::IntoScheduleConfigs;

    use super::*;

    #[test]
    fn test_reload_mode_eq() {
        assert_eq!(ReloadMode::Full, ReloadMode::Full);
        assert_eq!(ReloadMode::Partial, ReloadMode::Partial);
        assert_ne!(ReloadMode::Full, ReloadMode::Partial);
    }

    #[test]
    fn test_reload_mode_debug() {
        assert_eq!(format!("{:?}", ReloadMode::Full), "Full");
        assert_eq!(format!("{:?}", ReloadMode::Partial), "Partial");
    }

    #[test]
    fn test_reload_mode_clone_copy() {
        let mode = ReloadMode::Full;
        let cloned = mode.clone();
        let copied = mode;
        assert_eq!(cloned, copied);
    }

    #[test]
    fn test_hot_reload_generation_new() {
        let counter = Arc::new(AtomicU32::new(5));
        let hr_gen = HotReloadGeneration::new(counter.clone());
        assert_eq!(hr_gen.current, 5);
    }

    #[test]
    fn test_hot_reload_generation_update() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut hr_gen = HotReloadGeneration::new(counter.clone());
        assert_eq!(hr_gen.current, 0);

        counter.store(3, Ordering::SeqCst);
        hr_gen.update();
        assert_eq!(hr_gen.current, 3);
    }

    #[test]
    fn test_hot_reload_generation_startup_tracking() {
        let counter = Arc::new(AtomicU32::new(0));
        let hr_gen = HotReloadGeneration::new(counter);

        assert!(!hr_gen.has_startup_run(0));
        hr_gen.mark_startup_run();
        assert!(hr_gen.has_startup_run(0));
        assert!(!hr_gen.has_startup_run(1));
    }

    #[derive(Resource, Default)]
    struct Marker(bool);

    #[derive(Resource, Default)]
    struct MarkerA(bool);

    #[derive(Resource, Default)]
    struct MarkerB(bool);

    #[derive(Resource, Default)]
    struct MarkerC(bool);

    fn set_marker(mut m: bevy::ecs::system::ResMut<Marker>) {
        m.0 = true;
    }

    fn set_marker_a(mut m: bevy::ecs::system::ResMut<MarkerA>) {
        m.0 = true;
    }

    fn set_marker_b(mut m: bevy::ecs::system::ResMut<MarkerB>) {
        m.0 = true;
    }

    fn set_marker_c(mut m: bevy::ecs::system::ResMut<MarkerC>) {
        m.0 = true;
    }

    #[test]
    fn test_generation_matches_allows_matching_generation() {
        use bevy::ecs::{schedule::Schedule, world::World};

        let mut world = World::new();
        let counter = Arc::new(AtomicU32::new(0));
        world.insert_resource(HotReloadGeneration::new(counter));
        world.insert_resource(Marker(false));

        let mut schedule = Schedule::default();
        schedule.add_systems(set_marker.run_if(generation_matches(0)));
        schedule.run(&mut world);

        assert!(
            world.resource::<Marker>().0,
            "System should run when generation matches"
        );
    }

    #[test]
    fn test_generation_matches_blocks_wrong_generation() {
        use bevy::ecs::{schedule::Schedule, world::World};

        let mut world = World::new();
        let counter = Arc::new(AtomicU32::new(1));
        world.insert_resource(HotReloadGeneration::new(counter));
        world.insert_resource(Marker(false));

        let mut schedule = Schedule::default();
        schedule.add_systems(set_marker.run_if(generation_matches(0)));
        schedule.run(&mut world);

        assert!(
            !world.resource::<Marker>().0,
            "System should NOT run when generation doesn't match"
        );
    }

    #[test]
    fn test_generation_matches_allows_when_no_resource() {
        use bevy::ecs::{schedule::Schedule, world::World};

        let mut world = World::new();
        // Do NOT insert HotReloadGeneration
        world.insert_resource(Marker(false));

        let mut schedule = Schedule::default();
        schedule.add_systems(set_marker.run_if(generation_matches(0)));
        schedule.run(&mut world);

        assert!(
            world.resource::<Marker>().0,
            "System should run when HotReloadGeneration resource is absent (None => true)"
        );
    }

    #[test]
    fn test_generation_matches_multiple_generations() {
        use bevy::ecs::{schedule::Schedule, world::World};

        let mut world = World::new();
        let counter = Arc::new(AtomicU32::new(2));
        world.insert_resource(HotReloadGeneration::new(counter));
        world.insert_resource(MarkerA(false));
        world.insert_resource(MarkerB(false));
        world.insert_resource(MarkerC(false));

        let mut schedule = Schedule::default();
        schedule.add_systems(set_marker_a.run_if(generation_matches(0)));
        schedule.add_systems(set_marker_b.run_if(generation_matches(1)));
        schedule.add_systems(set_marker_c.run_if(generation_matches(2)));
        schedule.run(&mut world);

        assert!(
            !world.resource::<MarkerA>().0,
            "Gen 0 system should not run at gen 2"
        );
        assert!(
            !world.resource::<MarkerB>().0,
            "Gen 1 system should not run at gen 2"
        );
        assert!(
            world.resource::<MarkerC>().0,
            "Gen 2 system should run at gen 2"
        );
    }

    #[test]
    fn test_startup_or_reload_allows_fresh_generation() {
        use bevy::ecs::{schedule::Schedule, world::World};

        let mut world = World::new();
        let counter = Arc::new(AtomicU32::new(0));
        world.insert_resource(HotReloadGeneration::new(counter));
        world.insert_resource(Marker(false));

        let mut schedule = Schedule::default();
        schedule.add_systems(set_marker.run_if(startup_or_reload(0)));
        schedule.run(&mut world);

        assert!(
            world.resource::<Marker>().0,
            "System should run for fresh generation (startup not yet marked)"
        );
    }

    #[test]
    fn test_startup_or_reload_blocks_after_startup_run() {
        use bevy::ecs::{schedule::Schedule, world::World};

        let mut world = World::new();
        let counter = Arc::new(AtomicU32::new(0));
        let hr_gen = HotReloadGeneration::new(counter);
        hr_gen.mark_startup_run();
        world.insert_resource(hr_gen);
        world.insert_resource(Marker(false));

        let mut schedule = Schedule::default();
        schedule.add_systems(set_marker.run_if(startup_or_reload(0)));
        schedule.run(&mut world);

        assert!(
            !world.resource::<Marker>().0,
            "System should NOT run after startup has already run for this generation"
        );
    }

    #[test]
    fn test_startup_or_reload_blocks_wrong_generation() {
        use bevy::ecs::{schedule::Schedule, world::World};

        let mut world = World::new();
        let counter = Arc::new(AtomicU32::new(1));
        world.insert_resource(HotReloadGeneration::new(counter));
        world.insert_resource(Marker(false));

        let mut schedule = Schedule::default();
        schedule.add_systems(set_marker.run_if(startup_or_reload(0)));
        schedule.run(&mut world);

        assert!(
            !world.resource::<Marker>().0,
            "System should NOT run when generation doesn't match (even if startup hasn't run)"
        );
    }
}
