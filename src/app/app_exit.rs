use std::num::NonZero;

use bevy::app::AppExit;
use pybevy_core::domain::{FieldDomain, IntegerInput, check_for};
use pybevy_macros::pyenum;
use pyo3::{prelude::*, types::PyInt};

const EXIT_CODE_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: 0.0, hi: 255.0 }];
const ERROR_CODE_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: 1.0, hi: 255.0 }];

pub struct AppExitCode(NonZero<u8>);

impl FromPyObject<'_, '_> for AppExitCode {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let code = value.extract::<IntegerInput<u32>>()?;
        let code =
            check_for::<PyAppExit, _>("Error.value", code, ERROR_CODE_DOMAIN)?.stored() as u8;
        Ok(Self(
            NonZero::new(code).expect("validated nonzero exit code"),
        ))
    }
}

impl From<NonZero<u8>> for AppExitCode {
    fn from(code: NonZero<u8>) -> Self {
        Self(code)
    }
}

impl From<AppExitCode> for NonZero<u8> {
    fn from(code: AppExitCode) -> Self {
        code.0
    }
}

impl<'py> IntoPyObject<'py> for AppExitCode {
    type Target = PyInt;
    type Output = Bound<'py, PyInt>;
    type Error = <u8 as IntoPyObject<'py>>::Error;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        self.0.get().into_pyobject(py)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum AppExitValue {
    Success,
    Error { value: NonZero<u8> },
}

impl From<&AppExit> for AppExitValue {
    fn from(value: &AppExit) -> Self {
        match value {
            AppExit::Success => Self::Success,
            AppExit::Error(code) => Self::Error { value: *code },
        }
    }
}

impl From<AppExitValue> for AppExit {
    fn from(value: AppExitValue) -> Self {
        match value {
            AppExitValue::Success => Self::Success,
            AppExitValue::Error { value: code } => Self::Error(code),
        }
    }
}

#[pyenum(AppExit, message, writable, mirror = AppExitValue)]
#[pyclass(module = "pybevy.app", name = "AppExit")]
pub enum PyAppExit {
    Success(),
    #[py_bevy(tuple)]
    Error {
        #[py_type(AppExitCode)]
        value: NonZero<u8>,
    },
}

impl TryFrom<&PyAppExit> for AppExit {
    type Error = PyErr;

    fn try_from(value: &PyAppExit) -> PyResult<Self> {
        Ok(value.inner.clone().into())
    }
}

#[pymethods]
impl PyAppExit {
    #[staticmethod]
    pub fn error(py: Python<'_>) -> PyResult<Py<PyAny>> {
        materialize_app_exit(py, &AppExit::error())
    }

    #[staticmethod]
    pub fn from_code(py: Python<'_>, code: IntegerInput<u32>) -> PyResult<Py<PyAny>> {
        let code = check_for::<Self, _>("code", code, EXIT_CODE_DOMAIN)?.stored() as u8;
        materialize_app_exit(py, &AppExit::from_code(code))
    }

    pub fn is_success(&self) -> bool {
        matches!(self.inner, AppExitValue::Success)
    }

    pub fn is_error(&self) -> bool {
        matches!(self.inner, AppExitValue::Error { .. })
    }
}
