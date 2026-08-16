//! Backend-neutral identities for logical types sharing native storage.

use std::{any::TypeId, collections::HashMap};

use bevy::ecs::component::Component;

/// Process-local identity for a language-level type whose instances share a
/// native storage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogicalTypeId(u64);

impl LogicalTypeId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Per-entity logical identities, keyed by each native component's Rust type.
///
/// A map (rather than one marker component per entity) lets independent logical
/// asset/component families coexist on the same entity.
#[derive(Component, Debug, Default)]
pub struct LogicalTypeMap(HashMap<TypeId, LogicalTypeId>);

impl LogicalTypeMap {
    pub fn insert(&mut self, native_type: TypeId, logical_type: LogicalTypeId) {
        self.0.insert(native_type, logical_type);
    }

    pub fn remove(&mut self, native_type: TypeId) {
        self.0.remove(&native_type);
    }

    pub fn matches(&self, native_type: TypeId, logical_type: LogicalTypeId) -> bool {
        self.0.get(&native_type) == Some(&logical_type)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
