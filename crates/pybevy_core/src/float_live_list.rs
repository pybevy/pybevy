//! Live Python list protocol for a `Vec<f32>` component field.
//!
//! Backs `CascadeShadowConfig.bounds` and other scalar live lists. The shared
//! `impl_live_scalar_list!` macro in [`crate::live_sequence`] generates the
//! Python sequence methods; this module holds the concrete wrapper so its
//! behavior can be unit-tested beside the type.

use pybevy_storage::FieldStorage;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyList, PyString},
};

use crate::public_error;

#[pyclass(
    name = "_FloatLiveList",
    module = "pybevy.collections",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFloatLiveList {
    storage: FieldStorage<Vec<f32>>,
    length_rule: LengthRule,
}

#[derive(Clone, Copy)]
enum LengthRule {
    Any,
    Minimum(usize, &'static str),
    Fixed(&'static str),
}

impl PyFloatLiveList {
    fn from_storage(storage: FieldStorage<Vec<f32>>) -> Self {
        Self {
            storage,
            length_rule: LengthRule::Any,
        }
    }

    pub fn with_minimum_length(mut self, minimum: usize, error: &'static str) -> Self {
        self.length_rule = LengthRule::Minimum(minimum, error);
        self
    }

    /// Reject every length change; `name` identifies the field in the error.
    pub fn with_fixed_length(mut self, name: &'static str) -> Self {
        self.length_rule = LengthRule::Fixed(name);
        self
    }

    fn validate_resize(&self, length: usize) -> PyResult<()> {
        let error = match self.length_rule {
            LengthRule::Any => return Ok(()),
            LengthRule::Minimum(minimum, _) if length >= minimum => return Ok(()),
            LengthRule::Minimum(_, error) => error.to_string(),
            LengthRule::Fixed(name) => {
                if length == self.storage.as_ref()?.len() {
                    return Ok(());
                }
                public_error::fixed_length_collection(name)
            }
        };
        // Storage without write authority reports that error from the write itself.
        if self.storage.check_write_access().is_err() {
            return Ok(());
        }
        Err(PyValueError::new_err(error))
    }
}

crate::impl_live_scalar_list!(PyFloatLiveList, "_FloatLiveList", Vec<f32>, f32);

impl TryFrom<PyFloatLiveList> for Vec<f32> {
    type Error = PyErr;

    fn try_from(value: PyFloatLiveList) -> PyResult<Self> {
        Ok(value.storage.as_ref()?.clone())
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for PyFloatLiveList {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let values = if let Ok(list) = obj.cast::<Self>() {
            list.borrow().to_list()?
        } else if obj.is_instance_of::<PyString>() {
            return Err(PyTypeError::new_err(public_error::FLOAT_SEQUENCE_NOT_STR));
        } else {
            obj.try_iter()?
                .map(|item| item?.extract::<f32>())
                .collect::<PyResult<Vec<_>>>()?
        };
        Ok(Self::from_storage(FieldStorage::owned(values)))
    }
}

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
        assert_eq!(list.item(0).unwrap(), 1.0);
        assert_eq!(list.item(-1).unwrap(), 3.0);
        assert!(list.item(-4).is_err());

        list.__setitem__(0, 9.0).unwrap();
        assert_eq!(list.item(0).unwrap(), 9.0);
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
    fn fixed_length_rejects_every_resize_without_mutating() {
        let mut list = list(vec![1.0, 2.0]).with_fixed_length("Test.values");
        list.__setitem__(-1, 5.0).unwrap();
        Python::attach(|py| {
            let bound = Bound::new(py, list.clone()).unwrap();
            let expected = public_error::fixed_length_collection("Test.values");
            let errors = [
                list.append(3.0).unwrap_err(),
                list.insert(0, 3.0).unwrap_err(),
                list.pop(-1).unwrap_err(),
                list.__delitem__(0).unwrap_err(),
                list.clear().unwrap_err(),
                PyFloatLiveList::extend(&bound, PyList::new(py, [3.0]).unwrap().as_any())
                    .unwrap_err(),
            ];
            for error in errors {
                assert!(error.is_instance_of::<PyValueError>(py));
                assert_eq!(error.value(py).to_string(), expected);
            }
            PyFloatLiveList::extend(&bound, PyList::empty(py).as_any()).unwrap();
        });
        assert_eq!(list.to_list().unwrap(), vec![1.0, 5.0]);
    }

    #[test]
    fn extraction_copies_iterables_and_rejects_str() {
        Python::attach(|py| {
            let source = PyList::new(py, [1.0, 2.5]).unwrap();
            let list: PyFloatLiveList = source.extract().unwrap();
            source.set_item(0, 9.0).unwrap();
            assert_eq!(Vec::<f32>::try_from(list).unwrap(), vec![1.0, 2.5]);

            let Err(error) = PyString::new(py, "12").extract::<PyFloatLiveList>() else {
                panic!("str must not extract as a float list");
            };
            assert!(error.is_instance_of::<PyTypeError>(py));
        });
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
