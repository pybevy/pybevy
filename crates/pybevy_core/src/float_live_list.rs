//! Live Python list protocol for a `Vec<f32>` component field.
//!
//! Backs `CascadeShadowConfig.bounds` and other scalar live lists. The shared
//! `impl_live_scalar_list!` macro in [`crate::live_sequence`] generates the
//! Python sequence methods; this module holds the concrete wrapper so its
//! behavior can be unit-tested beside the type.

use pybevy_storage::{FieldStorage, FieldStorageInner};
use pyo3::{exceptions::PyValueError, prelude::*, types::PyList};

#[pyclass(name = "_FloatLiveList", skip_from_py_object)]
#[derive(Clone)]
pub struct PyFloatLiveList {
    storage: FieldStorage<Vec<f32>>,
    minimum_length: Option<(usize, &'static str)>,
}

impl PyFloatLiveList {
    fn from_storage(storage: FieldStorage<Vec<f32>>) -> Self {
        Self {
            storage,
            minimum_length: None,
        }
    }

    pub fn with_minimum_length(mut self, minimum: usize, error: &'static str) -> Self {
        self.minimum_length = Some((minimum, error));
        self
    }

    fn validate_length_after_removal(&self, length: usize) -> PyResult<()> {
        let Some((minimum, error)) = self.minimum_length else {
            return Ok(());
        };
        if length >= minimum {
            return Ok(());
        }
        match &self.storage.inner {
            FieldStorageInner::OwnedReadOnly { .. } | FieldStorageInner::BorrowedRef(_) => {
                return Ok(());
            }
            FieldStorageInner::Revalidating(field) if field.check_write_access().is_err() => {
                return Ok(());
            }
            _ => {}
        }
        Err(PyValueError::new_err(error))
    }
}

crate::impl_live_scalar_list!(PyFloatLiveList, "_FloatLiveList", Vec<f32>, f32);

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    fn list(values: Vec<f32>) -> PyFloatLiveList {
        PyFloatLiveList::from_storage(FieldStorage::owned(values))
    }

    #[test]
    fn scalar_list_protocol_mutates_and_reads_back() {
        let mut list = list(vec![1.0, 2.0, 3.0]);

        assert_eq!(list.__len__().unwrap(), 3);
        assert_eq!(list.__getitem__(0).unwrap(), 1.0);
        assert_eq!(list.__getitem__(-1).unwrap(), 3.0);
        assert!(list.__getitem__(-4).is_err());

        list.__setitem__(0, 9.0).unwrap();
        assert_eq!(list.__getitem__(0).unwrap(), 9.0);
        assert!(list.__setitem__(5, 0.0).is_err());

        list.append(4.0).unwrap();
        list.append(5.0).unwrap();
        list.append(6.0).unwrap();
        list.insert(0, 0.0).unwrap();
        assert_eq!(
            list.to_list().unwrap(),
            vec![0.0, 9.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
        assert_eq!(list.__repr__().unwrap(), "_FloatLiveList(len=7)");

        assert_eq!(list.pop(-1).unwrap(), 6.0);
        list.__delitem__(0).unwrap();
        assert_eq!(list.to_list().unwrap(), vec![9.0, 2.0, 3.0, 4.0, 5.0]);

        list.clear().unwrap();
        assert_eq!(list.__len__().unwrap(), 0);
        assert!(list.pop(0).is_err());
    }

    #[test]
    fn minimum_length_rejects_removals_without_mutating() {
        let mut list = list(vec![1.0, 2.0]).with_minimum_length(1, "bounds cannot be empty");
        assert_eq!(list.pop(-1).unwrap(), 2.0);
        Python::attach(|py| {
            assert!(list.pop(-1).unwrap_err().is_instance_of::<PyValueError>(py));
            assert!(
                list.__delitem__(0)
                    .unwrap_err()
                    .is_instance_of::<PyValueError>(py)
            );
            assert!(list.clear().unwrap_err().is_instance_of::<PyValueError>(py));
        });
        assert_eq!(list.to_list().unwrap(), vec![1.0]);
    }

    #[test]
    fn scalar_list_iterates_through_python() {
        Python::attach(|py| {
            let list = list(vec![1.0, 2.0]);
            let iterator = list.__iter__(py).unwrap();
            let first: f64 = iterator
                .bind(py)
                .call_method0("__next__")
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(first, 1.0);
        });
    }
}
