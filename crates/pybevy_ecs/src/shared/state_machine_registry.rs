//! Interpreter-neutral state-machine identity and hot-reload aliasing.

use std::collections::{HashMap, HashSet};

use super::schedule::StateMachineId;

/// Interpreter-neutral exact identity for one Python state class.
///
/// Adapters supply an address paired with its current qualified name.
pub type StateTypeKey = usize;

/// The two resource channels exposed by one state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateResourceKind {
    Current,
    Next,
}

impl StateResourceKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Current => "State",
            Self::Next => "NextState",
        }
    }
}

/// Validate the one-machine compatibility spelling for a state resource.
///
/// A missing or single machine is not ambiguous. Adapters must keep checking
/// at materialization time because another machine can be registered after the
/// system itself.
pub const fn validate_bare_state_resource_topology(
    kind: Option<StateResourceKind>,
    machine_count: usize,
) -> Result<(), StateResourceKind> {
    match (kind, machine_count > 1) {
        (Some(kind), true) => Err(kind),
        _ => Ok(()),
    }
}

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
/// Every address lookup checks `module.qualname` before reuse. Qualified names
/// preserve logical machines across class redefinition; monotonic IDs prevent
/// a recycled address from replacing an existing machine.
#[derive(Debug, Default)]
pub struct StateMachineIdentityRegistry {
    by_type: HashMap<StateTypeKey, (String, StateMachineId)>,
    next_id: usize,
    by_qualified_name: HashMap<String, StateMachineId>,
}

impl StateMachineIdentityRegistry {
    #[must_use]
    pub fn get(&self, type_key: StateTypeKey, qualified_name: &str) -> Option<StateMachineId> {
        self.by_type
            .get(&type_key)
            .and_then(|(name, id)| (name == qualified_name).then_some(*id))
    }

    pub fn retain(&mut self, qualified_names: &HashSet<String>) {
        self.by_type
            .retain(|_, (name, _)| qualified_names.contains(name));
        self.by_qualified_name
            .retain(|name, _| qualified_names.contains(name));
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
        if let Some(id) = self.get(type_key, qualified_name) {
            return StateMachineRegisterOutcome::Reused(id);
        }

        if let Some(id) = self.get_by_qualified_name(qualified_name) {
            self.by_type
                .insert(type_key, (qualified_name.to_owned(), id));
            return StateMachineRegisterOutcome::Aliased(id);
        }

        self.next_id += 1;
        let id = StateMachineId::new(self.next_id);
        self.by_type
            .insert(type_key, (qualified_name.to_owned(), id));
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
        let second = registry.register(0x1000, "game.Phase");

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
        assert_eq!(registry.get(0x2000, "game.Phase"), Some(first.machine_id()));
    }

    #[test]
    fn same_bare_name_in_different_modules_stays_distinct() {
        let mut registry = StateMachineIdentityRegistry::default();
        let first = registry.register(0x1000, "alpha.Phase");
        let second = registry.register(0x2000, "beta.Phase");

        assert!(matches!(second, StateMachineRegisterOutcome::Registered(_)));
        assert_ne!(first.machine_id(), second.machine_id());
    }

    #[test]
    fn recycled_addresses_do_not_reuse_machine_ids() {
        let mut registry = StateMachineIdentityRegistry::default();
        let first = registry.register(0x1000, "game.Phase").machine_id();
        registry.register(0x2000, "game.Phase");
        assert_eq!(registry.get(0x1000, "foreign.Phase"), None);
        let foreign = registry.register(0x1000, "foreign.Phase").machine_id();
        assert_ne!(first, foreign);
        assert_eq!(registry.get(0x2000, "game.Phase"), Some(first));
        assert_eq!(registry.get(0x1000, "foreign.Phase"), Some(foreign));
    }

    #[test]
    fn removing_machines_prunes_aliases_without_reusing_ids() {
        let mut registry = StateMachineIdentityRegistry::default();
        let first = registry.register(0x1000, "game.Phase").machine_id();
        registry.register(0x2000, "game.Phase");
        registry.retain(&HashSet::new());
        assert_eq!(registry.get_by_qualified_name("game.Phase"), None);
        assert_eq!(registry.get(0x2000, "game.Phase"), None);
        let second = registry.register(0x1000, "game.Phase").machine_id();
        assert_ne!(first, second);
    }

    #[test]
    fn bare_resource_is_ambiguous_only_with_multiple_machines() {
        for machine_count in [0, 1] {
            assert_eq!(
                validate_bare_state_resource_topology(
                    Some(StateResourceKind::Current),
                    machine_count,
                ),
                Ok(())
            );
            assert_eq!(
                validate_bare_state_resource_topology(Some(StateResourceKind::Next), machine_count),
                Ok(())
            );
        }

        assert_eq!(
            validate_bare_state_resource_topology(Some(StateResourceKind::Current), 2),
            Err(StateResourceKind::Current)
        );
        assert_eq!(
            validate_bare_state_resource_topology(Some(StateResourceKind::Next), 3),
            Err(StateResourceKind::Next)
        );
        assert_eq!(validate_bare_state_resource_topology(None, 2), Ok(()));
    }
}
