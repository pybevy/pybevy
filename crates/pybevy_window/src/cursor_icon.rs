use bevy::window::CursorIcon;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

use crate::cursor::PySystemCursorIcon;

#[component_storage(CursorIcon, bridge)]
#[pyclass(name = "CursorIcon", extends = PyComponent)]
#[derive(Clone)]
pub struct PyCursorIcon {
    pub(crate) storage: ComponentStorage<CursorIcon>,
}

#[pymethods]
impl PyCursorIcon {
    #[new]
    #[pyo3(signature = (system_icon = PySystemCursorIcon::Default))]
    pub fn new(system_icon: PySystemCursorIcon) -> (Self, PyComponent) {
        Self::from_owned(CursorIcon::System(system_icon.into()))
    }

    #[staticmethod]
    pub fn system(py: Python, icon: PySystemCursorIcon) -> PyResult<Py<Self>> {
        let (instance, base) = Self::from_owned(CursorIcon::System(icon.into()));
        Py::new(py, (instance, base))
    }

    #[getter]
    pub fn as_system(&self) -> PyResult<Option<PySystemCursorIcon>> {
        Ok(self.as_ref()?.as_system().copied().map(Into::into))
    }

    pub fn set_system(&mut self, icon: PySystemCursorIcon) -> PyResult<()> {
        *self.as_mut()? = CursorIcon::System(icon.into());
        Ok(())
    }

    pub fn is_system(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.as_system().is_some())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let cursor = self.as_ref()?;
        match cursor.as_system() {
            Some(icon) => Ok(format!("CursorIcon.System({:?})", icon)),
            None => Ok("CursorIcon.Custom(...)".to_string()),
        }
    }
}
