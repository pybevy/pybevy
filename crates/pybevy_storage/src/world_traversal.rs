//! Shared exclusion between whole-World borrows and interpreter GC traversal.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
};

use bevy::ecs::world::WorldId;

#[derive(Debug, Default)]
struct TraversalState {
    mutations: AtomicUsize,
    closed: AtomicBool,
}

const TRAVERSING: usize = usize::MAX;
static WORLD_STATES: OnceLock<Mutex<HashMap<WorldId, Weak<TraversalState>>>> = OnceLock::new();

/// Interpreter-neutral state for the existing World traversal gate.
///
/// Backends must keep suspension transitions attached to their interpreter when
/// collection uses multiple passes. Traversal itself never locks the registry.
#[derive(Clone, Debug)]
pub struct WorldTraversalState(WorldId, Arc<TraversalState>);

impl WorldTraversalState {
    pub fn new(world: WorldId) -> Self {
        let state = Self(world, Arc::new(TraversalState::default()));
        WORLD_STATES
            .get_or_init(Mutex::default)
            .lock()
            .unwrap()
            .insert(world, Arc::downgrade(&state.1));
        state
    }

    pub fn for_world(world: WorldId) -> Option<Self> {
        WORLD_STATES
            .get()?
            .lock()
            .unwrap()
            .get(&world)
            .and_then(Weak::upgrade)
            .map(|state| Self(world, state))
    }

    pub fn try_suspend(&self) -> Option<WorldMutationGuard> {
        loop {
            let active = self.1.mutations.load(Ordering::Acquire);
            if active == TRAVERSING {
                return None;
            }
            assert!(active < TRAVERSING - 1, "World mutation counter overflow");
            if self
                .1
                .mutations
                .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(WorldMutationGuard(self.clone()));
            }
        }
    }

    pub fn suspend(&self) -> WorldMutationGuard {
        loop {
            if let Some(guard) = self.try_suspend() {
                return guard;
            }
            thread::yield_now();
        }
    }

    pub fn try_traverse(&self) -> Option<WorldTraversalGuard> {
        if self.1.closed.load(Ordering::Acquire) {
            return None;
        }
        self.1
            .mutations
            .compare_exchange(0, TRAVERSING, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        let guard = WorldTraversalGuard(self.clone());
        if self.1.closed.load(Ordering::Acquire) {
            return None;
        }
        Some(guard)
    }

    /// Close while a mutation guard excludes traversal.
    pub fn close(&self) {
        self.1.closed.store(true, Ordering::Release);
        WORLD_STATES.get().unwrap().lock().unwrap().remove(&self.0);
    }
}

pub struct WorldMutationGuard(WorldTraversalState);

impl Drop for WorldMutationGuard {
    fn drop(&mut self) {
        self.0.1.mutations.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct WorldTraversalGuard(WorldTraversalState);

impl Drop for WorldTraversalGuard {
    fn drop(&mut self) {
        self.0.1.mutations.store(0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;

    use super::*;

    #[test]
    fn nested_mutations_exclude_traversal_until_the_last_guard_drops() {
        let world = World::new();
        let state = WorldTraversalState::new(world.id());
        let first = state.suspend();
        let second = state.suspend();
        assert!(state.try_traverse().is_none());
        drop(first);
        assert!(state.try_traverse().is_none());
        drop(second);
        let traversal = state.try_traverse().unwrap();
        assert!(state.try_suspend().is_none());
        assert!(state.try_traverse().is_none());
        drop(traversal);
        let _guard = state.suspend();
        state.close();
        assert!(WorldTraversalState::for_world(world.id()).is_none());
        assert!(state.try_traverse().is_none());
    }
}
