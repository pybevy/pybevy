use std::num::NonZero;

use bevy::app::AppExit;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

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
