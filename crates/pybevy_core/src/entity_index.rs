use std::hash::{Hash, Hasher};

use bevy::ecs::entity::EntityIndex;
use pybevy_macros::pyvalue;
use pyo3::{IntoPyObjectExt, prelude::*};

use crate::{FromBorrowedStorage, ValueStorage};

#[pyvalue(EntityIndex)]
#[pyclass(module = "pybevy.ecs", name = "EntityIndex", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyEntityIndex {
    storage: ValueStorage<EntityIndex>,
}

impl PartialEq for PyEntityIndex {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => self.storage == other.storage,
        }
    }
}

impl Eq for PyEntityIndex {}

#[pymethods]
impl PyEntityIndex {
    #[staticmethod]
    pub fn from_raw(raw: u32) -> Option<Self> {
        EntityIndex::from_raw_u32(raw).map(Self::from_owned)
    }

    pub fn index(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.index())
    }

    fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?))
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    fn __hash__(&self) -> PyResult<u64> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.as_ref()?.hash(&mut hasher);
        Ok(hasher.finish())
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: pyo3::pyclass::CompareOp,
    ) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let Ok(other) = other.extract::<Self>() else {
            return py.NotImplemented().into_py_any(py);
        };
        match op {
            pyo3::pyclass::CompareOp::Eq => (self == &other).into_py_any(py),
            pyo3::pyclass::CompareOp::Ne => (self != &other).into_py_any(py),
            _ => py.NotImplemented().into_py_any(py),
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("EntityIndex({})", self.index()?))
    }
}

impl From<EntityIndex> for PyEntityIndex {
    fn from(value: EntityIndex) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyEntityIndex> for EntityIndex {
    type Error = PyErr;

    fn try_from(value: PyEntityIndex) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyEntityIndex> for EntityIndex {
    type Error = PyErr;

    fn try_from(value: &PyEntityIndex) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn raw_index_round_trip_and_limit() {
        let index = PyEntityIndex::from_raw(7).unwrap();
        assert_eq!(index.index().unwrap(), 7);
        assert!(PyEntityIndex::from_raw(u32::MAX).is_none());
    }

    #[test]
    fn conversion_preserves_nominal_value() {
        let native = EntityIndex::from_raw_u32(42).unwrap();
        let wrapped = PyEntityIndex::from(native);
        assert_eq!(EntityIndex::try_from(&wrapped).unwrap(), native);
        assert_eq!(wrapped.__repr__().unwrap(), "EntityIndex(42)");
    }
}
