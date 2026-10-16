use std::collections::HashMap;

use bevy::input::gamepad::{AxisSettings, ButtonAxisSettings, ButtonSettings, GamepadSettings};
use pybevy_core::{
    ComponentStorage, PyComponent,
    domain::{FINITE_NON_NEGATIVE, FieldDomain, check_for, check_ordered_for},
    public_error,
    repr::float_repr,
};
use pybevy_macros::pycomponent;
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyDict, PyModule},
};

use crate::{gamepad_axis::PyGamepadAxis, gamepad_button::PyGamepadButton};

// The ranges Bevy's ButtonSettings and AxisSettings constructors enforce.
const UNIT_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: 0.0, hi: 1.0 }];
const NEGATIVE_UNIT_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: -1.0, hi: 0.0 }];
const AXIS_THRESHOLD_DOMAIN: &[FieldDomain] = &[FieldDomain::RangeInclusive { lo: 0.0, hi: 2.0 }];

#[pyclass(
    name = "ButtonSettings",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
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
        check_for::<Self, _>("press_threshold", press_threshold, UNIT_DOMAIN)?;
        check_for::<Self, _>("release_threshold", release_threshold, UNIT_DOMAIN)?;
        check_ordered_for::<Self, _>(
            "release_threshold",
            release_threshold,
            "press_threshold",
            press_threshold,
            false,
        )?;
        ButtonSettings::new(press_threshold, release_threshold)
            .map(|inner| PyButtonSettings { inner })
            .map_err(|e| PyValueError::new_err(public_error::gamepad_settings_failed("button", e)))
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
            "ButtonSettings(press_threshold={}, release_threshold={})",
            float_repr(self.inner.press_threshold()),
            float_repr(self.inner.release_threshold())
        )
    }
}

#[pyclass(
    name = "AxisSettings",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
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
        check_for::<Self, _>(
            "livezone_lowerbound",
            livezone_lowerbound,
            NEGATIVE_UNIT_DOMAIN,
        )?;
        check_for::<Self, _>(
            "deadzone_lowerbound",
            deadzone_lowerbound,
            NEGATIVE_UNIT_DOMAIN,
        )?;
        check_for::<Self, _>("deadzone_upperbound", deadzone_upperbound, UNIT_DOMAIN)?;
        check_for::<Self, _>("livezone_upperbound", livezone_upperbound, UNIT_DOMAIN)?;
        check_for::<Self, _>("threshold", threshold, AXIS_THRESHOLD_DOMAIN)?;
        check_ordered_for::<Self, _>(
            "livezone_lowerbound",
            livezone_lowerbound,
            "deadzone_lowerbound",
            deadzone_lowerbound,
            false,
        )?;
        check_ordered_for::<Self, _>(
            "deadzone_upperbound",
            deadzone_upperbound,
            "livezone_upperbound",
            livezone_upperbound,
            false,
        )?;
        AxisSettings::new(
            livezone_lowerbound,
            deadzone_lowerbound,
            deadzone_upperbound,
            livezone_upperbound,
            threshold,
        )
        .map(|inner| PyAxisSettings { inner })
        .map_err(|e| PyValueError::new_err(public_error::gamepad_settings_failed("axis", e)))
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
            "AxisSettings(livezone_lowerbound={}, deadzone_lowerbound={}, deadzone_upperbound={}, livezone_upperbound={}, threshold={})",
            float_repr(self.inner.livezone_lowerbound()),
            float_repr(self.inner.deadzone_lowerbound()),
            float_repr(self.inner.deadzone_upperbound()),
            float_repr(self.inner.livezone_upperbound()),
            float_repr(self.inner.threshold())
        )
    }
}

