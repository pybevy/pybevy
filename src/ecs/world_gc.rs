use std::thread;

use bevy::ecs::world::WorldId;
use pybevy_core::world_traversal::{WorldMutationGuard, WorldTraversalGuard, WorldTraversalState};
use pyo3::Python;

/// Python World and App owners register this state; borrowed adapters share it.
#[derive(Clone, Debug)]
pub(crate) struct WorldGcState(WorldTraversalState);

impl WorldGcState {
    pub(crate) fn new(world: WorldId) -> Self {
        Python::attach(|_| Self(WorldTraversalState::new(world)))
    }

    pub(crate) fn for_world(world: WorldId) -> Option<Self> {
        Python::attach(|_| WorldTraversalState::for_world(world).map(Self))
    }

    pub(crate) fn suspend(&self) -> WorldGcGuard {
        Python::attach(|py| {
            loop {
                if let Some(guard) = self.0.try_suspend() {
                    return WorldGcGuard(Some(guard));
                }
                py.detach(thread::yield_now);
            }
        })
    }

    pub(crate) fn try_traverse(&self) -> Option<WorldTraversalGuard> {
        self.0.try_traverse()
    }

    pub(crate) fn close(&self) {
        let _guard = self.suspend();
        Python::attach(|_| self.0.close());
    }
}

pub(crate) struct WorldGcGuard(Option<WorldMutationGuard>);

impl Drop for WorldGcGuard {
    fn drop(&mut self) {
        // Attachment makes suspension stable throughout both stop-the-world GC passes.
        Python::attach(|_| drop(self.0.take()));
    }
}
