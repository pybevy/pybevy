//! Entity wrapper for PyBevy
//!
//! This module provides the Python binding for Bevy's Entity type.

use bevy::ecs::entity::Entity;
use pyo3::prelude::*;

#[pyclass(name = "Entity", eq, hash, frozen)]
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
    pub fn from_bits(bits: u64) -> Self {
        PyEntity(Entity::from_bits(bits))
    }

    pub fn __repr__(&self) -> String {
        format!("Entity({:?})", self.0)
    }
}
