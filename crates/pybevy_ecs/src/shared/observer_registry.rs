//! Interpreter-neutral observer identity, indexing, filtering, and cleanup.
//!
//! This module deliberately does not invoke Python or inspect a [`World`].
//! Backend adapters resolve interpreter type objects and component ids before
//! registration, snapshot entries while the registry is borrowed, and perform
//! per-callback World checks only after releasing that borrow.

use std::{collections::HashMap, error::Error, fmt, sync::Arc};

use bevy::ecs::{component::ComponentId, entity::Entity, prelude::Resource};

/// Stable identity of an interpreter type object for one interpreter lifetime.
///
/// Adapters map their stable type-object identity to this integer key and must
/// retain a strong type handle for as long as it is registered, so an
/// interpreter cannot recycle the identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObserverTypeKey(usize);

impl ObserverTypeKey {
    /// Construct a key from a backend's type-object identity.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Return the backend identity represented by this key.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// The component lifecycle transition observed by an entry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LifecycleKind {
    Add,
    Insert,
    Remove,
    Discard,
    Despawn,
}

/// Registry key for a user-defined event or component lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ObserverEventKey {
    User(ObserverTypeKey),
    Lifecycle(LifecycleKind),
}

/// One component filter resolved in both interpreter and ECS identity spaces.
///
/// Keeping the pair in one value prevents independently-built type-key and
/// component-id vectors from drifting out of alignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedObserverComponent {
    pub type_key: ObserverTypeKey,
    pub component_id: ComponentId,
}

/// OR-filter applied to an observer dispatch.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObserverFilter {
    components: Vec<ResolvedObserverComponent>,
}

impl ObserverFilter {
    /// Construct a filter from components resolved during registration.
    #[must_use]
    pub fn new(components: Vec<ResolvedObserverComponent>) -> Self {
        Self { components }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    #[must_use]
    pub fn components(&self) -> &[ResolvedObserverComponent] {
        &self.components
    }

    /// Test a user-event target with the documented OR semantics.
    ///
    /// An empty filter matches without requiring a target. A non-empty filter
    /// requires a target and matches if it contains any resolved component.
    pub fn matches_user_target(
        &self,
        target: Option<Entity>,
        mut contains: impl FnMut(Entity, ComponentId) -> bool,
    ) -> bool {
        if self.is_empty() {
            return true;
        }
        let Some(target) = target else {
            return false;
        };
        self.components
            .iter()
            .any(|component| contains(target, component.component_id))
    }

    /// Test the exact component involved in a lifecycle transition.
    ///
    /// Lifecycle adapters pass both identities. Requiring the paired match
    /// avoids treating a stale interpreter key or an unrelated ECS id as the
    /// component named by the observer annotation.
    #[must_use]
    pub fn matches_lifecycle_component(
        &self,
        type_key: ObserverTypeKey,
        component_id: ComponentId,
    ) -> bool {
        self.components.iter().any(|component| {
            component.type_key == type_key && component.component_id == component_id
        })
    }
}

/// One observer registration.
///
/// `H` is a context-free prepared handle. The registry always owns it through
/// an [`Arc`], so cloning a snapshot cannot run Python, acquire the interpreter,
/// or invoke a finalizer while the registry is borrowed.
#[derive(Debug)]
pub struct ObserverEntry<H> {
    pub observer_entity: Entity,
    pub prepared: Arc<H>,
    pub event: ObserverEventKey,
    pub filter: ObserverFilter,
    pub target: Option<Entity>,
}

impl<H> Clone for ObserverEntry<H> {
    fn clone(&self) -> Self {
        Self {
            observer_entity: self.observer_entity,
            prepared: Arc::clone(&self.prepared),
            event: self.event,
            filter: self.filter.clone(),
            target: self.target,
        }
    }
}

/// An insertion would violate the one-entry-per-observer-entity invariant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuplicateObserverEntity(pub Entity);

impl fmt::Display for DuplicateObserverEntity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "observer entity {:?} is already registered",
            self.0
        )
    }
}

impl Error for DuplicateObserverEntity {}