#[pyclass(
    name = "ButtonAxisSettings",
    module = "pybevy.input",
    frozen,
    from_py_object
)]
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
    #[pyo3(signature = (*, high=0.95, low=0.05, threshold=0.01))]
    pub fn new(high: f32, low: f32, threshold: f32) -> PyResult<Self> {
        // Bevy documents 0.0 to 1.0 as the valid range but does not check it.
        check_for::<Self, _>("high", high, UNIT_DOMAIN)?;
        check_for::<Self, _>("low", low, UNIT_DOMAIN)?;
        check_for::<Self, _>("threshold", threshold, FINITE_NON_NEGATIVE)?;
        check_ordered_for::<Self, _>("low", low, "high", high, false)?;
        Ok(PyButtonAxisSettings {
            inner: ButtonAxisSettings {
                high,
                low,
                threshold,
            },
        })
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
            "ButtonAxisSettings(high={}, low={}, threshold={})",
            float_repr(self.inner.high),
            float_repr(self.inner.low),
            float_repr(self.inner.threshold)
        )
    }
}

#[pycomponent(GamepadSettings, no_clone, bridge)]
#[pyclass(name = "GamepadSettings", module = "pybevy.input", extends = PyComponent)]
pub struct PyGamepadSettings {
    pub(crate) storage: ComponentStorage<GamepadSettings>,
}

