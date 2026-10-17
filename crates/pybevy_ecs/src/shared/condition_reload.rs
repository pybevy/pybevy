use std::{
    mem::replace,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use bevy::{
    ecs::{
        change_detection::{CheckChangeTicks, Tick},
        query::FilteredAccessSet,
        schedule::{BoxedCondition, InternedSystemSet},
        system::{ReadOnlySystem, RunSystemError, System, SystemIn, SystemStateFlags},
        world::{DeferredWorld, World, WorldId, unsafe_world_cell::UnsafeWorldCell},
    },
    prelude::DebugName,
};

struct ConditionState {
    system: BoxedCondition,
    initialized: Option<(WorldId, FilteredAccessSet)>,
}

impl ConditionState {
    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        if let Some((id, access)) = &self.initialized {
            assert_eq!(*id, world.id());
            return access.clone();
        }
        let access = self.system.initialize(world);
        self.initialized = Some((world.id(), access.clone()));
        access
    }
}

/// Shared native ownership of a persistent condition, without extra interpreter references.
#[derive(Clone)]
pub struct ConditionHandle(Arc<Mutex<ConditionState>>);

#[derive(Debug, PartialEq, Eq)]
pub enum ConditionReloadError {
    DifferentWorld,
    AccessChanged,
    FlagsChanged,
}

impl ConditionHandle {
    fn lock(&self) -> MutexGuard<'_, ConditionState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Initialize a candidate without changing the installed callable or scheduler contract.
    pub fn prepare_replacement(
        &self,
        world: &mut World,
        mut replacement: BoxedCondition,
    ) -> Result<ConditionReplacement, ConditionReloadError> {
        let (access, flags) = {
            let mut current = self.lock();
            if current
                .initialized
                .as_ref()
                .is_some_and(|(id, _)| *id != world.id())
            {
                return Err(ConditionReloadError::DifferentWorld);
            }
            (current.initialize(world), current.system.flags())
        };
        let replacement_access = replacement.initialize(world);
        if access != replacement_access {
            return Err(ConditionReloadError::AccessChanged);
        }
        if flags != replacement.flags() {
            return Err(ConditionReloadError::FlagsChanged);
        }
        Ok(ConditionReplacement {
            handle: self.clone(),
            replacement,
            world: world.id(),
        })
    }
}

/// A checked condition replacement staged until the reload commits.
pub struct ConditionReplacement {
    handle: ConditionHandle,
    replacement: BoxedCondition,
    world: WorldId,
}

impl ConditionReplacement {
    /// Return the displaced system so the adapter can drop interpreter values outside locks.
    pub fn commit(mut self, world: &mut World, preserve_ticks: bool) -> BoxedCondition {
        assert_eq!(self.world, world.id());
        let mut current = self.handle.lock();
        if preserve_ticks {
            self.replacement.set_last_run(current.system.get_last_run());
        }
        replace(&mut current.system, self.replacement)
    }
}

pub struct ReloadableCondition {
    handle: ConditionHandle,
}

impl ReloadableCondition {
    pub fn new(system: BoxedCondition) -> (Self, ConditionHandle) {
        let handle = ConditionHandle(Arc::new(Mutex::new(ConditionState {
            system,
            initialized: None,
        })));
        (
            Self {
                handle: handle.clone(),
            },
            handle,
        )
    }
}

impl System for ReloadableCondition {
    type In = ();
    type Out = bool;

    fn name(&self) -> DebugName {
        self.handle.lock().system.name()
    }

    fn flags(&self) -> SystemStateFlags {
        self.handle.lock().system.flags()
    }

    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<bool, RunSystemError> {
        // SAFETY: replacements preserve the initialized access and thread flags.
        unsafe { self.handle.lock().system.run_unsafe(input, world) }
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.handle.lock().system.apply_deferred(world);
    }

    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.handle.lock().system.queue_deferred(world);
    }

    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        self.handle.lock().initialize(world)
    }

    fn check_change_tick(&mut self, check: CheckChangeTicks) {
        self.handle.lock().system.check_change_tick(check);
    }

    fn default_system_sets(&self) -> Vec<InternedSystemSet> {
        self.handle.lock().system.default_system_sets()
    }

    fn get_last_run(&self) -> Tick {
        self.handle.lock().system.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.handle.lock().system.set_last_run(last_run);
    }
}

