use std::collections::HashMap;

use bevy::input::gamepad::{AxisSettings, ButtonAxisSettings, ButtonSettings, GamepadSettings};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

#[pyclass(name = "ButtonSettings", frozen)]
#[derive(Debug, Clone)]
pub struct PyButtonSettings {
    pub(crate) inner: ButtonSettings,
}

impl From<&ButtonSettings> for PyButtonSettings {
    fn from(settings: &ButtonSettings) -> Self {
        PyButtonSettings {
            inner: settings.clone(),
        }
    }
}

#[pymethods]
impl PyButtonSettings {
    #[new]
    #[pyo3(signature = (press_threshold=0.75, release_threshold=0.65))]
    pub fn new(press_threshold: f32, release_threshold: f32) -> PyResult<Self> {
        ButtonSettings::new(press_threshold, release_threshold)
            .map(|inner| PyButtonSettings { inner })
            .map_err(|e| PyValueError::new_err(format!("{:?}", e)))
    }

    #[getter]
    pub fn press_threshold(&self) -> f32 {
        self.inner.press_threshold()
    }

    #[getter]
    pub fn release_threshold(&self) -> f32 {
        self.inner.release_threshold()
    }

    pub fn is_pressed(&self, value: f32) -> bool {
        self.inner.is_pressed(value)
    }

    pub fn is_released(&self, value: f32) -> bool {
        self.inner.is_released(value)
    }

    fn __repr__(&self) -> String {
        format!(
            "ButtonSettings(press={:.2}, release={:.2})",
            self.inner.press_threshold(),
            self.inner.release_threshold()
        )
    }
}

#[pyclass(name = "AxisSettings", frozen)]
#[derive(Debug, Clone)]
pub struct PyAxisSettings {
    pub(crate) inner: AxisSettings,
}

impl From<&AxisSettings> for PyAxisSettings {
    fn from(settings: &AxisSettings) -> Self {
        PyAxisSettings {
            inner: settings.clone(),
        }
    }
}

#[pymethods]
impl PyAxisSettings {
    #[new]
    #[pyo3(signature = (
        livezone_lowerbound=-1.0,
        deadzone_lowerbound=-0.05,
        deadzone_upperbound=0.05,
        livezone_upperbound=1.0,
        threshold=0.01
    ))]
    pub fn new(
        livezone_lowerbound: f32,
        deadzone_lowerbound: f32,
        deadzone_upperbound: f32,
        livezone_upperbound: f32,
        threshold: f32,
    ) -> PyResult<Self> {
        AxisSettings::new(
            livezone_lowerbound,
            deadzone_lowerbound,
            deadzone_upperbound,
            livezone_upperbound,
            threshold,
        )
        .map(|inner| PyAxisSettings { inner })
        .map_err(|e| PyValueError::new_err(format!("{:?}", e)))
    }

    #[getter]
    pub fn livezone_upperbound(&self) -> f32 {
        self.inner.livezone_upperbound()
    }

    #[getter]
    pub fn deadzone_upperbound(&self) -> f32 {
        self.inner.deadzone_upperbound()
    }

    #[getter]
    pub fn deadzone_lowerbound(&self) -> f32 {
        self.inner.deadzone_lowerbound()
    }

    #[getter]
    pub fn livezone_lowerbound(&self) -> f32 {
        self.inner.livezone_lowerbound()
    }

    #[getter]
    pub fn threshold(&self) -> f32 {
        self.inner.threshold()
    }

    pub fn clamp(&self, value: f32) -> f32 {
        self.inner.clamp(value)
    }

    fn __repr__(&self) -> String {
        format!(
            "AxisSettings(deadzone=[{:.2}, {:.2}], livezone=[{:.2}, {:.2}], threshold={:.2})",
            self.inner.deadzone_lowerbound(),
            self.inner.deadzone_upperbound(),
            self.inner.livezone_lowerbound(),
            self.inner.livezone_upperbound(),
            self.inner.threshold()
        )
    }
}

