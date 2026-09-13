use std::convert::Infallible;

use pyo3::{
    exceptions::{PySystemError, PyTypeError},
    prelude::*,
};

use super::{check_complete_form, check_conflict};
use crate::public_error;

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
}
