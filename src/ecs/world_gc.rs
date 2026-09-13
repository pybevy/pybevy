use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
};

use bevy::ecs::world::WorldId;
use pyo3::Python;

#[derive(Debug, Default)]
struct TraversalState {
    mutations: AtomicUsize,
    closed: AtomicBool,
}

const TRAVERSING: usize = usize::MAX;

static WORLD_GC_STATES: OnceLock<Mutex<HashMap<WorldId, Weak<TraversalState>>>> = OnceLock::new();

/// Only owned Python Worlds register this state; borrowed adapters share it.
#[derive(Clone, Debug)]
pub(crate) struct WorldGcState(WorldId, Arc<TraversalState>);

impl WorldGcState {
    pub(crate) fn new(world: WorldId) -> Self {
        let state = Self(world, Arc::new(TraversalState::default()));
        Python::attach(|_| {
            WORLD_GC_STATES
                .get_or_init(Mutex::default)
                .lock()
                .unwrap()
                .insert(world, Arc::downgrade(&state.1));
        });
        state
    }

    pub(crate) fn for_world(world: WorldId) -> Option<Self> {
        let states = WORLD_GC_STATES.get()?;
        Python::attach(|_| {
            states
                .lock()
                .unwrap()
                .get(&world)
                .and_then(Weak::upgrade)
                .map(|state| Self(world, state))
        })
    }

    pub(crate) fn suspend(&self) -> WorldGcGuard {
        Python::attach(|py| {
            loop {
                let active = self.1.mutations.load(Ordering::Acquire);
                if active == TRAVERSING {
                    py.detach(thread::yield_now);
                    continue;
                }
                assert!(active < TRAVERSING - 1, "World mutation counter overflow");
                if self
                    .1
                    .mutations
                    .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    break;
                }
            }
        });
        WorldGcGuard(self.clone())
    }

    pub(crate) fn try_traverse(&self) -> Option<WorldGcTraversalGuard> {
        if self.1.closed.load(Ordering::Acquire) {
            return None;
        }
        self.1
            .mutations
            .compare_exchange(0, TRAVERSING, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        let guard = WorldGcTraversalGuard(self.clone());
        if self.1.closed.load(Ordering::Acquire) {
            return None;
        }
        Some(guard)
    }

    pub(crate) fn close(&self) {
        let _guard = self.suspend();
        Python::attach(|_| {
            self.1.closed.store(true, Ordering::Release);
            WORLD_GC_STATES
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .remove(&self.0);
        });
    }
}

pub(crate) struct WorldGcTraversalGuard(WorldGcState);

impl Drop for WorldGcTraversalGuard {
    fn drop(&mut self) {
        self.0.1.mutations.store(0, Ordering::Release);
    }
}

pub(crate) struct WorldGcGuard(WorldGcState);

impl Drop for WorldGcGuard {
    fn drop(&mut self) {
        // Attachment makes suspension stable throughout both stop-the-world GC passes.
        Python::attach(|_| {
            self.0.1.mutations.fetch_sub(1, Ordering::AcqRel);
        });
    }
}