#[pyclass(name = "ButtonAxisSettings", frozen)]
#[derive(Debug, Clone)]
pub struct PyButtonAxisSettings {
    pub(crate) inner: ButtonAxisSettings,
}

impl From<&ButtonAxisSettings> for PyButtonAxisSettings {
    fn from(settings: &ButtonAxisSettings) -> Self {
        PyButtonAxisSettings {
            inner: settings.clone(),
        }
    }
}

#[pymethods]
impl PyButtonAxisSettings {
    #[new]
    #[pyo3(signature = (high=0.95, low=0.05, threshold=0.01))]
    pub fn new(high: f32, low: f32, threshold: f32) -> Self {
        PyButtonAxisSettings {
            inner: ButtonAxisSettings {
                high,
                low,
                threshold,
            },
        }
    }

    #[getter]
    pub fn high(&self) -> f32 {
        self.inner.high
    }

    #[getter]
    pub fn low(&self) -> f32 {
        self.inner.low
    }

    #[getter]
    pub fn threshold(&self) -> f32 {
        self.inner.threshold
    }

    fn __repr__(&self) -> String {
        format!(
            "ButtonAxisSettings(high={:.2}, low={:.2}, threshold={:.2})",
            self.inner.high, self.inner.low, self.inner.threshold
        )
    }
}

#[pycomponent(GamepadSettings, no_clone, bridge)]
#[pyclass(name = "GamepadSettings", extends = PyComponent, frozen)]
pub struct PyGamepadSettings {
    pub(crate) storage: ComponentStorage<GamepadSettings>,
}

#[pymethods]
impl PyGamepadSettings {
    pub fn button_settings_for(&self, button: PyGamepadButton) -> PyResult<PyButtonSettings> {
        let settings = self.as_ref()?;
        Ok(settings.get_button_settings(button.into()).into())
    }

    pub fn axis_settings_for(&self, axis: PyGamepadAxis) -> PyResult<PyAxisSettings> {
        let settings = self.as_ref()?;
        Ok(settings.get_axis_settings(axis.into()).into())
    }

    pub fn button_axis_settings_for(
        &self,
        button: PyGamepadButton,
    ) -> PyResult<PyButtonAxisSettings> {
        let settings = self.as_ref()?;
        Ok(settings.get_button_axis_settings(button.into()).into())
    }

    #[getter]
    pub fn default_button_settings(&self) -> PyResult<PyButtonSettings> {
        let settings = self.as_ref()?;
        Ok((&settings.default_button_settings).into())
    }

    #[getter]
    pub fn default_axis_settings(&self) -> PyResult<PyAxisSettings> {
        let settings = self.as_ref()?;
        Ok((&settings.default_axis_settings).into())
    }

    #[getter]
    pub fn default_button_axis_settings(&self) -> PyResult<PyButtonAxisSettings> {
        let settings = self.as_ref()?;
        Ok((&settings.default_button_axis_settings).into())
    }

    #[getter]
    pub fn button_settings(&self) -> PyResult<HashMap<PyGamepadButton, PyButtonSettings>> {
        let settings = self.as_ref()?;
        Ok(settings
            .button_settings
            .iter()
            .map(|(k, v)| ((*k).into(), v.into()))
            .collect())
    }

    #[getter]
    pub fn axis_settings(&self) -> PyResult<HashMap<PyGamepadAxis, PyAxisSettings>> {
        let settings = self.as_ref()?;
        Ok(settings
            .axis_settings
            .iter()
            .map(|(k, v)| ((*k).into(), v.into()))
            .collect())
    }

    #[getter]
    pub fn button_axis_settings(&self) -> PyResult<HashMap<PyGamepadButton, PyButtonAxisSettings>> {
        let settings = self.as_ref()?;
        Ok(settings
            .button_axis_settings
            .iter()
            .map(|(k, v)| ((*k).into(), v.into()))
            .collect())
    }

    fn __repr__(&self) -> &'static str {
        "GamepadSettings"
    }
}
