//! Hierarchy component wrappers
//!
//! This module provides the Python bindings for Bevy's hierarchy components:
//! - ChildOf: Relationship component indicating parent entity
//! - Children: Auto-managed list of child entities (read-only)

use bevy::ecs::hierarchy::{ChildOf, Children};
use pyo3::{exceptions::PyIndexError, prelude::*};

use crate::{ComponentStorage, PyComponent, PyEntity};

// Manual implementation because component_storage macro uses pybevy_core:: paths
// which don't work inside pybevy_core itself.
#[pyclass(name = "ChildOf", extends = PyComponent, frozen)]
#[derive(Debug, Clone)]
pub struct PyChildOf {
    pub(crate) storage: ComponentStorage<ChildOf>,
}

impl From<ChildOf> for PyChildOf {
    fn from(component: ChildOf) -> Self {
        Self {
            storage: ComponentStorage::owned(component),
        }
    }
}

impl TryFrom<PyChildOf> for ChildOf {
    type Error = PyErr;

    fn try_from(py_component: PyChildOf) -> PyResult<Self> {
        Ok(py_component.storage.into_owned()?)
    }
}

impl TryFrom<&ChildOf> for PyChildOf {
    type Error = PyErr;

    fn try_from(component: &ChildOf) -> PyResult<Self> {
        Ok(Self {
            storage: ComponentStorage::owned(component.clone()),
        })
    }
}

impl PyChildOf {
    /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
    pub fn from_owned(component: ChildOf) -> (Self, PyComponent) {
        (
            Self {
                storage: ComponentStorage::owned(component),
            },
            PyComponent,
        )
    }

    /// Create from a borrowed component storage (for query iteration).
    pub fn from_borrowed(storage: ComponentStorage<ChildOf>) -> (Self, PyComponent) {
        (Self { storage }, PyComponent)
    }

    #[inline(always)]
    pub fn as_ref(&self) -> PyResult<&ChildOf> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    pub fn as_mut(&mut self) -> PyResult<&mut ChildOf> {
        Ok(self.storage.as_mut()?)
    }
}

#[pymethods]
impl PyChildOf {
    #[new]
    pub fn new(parent: PyEntity) -> (Self, PyComponent) {
        Self::from_owned(ChildOf(parent.0))
    }

    pub fn parent(&self) -> PyResult<PyEntity> {
        Ok(PyEntity(self.as_ref()?.parent()))
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()?.parent() == other.as_ref()?.parent())
    }
}

// ============================================================================
// PyChildren - Auto-managed list of child entities (read-only)
// ============================================================================

/// Auto-managed list of child entities.
///
/// This component is automatically maintained by Bevy when ChildOf relationships change.
/// It should NOT be modified directly - instead, add/remove ChildOf components on children.
#[pyclass(name = "Children", extends = PyComponent, frozen, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyChildren {
    entities: Vec<PyEntity>,
}

impl TryFrom<&Children> for PyChildren {
    type Error = PyErr;

    fn try_from(value: &Children) -> Result<Self, Self::Error> {
        // Children derefs to &[Entity], so we can iterate
        let entities = value.iter().map(|e| PyEntity(*e)).collect();
        Ok(PyChildren { entities })
    }
}

impl TryFrom<PyChildren> for Children {
    type Error = PyErr;

    fn try_from(_value: PyChildren) -> Result<Self, Self::Error> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot create Children from Python - it is auto-managed by Bevy. Use ChildOf components to build hierarchies.",
        ))
    }
}

#[pymethods]
impl PyChildren {
    /// Get list of child entities.
    pub fn entities(&self) -> Vec<PyEntity> {
        self.entities.clone()
    }

    /// Get number of children.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Check if has any children.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Iterate over children.
    fn __iter__(slf: PyRef<'_, Self>) -> PyChildrenIterator {
        PyChildrenIterator {
            children: slf.entities.clone(),
            index: 0,
        }
    }

    /// Get child by index.
    fn __getitem__(&self, idx: isize) -> PyResult<PyEntity> {
        let len = self.entities.len() as isize;
        let actual_idx = if idx < 0 {
            (len + idx) as usize
        } else {
            idx as usize
        };

        self.entities
            .get(actual_idx)
            .copied()
            .ok_or_else(|| PyIndexError::new_err(format!("Index {} out of bounds", idx)))
    }

    /// Support len() in Python.
    fn __len__(&self) -> usize {
        self.entities.len()
    }

    fn __repr__(&self) -> String {
        format!("Children(count={})", self.entities.len())
    }
}

#[pyclass]
pub struct PyChildrenIterator {
    children: Vec<PyEntity>,
    index: usize,
}

#[pymethods]
impl PyChildrenIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyEntity> {
        if slf.index < slf.children.len() {
            let entity = slf.children[slf.index];
            slf.index += 1;
            Some(entity)
        } else {
            None
        }
    }
}
