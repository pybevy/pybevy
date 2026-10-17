use std::sync::{Arc, Mutex};

pub use pybevy_ecs::shared::system_runtime::{
    HotReloadGeneration, ReloadGenerationSet, generation_matches, startup_or_reload,
};

use crate::lock_or_recover;

/// Mode for hot reload operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadMode {
    /// Full reload: despawn entities, clear custom resources, reload all systems including Startup
    Full,
    /// Partial reload: keep entities and resources, only update Update/Last systems
    Partial,
}

#[derive(Debug)]
struct ReloadRequestInner {
    pending: Option<ReloadMode>,
    last_mode: ReloadMode,
    active_mode: Option<ReloadMode>,
}

/// Thread-safe, last-request-wins mailbox for reload requests.
///
/// Loader functions, script sources, and completion results remain in the
/// interpreter adapters. This type owns only the common request lifecycle.
#[derive(Clone, Debug)]
pub struct ReloadRequestState {
    inner: Arc<Mutex<ReloadRequestInner>>,
}

/// Keeps the consumed request's import mode stable until the attempt ends.
#[derive(Debug)]
pub struct ReloadRequest {
    mode: ReloadMode,
    state: ReloadRequestState,
}

impl ReloadRequest {
    pub fn mode(&self) -> ReloadMode {
        self.mode
    }
}

impl Drop for ReloadRequest {
    fn drop(&mut self) {
        lock_or_recover(&self.state.inner).active_mode = None;
    }
}

impl Default for ReloadRequestState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ReloadRequestInner {
                pending: None,
                last_mode: ReloadMode::Full,
                active_mode: None,
            })),
        }
    }
}

impl ReloadRequestState {
    /// Queue a reload, replacing any request that has not yet been consumed.
    pub fn request(&self, mode: ReloadMode) {
        let mut inner = lock_or_recover(&self.inner);
        inner.pending = Some(mode);
        inner.last_mode = mode;
    }

    /// Queue a reload only if the mailbox is currently empty.
    pub fn request_if_idle(&self, mode: ReloadMode) -> bool {
        let mut inner = lock_or_recover(&self.inner);
        if inner.pending.is_some() {
            return false;
        }
        inner.pending = Some(mode);
        inner.last_mode = mode;
        true
    }

    /// Consume an idle mailbox's request and hold its mode for the attempt.
    pub fn take(&self) -> Option<ReloadRequest> {
        let mut inner = lock_or_recover(&self.inner);
        if inner.active_mode.is_some() {
            return None;
        }
        let mode = inner.pending.take()?;
        inner.active_mode = Some(mode);
        Some(ReloadRequest {
            mode,
            state: self.clone(),
        })
    }

    pub fn is_pending(&self) -> bool {
        lock_or_recover(&self.inner).pending.is_some()
    }

    /// Mode of the most recently queued request, retained after consumption.
    pub fn last_mode(&self) -> ReloadMode {
        lock_or_recover(&self.inner).last_mode
    }

    /// Import policy of the active attempt, or the most recent idle request.
    pub fn import_mode(&self) -> ReloadMode {
        let inner = lock_or_recover(&self.inner);
        inner.active_mode.unwrap_or(inner.last_mode)
    }

    /// Make the loader import with `mode` for the attempt in progress.
    pub fn set_import_mode(&self, mode: ReloadMode) {
        let mut inner = lock_or_recover(&self.inner);
        if let Some(active) = &mut inner.active_mode {
            *active = mode;
        } else {
            inner.last_mode = mode;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc, Barrier,
            atomic::{AtomicU32, AtomicUsize, Ordering},
        },
        thread,
    };

    use bevy::{
        ecs::{
            schedule::{IntoScheduleConfigs, Schedule},
            world::World,
        },
        prelude::Resource,
    };

    use super::*;

    #[test]
    fn reload_request_defaults_to_idle_full() {
        let requests = ReloadRequestState::default();
        assert!(!requests.is_pending());
        assert_eq!(requests.last_mode(), ReloadMode::Full);
        assert!(requests.take().is_none());
    }

    #[test]
    fn reload_request_is_last_request_wins_and_single_consume() {
        let requests = ReloadRequestState::default();
        requests.request(ReloadMode::Full);
        requests.request(ReloadMode::Partial);

        assert_eq!(
            requests.take().map(|request| request.mode()),
            Some(ReloadMode::Partial)
        );
        assert!(requests.take().is_none());
        assert_eq!(requests.last_mode(), ReloadMode::Partial);
    }

    #[test]
    fn request_if_idle_preserves_existing_request() {
        let requests = ReloadRequestState::default();
        assert!(requests.request_if_idle(ReloadMode::Partial));
        assert!(!requests.request_if_idle(ReloadMode::Full));
        assert_eq!(
            requests.take().map(|request| request.mode()),
            Some(ReloadMode::Partial)
        );
    }

    #[test]
    fn concurrent_request_if_idle_admits_exactly_one_request() {
        let requests = ReloadRequestState::default();
        let barrier = Arc::new(Barrier::new(8));
        let admitted = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..8)
            .map(|index| {
                let requests = requests.clone();
                let barrier = barrier.clone();
                let admitted = admitted.clone();
                thread::spawn(move || {
                    barrier.wait();
                    let mode = if index % 2 == 0 {
                        ReloadMode::Full
                    } else {
                        ReloadMode::Partial
                    };
                    if requests.request_if_idle(mode) {
                        admitted.fetch_add(1, Ordering::SeqCst);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }

        assert_eq!(admitted.load(Ordering::SeqCst), 1);
        assert!(requests.take().is_some());
        assert!(!requests.is_pending());
    }

    #[test]
    fn active_import_mode_ignores_later_requests_and_preserves_the_next_one() {
        for mode in [ReloadMode::Full, ReloadMode::Partial] {
            let requests = ReloadRequestState::default();
            requests.request(mode);
            let active = requests.take().unwrap();
            let next = if mode == ReloadMode::Full {
                ReloadMode::Partial
            } else {
                ReloadMode::Full
            };
            assert!(requests.request_if_idle(next));
            assert_eq!(requests.last_mode(), next);
            assert_eq!(requests.import_mode(), mode);
            assert!(requests.take().is_none());
            drop(active);
            assert_eq!(requests.import_mode(), next);
            assert_eq!(requests.take().unwrap().mode(), next);
            assert!(!requests.is_pending());
        }
    }

    #[test]
    fn import_override_and_unwinding_leave_the_next_request_intact() {
        let requests = ReloadRequestState::default();
        requests.request(ReloadMode::Partial);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _active = requests.take().unwrap();
            requests.set_import_mode(ReloadMode::Full);
            assert!(requests.request_if_idle(ReloadMode::Partial));
            assert_eq!(requests.import_mode(), ReloadMode::Full);
            panic!("failed reload attempt");
        }));
        assert!(result.is_err());
        assert_eq!(requests.import_mode(), ReloadMode::Partial);
        assert_eq!(requests.take().unwrap().mode(), ReloadMode::Partial);
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
