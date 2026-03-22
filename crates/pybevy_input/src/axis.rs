use bevy::input::{Axis, gamepad::GamepadAxis};
use pybevy_core::{PyResource, ResourceStorage};
use pyo3::prelude::*;

use crate::gamepad_axis::PyGamepadAxis;

#[pyclass(name = "Axis", extends = PyResource)]
pub struct PyAxis {
    storage: Option<ResourceStorage<Axis<GamepadAxis>>>,
}

impl PyAxis {
    pub fn from_borrowed(storage: ResourceStorage<Axis<GamepadAxis>>) -> (Self, PyResource) {
        (
            PyAxis {
                storage: Some(storage),
            },
            PyResource,
        )
    }

    fn get_axis(&self) -> PyResult<&Axis<GamepadAxis>> {
        match &self.storage {
            Some(storage) => Ok(storage.as_ref()?),
            None => {
                static EMPTY_AXIS: std::sync::OnceLock<Axis<GamepadAxis>> =
                    std::sync::OnceLock::new();
                Ok(EMPTY_AXIS.get_or_init(Axis::default))
            }
        }
    }
}

#[pymethods]
impl PyAxis {
    #[classattr]
    const MIN: f32 = Axis::<GamepadAxis>::MIN;

    #[classattr]
    const MAX: f32 = Axis::<GamepadAxis>::MAX;

    #[new]
    pub fn new() -> (Self, PyResource) {
        (PyAxis { storage: None }, PyResource)
    }

    pub fn get(&self, axis: PyGamepadAxis) -> PyResult<Option<f32>> {
        let axis_data = self.get_axis()?;
        let bevy_axis: GamepadAxis = axis.into();
        Ok(axis_data.get(bevy_axis))
    }

    pub fn get_unclamped(&self, axis: PyGamepadAxis) -> PyResult<Option<f32>> {
        let axis_data = self.get_axis()?;
        let bevy_axis: GamepadAxis = axis.into();
        Ok(axis_data.get_unclamped(bevy_axis))
    }

    pub fn all_axes(&self) -> PyResult<Vec<PyGamepadAxis>> {
        let axis_data = self.get_axis()?;
        Ok(axis_data.all_axes().map(|a| (*a).into()).collect())
    }

    pub fn all_axes_and_values(&self) -> PyResult<Vec<(PyGamepadAxis, f32)>> {
        let axis_data = self.get_axis()?;
        Ok(axis_data
            .all_axes_and_values()
            .map(|(a, v)| ((*a).into(), v))
            .collect())
    }

    fn __repr__(&self) -> String {
        if self.storage.is_some() {
            "Axis(initialized)".to_string()
        } else {
            "Axis(uninitialized)".to_string()
        }
    }
}