// SAFETY: construction and replacement accept only boxed ReadOnlySystem values.
unsafe impl ReadOnlySystem for ReloadableCondition {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use bevy::{
        ecs::system::{IntoSystem, NonSendMarker, Res},
        prelude::Resource,
    };

    use super::*;

    #[derive(Resource)]
    struct Gate;

    fn enabled() -> bool {
        true
    }
    fn disabled() -> bool {
        false
    }
    fn with_resource(_: Res<Gate>) -> bool {
        true
    }
    fn main_thread(_: NonSendMarker) -> bool {
        true
    }

    #[test]
    fn replacement_is_staged_and_uses_the_current_tick_at_commit() {
        let mut world = World::new();
        let (mut condition, handle) =
            ReloadableCondition::new(Box::new(IntoSystem::into_system(enabled)));
        condition.initialize(&mut world);
        let pending = handle
            .prepare_replacement(&mut world, Box::new(IntoSystem::into_system(disabled)))
            .unwrap();
        assert!(condition.run((), &mut world).unwrap());
        condition.set_last_run(Tick::new(17));
        drop(pending.commit(&mut world, true));
        assert_eq!(condition.get_last_run(), Tick::new(17));
        assert!(!condition.run((), &mut world).unwrap());
    }

    #[test]
    fn incompatible_access_and_flags_leave_the_old_condition_running() {
        let mut world = World::new();
        world.insert_resource(Gate);
        let (mut condition, handle) =
            ReloadableCondition::new(Box::new(IntoSystem::into_system(enabled)));
        condition.initialize(&mut world);
        assert!(matches!(
            handle
                .prepare_replacement(&mut world, Box::new(IntoSystem::into_system(with_resource)),),
            Err(ConditionReloadError::AccessChanged)
        ));
        assert!(matches!(
            handle.prepare_replacement(&mut world, Box::new(IntoSystem::into_system(main_thread)),),
            Err(ConditionReloadError::FlagsChanged)
        ));
        let mut other = World::new();
        assert!(matches!(
            handle.prepare_replacement(&mut other, Box::new(IntoSystem::into_system(disabled)),),
            Err(ConditionReloadError::DifferentWorld)
        ));
        assert!(condition.run((), &mut world).unwrap());
    }

    struct DropProbe {
        handle: Arc<Mutex<Option<ConditionHandle>>>,
        unlocked: Arc<AtomicBool>,
    }

    impl Drop for DropProbe {
        fn drop(&mut self) {
            let handle = self.handle.lock().unwrap();
            self.unlocked.store(
                handle.as_ref().unwrap().0.try_lock().is_ok(),
                Ordering::Relaxed,
            );
        }
    }

    #[test]
    fn displaced_condition_drops_after_the_lock_is_released() {
        let handle_slot = Arc::new(Mutex::new(None));
        let unlocked = Arc::new(AtomicBool::new(false));
        let probe = DropProbe {
            handle: handle_slot.clone(),
            unlocked: unlocked.clone(),
        };
        let old = move || {
            let _ = &probe;
            true
        };
        let (mut condition, handle) =
            ReloadableCondition::new(Box::new(IntoSystem::into_system(old)));
        *handle_slot.lock().unwrap() = Some(handle.clone());
        let mut world = World::new();
        condition.initialize(&mut world);
        let pending = handle
            .prepare_replacement(&mut world, Box::new(IntoSystem::into_system(disabled)))
            .unwrap();
        drop(pending.commit(&mut world, false));
        assert!(unlocked.load(Ordering::Relaxed));
    }
}