/// Context-free observer registry with forward and reverse indices.
///
/// Entries in each event vector preserve registration order. All removal APIs
/// return complete entries so interpreter handles can be destroyed only after
/// the caller releases the World resource borrow.
#[derive(Debug, Resource)]
pub struct ObserverRegistryCore<H> {
    by_event: HashMap<ObserverEventKey, Vec<ObserverEntry<H>>>,
    event_for_observer: HashMap<Entity, ObserverEventKey>,
    observers_for_target: HashMap<Entity, Vec<Entity>>,
}

impl<H> Default for ObserverRegistryCore<H> {
    fn default() -> Self {
        Self {
            by_event: HashMap::new(),
            event_for_observer: HashMap::new(),
            observers_for_target: HashMap::new(),
        }
    }
}

impl<H> ObserverRegistryCore<H> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.event_for_observer.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.event_for_observer.len()
    }

    /// Insert one fully-resolved entry into every index.
    ///
    /// Registration should parse and resolve all fallible backend state before
    /// calling this method. A duplicate entity is rejected without changing any
    /// index.
    pub fn insert(&mut self, entry: ObserverEntry<H>) -> Result<(), DuplicateObserverEntity> {
        let observer = entry.observer_entity;
        if self.event_for_observer.contains_key(&observer) {
            return Err(DuplicateObserverEntity(observer));
        }

        let event = entry.event;
        let target = entry.target;
        self.by_event.entry(event).or_default().push(entry);
        self.event_for_observer.insert(observer, event);
        if let Some(target) = target {
            self.observers_for_target
                .entry(target)
                .or_default()
                .push(observer);
        }
        Ok(())
    }

    /// Snapshot entries for an event and target in registration order.
    ///
    /// Bundle/lifecycle component checks intentionally happen later, once per
    /// callback, so structural changes made by an earlier observer are visible
    /// to later observers. An entry present in this snapshot remains considered
    /// once even if it is removed during dispatch; registrations made after the
    /// snapshot wait for the next dispatch.
    #[must_use]
    pub fn snapshot(
        &self,
        event: ObserverEventKey,
        target: Option<Entity>,
    ) -> Vec<ObserverEntry<H>> {
        self.by_event
            .get(&event)
            .into_iter()
            .flatten()
            .filter(|entry| entry.target.is_none() || entry.target == target)
            .cloned()
            .collect()
    }

    /// Remove one observer and return its complete entry.
    pub fn remove(&mut self, observer: Entity) -> Option<ObserverEntry<H>> {
        let event = self.event_for_observer.remove(&observer)?;
        let entries = self
            .by_event
            .get_mut(&event)
            .expect("observer reverse index must reference an event bucket");
        let position = entries
            .iter()
            .position(|entry| entry.observer_entity == observer)
            .expect("observer reverse index must reference an event entry");
        let removed = entries.remove(position);
        if entries.is_empty() {
            self.by_event.remove(&event);
        }

        if let Some(target) = removed.target {
            let observers = self
                .observers_for_target
                .get_mut(&target)
                .expect("target reverse index must reference an observer list");
            let position = observers
                .iter()
                .position(|candidate| *candidate == observer)
                .expect("target reverse index must reference the observer");
            observers.remove(position);
            if observers.is_empty() {
                self.observers_for_target.remove(&target);
            }
        }

        Some(removed)
    }

    /// Remove every entity-targeted observer watching `target`.
    ///
    /// Returned entries follow their registration order for that target.
    pub fn remove_for_target(&mut self, target: Entity) -> Vec<ObserverEntry<H>> {
        let Some(observers) = self.observers_for_target.remove(&target) else {
            return Vec::new();
        };

        observers
            .into_iter()
            .map(|observer| {
                let event = self
                    .event_for_observer
                    .remove(&observer)
                    .expect("target index must reference a registered observer");
                let entries = self
                    .by_event
                    .get_mut(&event)
                    .expect("observer reverse index must reference an event bucket");
                let position = entries
                    .iter()
                    .position(|entry| entry.observer_entity == observer)
                    .expect("observer reverse index must reference an event entry");
                let removed = entries.remove(position);
                if entries.is_empty() {
                    self.by_event.remove(&event);
                }
                removed
            })
            .collect()
    }

    /// Drain every entry and reset every index.
    ///
    /// Callers must drop/retire returned handles after releasing the registry's
    /// World resource borrow.
    pub fn clear(&mut self) -> Vec<ObserverEntry<H>> {
        self.event_for_observer.clear();
        self.observers_for_target.clear();
        self.by_event
            .drain()
            .flat_map(|(_, entries)| entries)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{component::ComponentId, entity::Entity};

    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("test entity index is nonzero")
    }

    fn component(index: usize) -> ResolvedObserverComponent {
        ResolvedObserverComponent {
            type_key: ObserverTypeKey::new(index + 100),
            component_id: ComponentId::new(index),
        }
    }

    fn entry(
        observer_index: u32,
        prepared: &'static str,
        event: ObserverEventKey,
        filter: ObserverFilter,
        target: Option<Entity>,
    ) -> ObserverEntry<String> {
        ObserverEntry {
            observer_entity: entity(observer_index),
            prepared: Arc::new(prepared.to_string()),
            event,
            filter,
            target,
        }
    }

    #[test]
    fn typed_event_keys_do_not_alias() {
        let type_key = ObserverTypeKey::new(7);
        assert_ne!(
            ObserverEventKey::User(type_key),
            ObserverEventKey::Lifecycle(LifecycleKind::Add)
        );
        assert_ne!(
            ObserverEventKey::Lifecycle(LifecycleKind::Add),
            ObserverEventKey::Lifecycle(LifecycleKind::Insert)
        );
    }

    #[test]
    fn snapshot_preserves_registration_order_and_target_scope() {
        let event = ObserverEventKey::User(ObserverTypeKey::new(1));
        let target = entity(90);
        let other_target = entity(91);
        let mut registry = ObserverRegistryCore::default();
        registry
            .insert(entry(1, "global-a", event, ObserverFilter::default(), None))
            .unwrap();
        registry
            .insert(entry(
                2,
                "target",
                event,
                ObserverFilter::default(),
                Some(target),
            ))
            .unwrap();
        registry
            .insert(entry(3, "global-b", event, ObserverFilter::default(), None))
            .unwrap();
        registry
            .insert(entry(
                4,
                "other",
                event,
                ObserverFilter::default(),
                Some(other_target),
            ))
            .unwrap();

        let snapshot = registry.snapshot(event, Some(target));
        let names: Vec<_> = snapshot
            .iter()
            .map(|entry| entry.prepared.as_str())
            .collect();
        assert_eq!(names, ["global-a", "target", "global-b"]);

        let global_snapshot = registry.snapshot(event, None);
        let names: Vec<_> = global_snapshot
            .iter()
            .map(|entry| entry.prepared.as_str())
            .collect();
        assert_eq!(names, ["global-a", "global-b"]);
    }

    #[test]
    fn user_filter_is_or_and_requires_a_target() {
        let a = component(1);
        let b = component(2);
        let filter = ObserverFilter::new(vec![a, b]);
        let target = entity(50);

        assert!(!filter.matches_user_target(None, |_, _| true));
        assert!(filter.matches_user_target(Some(target), |entity, id| {
            entity == target && id == b.component_id
        }));
        assert!(!filter.matches_user_target(Some(target), |_, _| false));
        assert!(ObserverFilter::default().matches_user_target(None, |_, _| false));
    }

    #[test]
    fn lifecycle_filter_requires_the_exact_paired_identity() {
        let component = component(4);
        let filter = ObserverFilter::new(vec![component]);

        assert!(filter.matches_lifecycle_component(component.type_key, component.component_id));
        assert!(!filter.matches_lifecycle_component(
            ObserverTypeKey::new(component.type_key.get() + 1),
            component.component_id
        ));
        assert!(!filter.matches_lifecycle_component(component.type_key, ComponentId::new(99)));
    }

    #[test]
    fn duplicate_insert_is_rejected_without_mutating_indices() {
        let event = ObserverEventKey::User(ObserverTypeKey::new(1));
        let mut registry = ObserverRegistryCore::default();
        registry
            .insert(entry(1, "first", event, ObserverFilter::default(), None))
            .unwrap();
        let error = registry
            .insert(entry(
                1,
                "duplicate",
                ObserverEventKey::Lifecycle(LifecycleKind::Add),
                ObserverFilter::default(),
                Some(entity(9)),
            ))
            .unwrap_err();

        assert_eq!(error, DuplicateObserverEntity(entity(1)));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.snapshot(event, None).len(), 1);
        assert!(
            registry
                .snapshot(
                    ObserverEventKey::Lifecycle(LifecycleKind::Add),
                    Some(entity(9))
                )
                .is_empty()
        );
    }

    #[test]
    fn remove_updates_event_and_target_indices() {
        let event = ObserverEventKey::User(ObserverTypeKey::new(1));
        let target = entity(80);
        let mut registry = ObserverRegistryCore::default();
        registry
            .insert(entry(
                1,
                "target-a",
                event,
                ObserverFilter::default(),
                Some(target),
            ))
            .unwrap();
        registry
            .insert(entry(
                2,
                "target-b",
                event,
                ObserverFilter::default(),
                Some(target),
            ))
            .unwrap();

        let removed = registry.remove(entity(1)).unwrap();
        assert_eq!(removed.prepared.as_str(), "target-a");
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.snapshot(event, Some(target)).len(), 1);

        let removed = registry.remove_for_target(target);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].prepared.as_str(), "target-b");
        assert!(registry.is_empty());
        assert!(registry.remove_for_target(target).is_empty());
    }

    #[test]
    fn remove_for_target_preserves_target_registration_order() {
        let user = ObserverEventKey::User(ObserverTypeKey::new(1));
        let lifecycle = ObserverEventKey::Lifecycle(LifecycleKind::Despawn);
        let target = entity(70);
        let other = entity(71);
        let mut registry = ObserverRegistryCore::default();
        registry
            .insert(entry(
                1,
                "first",
                user,
                ObserverFilter::default(),
                Some(target),
            ))
            .unwrap();
        registry
            .insert(entry(2, "global", user, ObserverFilter::default(), None))
            .unwrap();
        registry
            .insert(entry(
                3,
                "second",
                lifecycle,
                ObserverFilter::new(vec![component(3)]),
                Some(target),
            ))
            .unwrap();
        registry
            .insert(entry(
                4,
                "other",
                user,
                ObserverFilter::default(),
                Some(other),
            ))
            .unwrap();

        let removed = registry.remove_for_target(target);
        let names: Vec<_> = removed
            .iter()
            .map(|entry| entry.prepared.as_str())
            .collect();
        assert_eq!(names, ["first", "second"]);
        assert_eq!(registry.len(), 2);
        assert_eq!(registry.snapshot(user, None).len(), 1);
        assert_eq!(registry.snapshot(user, Some(other)).len(), 2);
    }

    #[test]
    fn snapshots_are_stable_across_removal_and_later_registration() {
        let event = ObserverEventKey::User(ObserverTypeKey::new(1));
        let mut registry = ObserverRegistryCore::default();
        registry
            .insert(entry(1, "old", event, ObserverFilter::default(), None))
            .unwrap();

        let snapshot = registry.snapshot(event, None);
        registry.remove(entity(1));
        registry
            .insert(entry(2, "new", event, ObserverFilter::default(), None))
            .unwrap();

        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].prepared.as_str(), "old");
        assert_eq!(registry.snapshot(event, None)[0].prepared.as_str(), "new");
    }

    #[test]
    fn clear_returns_every_entry_and_resets_all_indices() {
        let event = ObserverEventKey::User(ObserverTypeKey::new(1));
        let target = entity(60);
        let mut registry = ObserverRegistryCore::default();
        registry
            .insert(entry(1, "global", event, ObserverFilter::default(), None))
            .unwrap();
        registry
            .insert(entry(
                2,
                "target",
                event,
                ObserverFilter::default(),
                Some(target),
            ))
            .unwrap();

        let mut names: Vec<_> = registry
            .clear()
            .into_iter()
            .map(|entry| entry.prepared.as_str().to_string())
            .collect();
        names.sort_unstable();
        assert_eq!(names, ["global".to_string(), "target".to_string()]);
        assert!(registry.is_empty());
        assert!(registry.snapshot(event, Some(target)).is_empty());
        assert!(registry.remove_for_target(target).is_empty());
    }
}
