use bevy::window::CursorOptions;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

use crate::cursor::PyCursorGrabMode;

#[component_storage(CursorOptions)]
#[pyclass(name = "CursorOptions", extends = PyComponent)]
#[derive(Clone)]
pub struct PyCursorOptions {
    pub(crate) storage: ComponentStorage<CursorOptions>,
}

#[pymethods]
impl PyCursorOptions {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(CursorOptions::default())
    }

    #[getter]
    pub fn visible(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.visible)
    }

    #[setter]
    pub fn set_visible(&mut self, visible: bool) -> PyResult<()> {
        self.as_mut()?.visible = visible;
        Ok(())
    }

    #[getter]
    pub fn grab_mode(&self) -> PyResult<PyCursorGrabMode> {
        Ok(self.as_ref()?.grab_mode.into())
    }

    #[setter]
    pub fn set_grab_mode(&mut self, mode: PyCursorGrabMode) -> PyResult<()> {
        self.as_mut()?.grab_mode = mode.into();
        Ok(())
    }

    #[getter]
    pub fn hit_test(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.hit_test)
    }

    #[setter]
    pub fn set_hit_test(&mut self, hit_test: bool) -> PyResult<()> {
        self.as_mut()?.hit_test = hit_test;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let cursor = self.as_ref()?;
        Ok(format!(
            "CursorOptions(visible={}, grab_mode={:?}, hit_test={})",
            cursor.visible, cursor.grab_mode, cursor.hit_test
        ))
    }
}
