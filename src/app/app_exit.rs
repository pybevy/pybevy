use std::num::NonZero;

use bevy::app::AppExit;
use pybevy_core::domain::{FieldDomain, IntegerInput, check_for};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

const EXIT_CODE_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: 0.0, hi: 255.0 }];

#[derive(Debug, Clone, PartialEq)]
enum AppExitValue {
    Success,
    Error { code: NonZero<u8> },
}

impl From<&AppExit> for AppExitValue {
    fn from(value: &AppExit) -> Self {
        match value {
            AppExit::Success => Self::Success,
            AppExit::Error(code) => Self::Error { code: *code },
        }
    }
}

impl From<AppExitValue> for AppExit {
    fn from(value: AppExitValue) -> Self {
        match value {
            AppExitValue::Success => Self::Success,
            AppExitValue::Error { code } => Self::Error(code),
        }
    }
}

#[pyenum(AppExit, message, writable, mirror = AppExitValue)]
#[pyclass(module = "pybevy.app", name = "AppExit")]
pub enum PyAppExit {
    Success(),
    #[py_bevy(tuple)]
    Error {
        code: NonZero<u8>,
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
