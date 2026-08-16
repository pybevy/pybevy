//! Entity wrapper for PyBevy
//!
//! This module provides the Python binding for Bevy's Entity type.

use bevy::ecs::entity::Entity;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::public_error::invalid_entity_bits;

#[pyclass(name = "Entity", eq, hash, frozen, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyEntity(pub Entity);

impl From<Entity> for PyEntity {
    fn from(entity: Entity) -> Self {
        PyEntity(entity)
    }
}

impl From<PyEntity> for Entity {
    fn from(py_entity: PyEntity) -> Self {
        py_entity.0
    }
}

#[pymethods]
impl PyEntity {
    #[staticmethod]
    fn from_raw(raw: u32) -> Option<Self> {
        Entity::from_raw_u32(raw).map(PyEntity)
    }

    /// Convert Entity to u64 bits representation
    pub fn to_bits(&self) -> u64 {
        self.0.to_bits()
    }

    /// Create Entity from u64 bits representation
    #[staticmethod]
    pub fn from_bits(bits: u64) -> PyResult<Self> {
        Entity::try_from_bits(bits)
            .map(PyEntity)
            .ok_or_else(|| PyValueError::new_err(invalid_entity_bits(bits)))
    }

    pub fn __repr__(&self) -> String {
        format!("Entity({:?})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_round_trip() {
        let entity = Entity::from_bits(42);
        let py_entity = PyEntity::from(entity);
        let bits = py_entity.to_bits();
        let restored = PyEntity::from_bits(bits).unwrap();
        assert_eq!(py_entity, restored);
    }

    #[test]
    fn from_raw_valid() {
        let py_entity = PyEntity::from_raw(7);
        assert!(py_entity.is_some());
        let bits = py_entity.unwrap().to_bits();
        let restored = PyEntity::from_raw(7).unwrap().to_bits();
        assert_eq!(bits, restored);
    }

    #[test]
    fn into_bevy_entity() {
        let py_entity = PyEntity::from_bits(99).unwrap();
        let bevy_entity: Entity = py_entity.into();
        assert_eq!(bevy_entity.to_bits(), 99);
    }

    #[test]
    fn from_bevy_entity() {
        let bevy_entity = Entity::from_bits(123);
        let py_entity: PyEntity = bevy_entity.into();
        assert_eq!(py_entity.to_bits(), 123);
    }

    #[test]
    fn equality_and_hash() {
        let a = PyEntity::from_bits(10).unwrap();
        let b = PyEntity::from_bits(10).unwrap();
        let c = PyEntity::from_bits(20).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);

        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn repr_contains_entity() {
        let e = PyEntity::from_bits(42).unwrap();
        let repr = e.__repr__();
        assert!(repr.starts_with("Entity("));
    }

    #[test]
    fn invalid_bits_are_rejected_instead_of_panicking() {
        // A zero low word is an invalid entity index, whatever the generation
        // in the high word. Bevy's Entity::from_bits panics on both.
        for bits in [0u64, 0xFFFF_FFFF_0000_0000] {
            assert!(PyEntity::from_bits(bits).is_err(), "{bits:#x} was accepted");
        }
    }
}
