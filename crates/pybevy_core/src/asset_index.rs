use std::hash::{Hash, Hasher};

use bevy::asset::AssetIndex;
use pybevy_macros::pyvalue;
use pyo3::{IntoPyObjectExt, prelude::*};

use crate::{FromBorrowedStorage, ValueStorage};

#[pyvalue(AssetIndex)]
#[pyclass(module = "pybevy.assets", name = "AssetIndex", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAssetIndex {
    storage: ValueStorage<AssetIndex>,
}

impl PartialEq for PyAssetIndex {
    /// Compare by value; fall back to storage identity when either side cannot
    /// be read, so `x == x` holds for an expired borrow.
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => self.storage == other.storage,
        }
    }
}

impl Eq for PyAssetIndex {}

#[pymethods]
impl PyAssetIndex {
    #[staticmethod]
    pub fn from_bits(bits: u64) -> Self {
        Self::from_owned(AssetIndex::from_bits(bits))
    }

    pub fn to_bits(&self) -> PyResult<u64> {
        Ok(self.as_ref()?.to_bits())
    }

    fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.to_bevy()?))
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
        Ok(format!("AssetIndex(bits={})", self.to_bits()?))
    }
}

#[cfg(test)]
mod tests {
    use pybevy_storage::{AccessMode, ValidityFlag};

    use super::*;

    #[test]
    fn expired_borrow_still_equals_itself() {
        let mut value = AssetIndex::from_bits(7);
        let validity = ValidityFlag::new().with_access_mode(AccessMode::Read);
        // SAFETY: value outlives the borrows within this test scope.
        let storage = unsafe { ValueStorage::borrowed(&mut value as *mut AssetIndex, validity) };
        let first = PyAssetIndex::from_borrowed(storage.clone());
        let second = PyAssetIndex::from_borrowed(storage);

        assert!(first.to_bevy().is_err());
        assert_eq!(first, second);
        assert_eq!(first, first.clone());
    }

    #[test]
    fn distinct_expired_borrows_stay_unequal() {
        let mut left = AssetIndex::from_bits(7);
        let mut right = AssetIndex::from_bits(7);
        let validity = ValidityFlag::new().with_access_mode(AccessMode::Read);
        // SAFETY: values outlive the borrows within this test scope.
        let first = PyAssetIndex::from_borrowed(unsafe {
            ValueStorage::borrowed(&mut left as *mut AssetIndex, validity.clone())
        });
        // SAFETY: values outlive the borrows within this test scope.
        let second = PyAssetIndex::from_borrowed(unsafe {
            ValueStorage::borrowed(&mut right as *mut AssetIndex, validity)
        });

        assert_ne!(first, second);
    }
}
