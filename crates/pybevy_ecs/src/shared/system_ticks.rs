//! Interpreter-free execution history transferred between reload registrations.

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicU32, Ordering},
};

use bevy::ecs::{change_detection::Tick, query::FilteredAccessSet, world::WorldId};

use super::schedule::{ConditionExpr, ScheduleOrdering};

#[derive(Clone)]
struct TickSource {
    world: WorldId,
    access: FilteredAccessSet,
    tick: Arc<AtomicU32>,
}

#[derive(Default)]
struct HistoryState {
    initialized: Option<(WorldId, FilteredAccessSet)>,
    predecessor: Option<TickSource>,
}

/// Native-only handle; retaining history never retains a Python callable or World.
#[derive(Clone, Default)]
pub struct SystemRunHistory {
    tick: Arc<AtomicU32>,
    state: Arc<Mutex<HistoryState>>,
}

impl SystemRunHistory {
    pub fn record(&self, tick: Tick) {
        self.tick.store(tick.get(), Ordering::Relaxed);
    }

    pub fn inherit_from(&self, previous: &Self) {
        let source = {
            let state = previous
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            state
                .initialized
                .as_ref()
                .map(|(world, access)| TickSource {
                    world: *world,
                    access: access.clone(),
                    // The old executable keeps aging this tick until replacement initialization.
                    tick: previous.tick.clone(),
                })
                .or_else(|| state.predecessor.clone())
        };
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .predecessor = source;
    }

    pub(crate) fn initialize(
        &self,
        world: WorldId,
        access: &FilteredAccessSet,
        fresh: Tick,
    ) -> Tick {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let tick = state
            .predecessor
            .take()
            .filter(|previous| previous.world == world && previous.access == *access)
            .map_or(fresh, |previous| {
                Tick::new(previous.tick.load(Ordering::Relaxed))
            });
        state.initialized = Some((world, access.clone()));
        self.record(tick);
        tick
    }
}

/// One configured system, including the separate ticks of its pipe/condition leaves.
#[derive(Default)]
pub struct SystemTickRegistration {
    pub identity: SystemRegistrationIdentity,
    pub leaves: Vec<SystemRunHistory>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SystemRegistrationIdentity {
    pub callable: String,
    pub pipeline: Vec<String>,
    pub conditions: Vec<ConditionExpr<String>>,
    pub ordering: ScheduleOrdering,
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;

    use super::*;

    #[test]
    fn inheritance_follows_predecessor_until_initialization_then_detaches() {
        let world = World::new();
        let other_world = World::new();
        let access = FilteredAccessSet::default();
        let old = SystemRunHistory::default();
        old.initialize(world.id(), &access, Tick::new(7));
        old.record(Tick::new(19));
        let replacement = SystemRunHistory::default();
        replacement.inherit_from(&old);
        old.record(Tick::new(23));
        assert_eq!(
            replacement.initialize(world.id(), &access, Tick::new(0)),
            Tick::new(23)
        );
        replacement.record(Tick::new(40));
        assert_eq!(old.tick.load(Ordering::Relaxed), 23);
        old.record(Tick::new(29));
        assert_eq!(replacement.tick.load(Ordering::Relaxed), 40);

        let other = SystemRunHistory::default();
        other.inherit_from(&old);
        assert_eq!(
            other.initialize(other_world.id(), &access, Tick::new(0)),
            Tick::new(0)
        );

        let mut changed = FilteredAccessSet::default();
        changed.write_all();
        let replacement = SystemRunHistory::default();
        replacement.inherit_from(&old);
        assert_eq!(
            replacement.initialize(world.id(), &changed, Tick::new(0)),
            Tick::new(0)
        );
    }

    #[test]
    fn uninitialized_predecessor_does_not_fabricate_a_run() {
        let world = World::new();
        let old = SystemRunHistory::default();
        let replacement = SystemRunHistory::default();
        replacement.inherit_from(&old);
        assert_eq!(
            replacement.initialize(world.id(), &FilteredAccessSet::default(), Tick::new(0)),
            Tick::new(0)
        );
    }

    #[test]
    fn consecutive_reloads_preserve_history_before_a_schedule_initializes() {
        let world = World::new();
        let access = FilteredAccessSet::default();
        let original = SystemRunHistory::default();
        original.initialize(world.id(), &access, Tick::new(19));
        let uninitialized = SystemRunHistory::default();
        uninitialized.inherit_from(&original);
        let replacement = SystemRunHistory::default();
        replacement.inherit_from(&uninitialized);
        original.record(Tick::new(23));
        assert_eq!(
            replacement.initialize(world.id(), &access, Tick::new(0)),
            Tick::new(23)
        );
    }

    #[test]
    fn transfer_preserves_tick_wraparound_ordering() {
        let world = World::new();
        let access = FilteredAccessSet::default();
        let old = SystemRunHistory::default();
        old.initialize(world.id(), &access, Tick::new(u32::MAX - 2));
        let replacement = SystemRunHistory::default();
        replacement.inherit_from(&old);
        let last_run = replacement.initialize(world.id(), &access, Tick::new(0));
        assert!(Tick::new(u32::MAX).is_newer_than(last_run, Tick::new(3)));
        assert!(Tick::new(1).is_newer_than(last_run, Tick::new(3)));
        assert!(!Tick::new(u32::MAX - 3).is_newer_than(last_run, Tick::new(3)));
    }
}
