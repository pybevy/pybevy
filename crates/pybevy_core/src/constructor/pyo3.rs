use std::convert::Infallible;

use pyo3::{
    PyClass,
    exceptions::{PySystemError, PyTypeError},
    prelude::*,
};

use super::{Omittable, check_complete_form, check_conflict};
use crate::{domain::DurationInput, public_error};

pub fn reject_explicit_none(obj: &Borrowed<'_, '_, PyAny>, expected: &str) -> PyResult<()> {
    if obj.is_none() {
        return Err(PyTypeError::new_err(public_error::omitted_argument_none(
            expected,
        )));
    }
    Ok(())
}

impl<'a, 'py, T: PyClass> FromPyObject<'a, 'py> for Omittable<Py<T>> {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        reject_explicit_none(&obj, <T as PyClass>::NAME)?;
        Ok(Self::Supplied(obj.cast::<T>()?.to_owned().unbind()))
    }
}

impl<'a, 'py, T: PyClass> FromPyObject<'a, 'py> for Omittable<PyRef<'py, T>> {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        reject_explicit_none(&obj, <T as PyClass>::NAME)?;
        Ok(Self::Supplied(obj.cast::<T>()?.try_borrow()?))
    }
}

impl<'a, 'py, T> FromPyObject<'a, 'py> for Omittable<Option<T>>
where
    T: FromPyObject<'a, 'py>,
    T::Error: Into<PyErr>,
{
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        if obj.is_none() {
            return Ok(Self::Supplied(None));
        }
        Ok(Self::Supplied(Some(T::extract(obj).map_err(Into::into)?)))
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for Omittable<Bound<'py, PyAny>> {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        reject_explicit_none(&obj, "a value")?;
        Ok(Self::Supplied(obj.to_owned()))
    }
}

impl<'a, 'py, T> FromPyObject<'a, 'py> for Omittable<Vec<T>>
where
    Vec<T>: FromPyObject<'a, 'py, Error = PyErr>,
{
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        reject_explicit_none(&obj, "list")?;
        Ok(Self::Supplied(obj.extract()?))
    }
}

/// An argument type that a crate downstream of this one takes as
/// `Omittable<T>`; the orphan rule keeps that crate from implementing the
/// extraction itself.
pub trait OmittableArg {
    /// What explicit `None` is rejected in favour of.
    const EXPECTED: &'static str;
}

impl OmittableArg for DurationInput {
    const EXPECTED: &'static str = "Duration, float (seconds), or int (seconds)";
}

impl<'a, 'py, T> FromPyObject<'a, 'py> for Omittable<T>
where
    T: OmittableArg + FromPyObject<'a, 'py, Error = PyErr>,
{
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        reject_explicit_none(&obj, T::EXPECTED)?;
        Ok(Self::Supplied(T::extract(obj)?))
    }
}

pub enum RawArg<'py> {
    Missing,
    Supplied(Bound<'py, PyAny>),
}

impl<'a, 'py> FromPyObject<'a, 'py> for RawArg<'py> {
    type Error = Infallible;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        Ok(Self::Supplied(obj.to_owned()))
    }
}

impl<'py> RawArg<'py> {
    pub fn is_supplied(&self) -> bool {
        matches!(self, Self::Supplied(_))
    }

    pub fn optional<T>(&self, ty: &str, param: &str, expected: &str) -> PyResult<Option<T>>
    where
        T: for<'a> FromPyObject<'a, 'py>,
    {
        let Self::Supplied(obj) = self else {
            return Ok(None);
        };
        obj.extract::<T>().map(Some).map_err(|error| {
            let error: PyErr = error.into();
            if error.is_instance_of::<PyTypeError>(obj.py()) {
                let got = obj
                    .get_type()
                    .name()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|_| "unknown".to_owned());
                PyTypeError::new_err(public_error::constructor_argument_type(
                    ty, param, expected, &got,
                ))
            } else {
                error
            }
        })
    }

    pub fn required<T>(&self, ty: &str, param: &str, expected: &str) -> PyResult<T>
    where
        T: for<'a> FromPyObject<'a, 'py>,
    {
        self.optional(ty, param, expected)?
            .ok_or_else(|| PySystemError::new_err("selected constructor member was not supplied"))
    }
}

pub fn reject_conflicting_forms(
    ty: &str,
    native: &[(&str, &RawArg<'_>)],
    alternate: &[(&str, &RawArg<'_>)],
) -> PyResult<()> {
    let native: Vec<_> = native
        .iter()
        .map(|(name, arg)| (*name, arg.is_supplied()))
        .collect();
    let alternate: Vec<_> = alternate
        .iter()
        .map(|(name, arg)| (*name, arg.is_supplied()))
        .collect();
    check_conflict(ty, &native, &alternate).map_err(PyTypeError::new_err)
}

pub fn require_complete_form(ty: &str, members: &[(&str, &RawArg<'_>)]) -> PyResult<()> {
    let members: Vec<_> = members
        .iter()
        .map(|(name, arg)| (*name, arg.is_supplied()))
        .collect();
    check_complete_form(ty, &members).map_err(PyTypeError::new_err)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use pyo3::types::PyList;

    use super::*;

    #[test]
    fn none_is_supplied_not_missing() {
        Python::attach(|py| {
            let none = py.None().into_bound(py);
            let extracted: Result<RawArg<'_>, Infallible> =
                FromPyObject::extract(none.as_borrowed());
            assert!(extracted.unwrap().is_supplied());
            assert!(!RawArg::Missing.is_supplied());
        });
    }

    #[test]
    fn omittable_vec_rejects_none_and_extracts_items() {
        Python::attach(|py| {
            let none = py.None().into_bound(py);
            let error = Omittable::<Vec<u32>>::extract(none.as_borrowed()).unwrap_err();
            assert!(error.is_instance_of::<PyTypeError>(py));

            let list = PyList::new(py, [1u32, 2]).unwrap();
            let extracted = Omittable::<Vec<u32>>::extract(list.as_any().as_borrowed()).unwrap();
            assert_eq!(extracted.into_option(), Some(vec![1, 2]));
        });
    }

    #[derive(Debug, PartialEq)]
    struct Count(u8);

    impl OmittableArg for Count {
        const EXPECTED: &'static str = "Count";
    }

    impl FromPyObject<'_, '_> for Count {
        type Error = PyErr;

        fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
            Ok(Self(obj.extract()?))
        }
    }

    #[test]
    fn omittable_arg_rejects_none_by_name_and_extracts_the_value() {
        Python::attach(|py| {
            let none = py.None().into_bound(py);
            let error = Omittable::<Count>::extract(none.as_borrowed()).unwrap_err();
            assert!(error.is_instance_of::<PyTypeError>(py));
            assert_eq!(
                error.value(py).to_string(),
                public_error::omitted_argument_none("Count")
            );

            let value = 7u8.into_pyobject(py).unwrap();
            let extracted = Omittable::<Count>::extract(value.as_any().as_borrowed()).unwrap();
            assert_eq!(extracted.into_option(), Some(Count(7)));
        });
    }
}
