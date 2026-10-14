use bevy::window::{CursorIcon, SystemCursorIcon};
use pybevy_core::{ComponentStorage, enum_comparison::compare_values};
use pybevy_macros::pyenum;
use pyo3::{prelude::*, pyclass::CompareOp};

use crate::cursor::PySystemCursorIcon;

#[pyenum(CursorIcon, component, manual_comparison)]
#[pyclass(name = "CursorIcon", module = "pybevy.window")]
pub enum PyCursorIcon {
    #[py_bevy(tuple)]
    System {
        #[py_type(PySystemCursorIcon)]
        #[py_set]
        value: SystemCursorIcon,
    },
}

#[pymethods]
impl PyCursorIcon {
    #[getter]
    pub fn as_system(&self) -> PyResult<Option<PySystemCursorIcon>> {
        Ok(self.as_ref()?.as_system().copied().map(Into::into))
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<Py<PyAny>> {
        compare_values(self, other, op, |left, right| {
            Ok(left.as_ref()? == right.as_ref()?)
        })
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let CursorIcon::System(icon) = &*self.as_ref()?;
        Ok(format!(
            "CursorIcon.System({})",
            PySystemCursorIcon::from(*icon).__repr__()
        ))
    }
}
