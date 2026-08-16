use bevy::window::CursorOptions;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::cursor::PyCursorGrabMode;

#[pycomponent(CursorOptions, bridge)]
#[pyclass(name = "CursorOptions", extends = PyComponent)]
pub struct PyCursorOptions {
    pub(crate) storage: ComponentStorage<CursorOptions>,
}

#[pymethods]
impl PyCursorOptions {
    #[new]
    #[pyo3(signature = (visible = true, grab_mode = PyCursorGrabMode::None, hit_test = true))]
    pub fn new(
        visible: bool,
        grab_mode: PyCursorGrabMode,
        hit_test: bool,
    ) -> PyClassInitializer<Self> {
        Self::from_owned(CursorOptions {
            visible,
            grab_mode: grab_mode.into(),
            hit_test,
        })
        .into()
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