#[pymethods]
impl PyGamepadSettings {
    #[new]
    #[pyo3(signature = (
        *,
        default_button_settings = PyButtonSettings { inner: ButtonSettings::default() },
        default_axis_settings = PyAxisSettings { inner: AxisSettings::default() },
        default_button_axis_settings = PyButtonAxisSettings {
            inner: ButtonAxisSettings::default()
        },
        button_settings = HashMap::new(),
        axis_settings = HashMap::new(),
        button_axis_settings = HashMap::new()
    ))]
    pub fn new(
        default_button_settings: PyButtonSettings,
        default_axis_settings: PyAxisSettings,
        default_button_axis_settings: PyButtonAxisSettings,
        button_settings: HashMap<PyGamepadButton, Py<PyButtonSettings>>,
        axis_settings: HashMap<PyGamepadAxis, Py<PyAxisSettings>>,
        button_axis_settings: HashMap<PyGamepadButton, Py<PyButtonAxisSettings>>,
    ) -> PyClassInitializer<Self> {
        let settings = GamepadSettings {
            default_button_settings: default_button_settings.inner,
            default_axis_settings: default_axis_settings.inner,
            default_button_axis_settings: default_button_axis_settings.inner,
            button_settings: button_settings
                .into_iter()
                .map(|(button, settings)| (button.into(), settings.get().inner.clone()))
                .collect(),
            axis_settings: axis_settings
                .into_iter()
                .map(|(axis, settings)| (axis.into(), settings.get().inner.clone()))
                .collect(),
            button_axis_settings: button_axis_settings
                .into_iter()
                .map(|(button, settings)| (button.into(), settings.get().inner.clone()))
                .collect(),
        };
        (
            Self {
                storage: ComponentStorage::owned(settings),
            },
            PyComponent,
        )
            .into()
    }

    #[setter]
    pub fn set_default_button_settings(&mut self, settings: &PyButtonSettings) -> PyResult<()> {
        self.as_mut()?.default_button_settings = settings.inner.clone();
        Ok(())
    }

    #[setter]
    pub fn set_default_axis_settings(&mut self, settings: &PyAxisSettings) -> PyResult<()> {
        self.as_mut()?.default_axis_settings = settings.inner.clone();
        Ok(())
    }

    #[setter]
    pub fn set_default_button_axis_settings(
        &mut self,
        settings: &PyButtonAxisSettings,
    ) -> PyResult<()> {
        self.as_mut()?.default_button_axis_settings = settings.inner.clone();
        Ok(())
    }

    #[setter]
    pub fn set_button_settings(
        &mut self,
        settings: HashMap<PyGamepadButton, Py<PyButtonSettings>>,
    ) -> PyResult<()> {
        self.as_mut()?.button_settings = settings
            .into_iter()
            .map(|(k, v)| (k.into(), v.get().inner.clone()))
            .collect();
        Ok(())
    }

    #[setter]
    pub fn set_axis_settings(
        &mut self,
        settings: HashMap<PyGamepadAxis, Py<PyAxisSettings>>,
    ) -> PyResult<()> {
        self.as_mut()?.axis_settings = settings
            .into_iter()
            .map(|(k, v)| (k.into(), v.get().inner.clone()))
            .collect();
        Ok(())
    }

    #[setter]
    pub fn set_button_axis_settings(
        &mut self,
        settings: HashMap<PyGamepadButton, Py<PyButtonAxisSettings>>,
    ) -> PyResult<()> {
        self.as_mut()?.button_axis_settings = settings
            .into_iter()
            .map(|(k, v)| (k.into(), v.get().inner.clone()))
            .collect();
        Ok(())
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

    #[pyo3(name = "get_button_settings")]
    pub fn lookup_button_settings(&self, button: PyGamepadButton) -> PyResult<PyButtonSettings> {
        Ok(self.as_ref()?.get_button_settings(button.into()).into())
    }

    #[pyo3(name = "get_axis_settings")]
    pub fn lookup_axis_settings(&self, axis: PyGamepadAxis) -> PyResult<PyAxisSettings> {
        Ok(self.as_ref()?.get_axis_settings(axis.into()).into())
    }

    #[pyo3(name = "get_button_axis_settings")]
    pub fn lookup_button_axis_settings(
        &self,
        button: PyGamepadButton,
    ) -> PyResult<PyButtonAxisSettings> {
        Ok(self
            .as_ref()?
            .get_button_axis_settings(button.into())
            .into())
    }

    #[getter(button_settings)]
    pub fn button_settings_snapshot<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let settings = self.as_ref()?;
        let values = PyDict::new(py);
        for (button, value) in &settings.button_settings {
            values.set_item(
                PyGamepadButton::from(*button),
                PyButtonSettings::from(value),
            )?;
        }
        PyModule::import(py, "types")?
            .getattr("MappingProxyType")?
            .call1((values,))
    }

    #[getter(axis_settings)]
    pub fn axis_settings_snapshot<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let settings = self.as_ref()?;
        let values = PyDict::new(py);
        for (axis, value) in &settings.axis_settings {
            values.set_item(PyGamepadAxis::from(*axis), PyAxisSettings::from(value))?;
        }
        PyModule::import(py, "types")?
            .getattr("MappingProxyType")?
            .call1((values,))
    }

    #[getter(button_axis_settings)]
    pub fn button_axis_settings_snapshot<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let settings = self.as_ref()?;
        let values = PyDict::new(py);
        for (button, value) in &settings.button_axis_settings {
            values.set_item(
                PyGamepadButton::from(*button),
                PyButtonAxisSettings::from(value),
            )?;
        }
        PyModule::import(py, "types")?
            .getattr("MappingProxyType")?
            .call1((values,))
    }

    fn __repr__(&self) -> PyResult<String> {
        let settings = self.as_ref()?;
        let mut buttons: Vec<String> = settings
            .button_settings
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}: {}",
                    PyGamepadButton::from(*key).__repr__(),
                    PyButtonSettings::from(value).__repr__()
                )
            })
            .collect();
        buttons.sort();
        let mut axes: Vec<String> = settings
            .axis_settings
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}: {}",
                    PyGamepadAxis::from(*key).__repr__(),
                    PyAxisSettings::from(value).__repr__()
                )
            })
            .collect();
        axes.sort();
        let mut button_axes: Vec<String> = settings
            .button_axis_settings
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}: {}",
                    PyGamepadButton::from(*key).__repr__(),
                    PyButtonAxisSettings::from(value).__repr__()
                )
            })
            .collect();
        button_axes.sort();
        Ok(format!(
            "GamepadSettings(default_button_settings={}, default_axis_settings={}, default_button_axis_settings={}, button_settings={{{}}}, axis_settings={{{}}}, button_axis_settings={{{}}})",
            PyButtonSettings::from(&settings.default_button_settings).__repr__(),
            PyAxisSettings::from(&settings.default_axis_settings).__repr__(),
            PyButtonAxisSettings::from(&settings.default_button_axis_settings).__repr__(),
            buttons.join(", "),
            axes.join(", "),
            button_axes.join(", ")
        ))
    }
}
