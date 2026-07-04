use bevy::winit::WinitSettings;
use pyo3::prelude::*;

use crate::update_mode::PyUpdateMode;
#[pyclass(name = "WinitSettings", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PyWinitSettings(pub(crate) WinitSettings);

#[pymethods]
impl PyWinitSettings {
    #[new]
    #[pyo3(signature = (focused_mode = None, unfocused_mode = None))]
    pub fn new(focused_mode: Option<PyUpdateMode>, unfocused_mode: Option<PyUpdateMode>) -> Self {
        let defaults = WinitSettings::default();
        PyWinitSettings(WinitSettings {
            focused_mode: focused_mode
                .map(Into::into)
                .unwrap_or(defaults.focused_mode),
            unfocused_mode: unfocused_mode
                .map(Into::into)
                .unwrap_or(defaults.unfocused_mode),
        })
    }

    #[staticmethod]
    pub fn game() -> Self {
        PyWinitSettings(WinitSettings::game())
    }

    #[staticmethod]
    pub fn desktop_app() -> Self {
        PyWinitSettings(WinitSettings::desktop_app())
    }

    #[staticmethod]
    pub fn continuous() -> Self {
        PyWinitSettings(WinitSettings::continuous())
    }

    #[getter]
    pub fn focused_mode(&self) -> PyUpdateMode {
        self.0.focused_mode.into()
    }

    #[getter]
    pub fn unfocused_mode(&self) -> PyUpdateMode {
        self.0.unfocused_mode.into()
    }

    pub fn __repr__(&self) -> String {
        format!(
            "WinitSettings(focused_mode={}, unfocused_mode={})",
            PyUpdateMode::from(self.0.focused_mode).__repr__(),
            PyUpdateMode::from(self.0.unfocused_mode).__repr__(),
        )
    }
}

impl From<PyWinitSettings> for WinitSettings {
    fn from(val: PyWinitSettings) -> Self {
        val.0
    }
}

impl From<WinitSettings> for PyWinitSettings {
    fn from(val: WinitSettings) -> Self {
        PyWinitSettings(val)
    }
}
