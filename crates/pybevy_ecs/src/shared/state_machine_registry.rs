//! Interpreter-neutral state-machine identity and hot-reload aliasing.

use std::collections::HashMap;

use super::schedule::StateMachineId;

/// Interpreter-neutral exact identity for one Python state class.
///
/// PyO3 supplies `PyTypeObject* as usize`; RustPython supplies its object id.
pub type StateTypeKey = usize;

/// Result of registering a state class identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateMachineRegisterOutcome {
    /// The exact class identity was already known.
    Reused(StateMachineId),
    /// A redefined class was attached to an existing qualified-name machine.
    Aliased(StateMachineId),
    /// A new logical state machine was created.
    Registered(StateMachineId),
}

impl StateMachineRegisterOutcome {
    #[must_use]
    pub const fn machine_id(self) -> StateMachineId {
        match self {
            Self::Reused(id) | Self::Aliased(id) | Self::Registered(id) => id,
        }
    }
}

/// App-local mapping from interpreter class identities to stable state machines.
///
/// The full `module.qualname` is used only when an exact identity is new. This
/// preserves one machine across class redefinition without conflating unrelated
/// classes that happen to share a bare name.
#[derive(Debug, Default)]
pub struct StateMachineIdentityRegistry {
    by_type: HashMap<StateTypeKey, StateMachineId>,
    by_qualified_name: HashMap<String, StateMachineId>,
}

impl StateMachineIdentityRegistry {
    #[must_use]
    pub fn get(&self, type_key: StateTypeKey) -> Option<StateMachineId> {
        self.by_type.get(&type_key).copied()
    }

    #[must_use]
    pub fn get_by_qualified_name(&self, qualified_name: &str) -> Option<StateMachineId> {
        self.by_qualified_name.get(qualified_name).copied()
    }

    pub fn register(
        &mut self,
        type_key: StateTypeKey,
        qualified_name: &str,
    ) -> StateMachineRegisterOutcome {
        if let Some(id) = self.get(type_key) {
            return StateMachineRegisterOutcome::Reused(id);
        }

        if let Some(id) = self.get_by_qualified_name(qualified_name) {
            self.by_type.insert(type_key, id);
            return StateMachineRegisterOutcome::Aliased(id);
        }

        let id = StateMachineId::new(type_key);
        self.by_type.insert(type_key, id);
        self.by_qualified_name.insert(qualified_name.to_owned(), id);
        StateMachineRegisterOutcome::Registered(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_identity_is_idempotent() {
        let mut registry = StateMachineIdentityRegistry::default();
        let first = registry.register(0x1000, "game.Phase");
        let second = registry.register(0x1000, "changed.Phase");

        assert_eq!(
            second,
            StateMachineRegisterOutcome::Reused(first.machine_id())
        );
    }

    #[test]
    fn redefined_qualified_class_aliases_existing_machine() {
        let mut registry = StateMachineIdentityRegistry::default();
        let first = registry.register(0x1000, "game.Phase");
        let second = registry.register(0x2000, "game.Phase");

        assert_eq!(
            second,
            StateMachineRegisterOutcome::Aliased(first.machine_id())
        );
        assert_eq!(registry.get(0x2000), Some(first.machine_id()));
    }

    #[test]
    fn same_bare_name_in_different_modules_stays_distinct() {
        let mut registry = StateMachineIdentityRegistry::default();
        let first = registry.register(0x1000, "alpha.Phase");
        let second = registry.register(0x2000, "beta.Phase");

        assert!(matches!(second, StateMachineRegisterOutcome::Registered(_)));
        assert_ne!(first.machine_id(), second.machine_id());
    }
}
