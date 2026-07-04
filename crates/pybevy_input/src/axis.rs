use bevy::input::{Axis, gamepad::GamepadAxis};
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

use crate::gamepad_axis::PyGamepadAxis;

#[pyresource(Axis<GamepadAxis>, no_clone, bridge, "Axis", no_mut, no_insert)]
#[pyclass(name = "Axis", extends = PyResource)]
pub struct PyAxis {
    pub(crate) storage: ResourceStorage<Axis<GamepadAxis>>,
}

#[pymethods]
impl PyAxis {
    #[classattr]
    const MIN: f32 = Axis::<GamepadAxis>::MIN;

    #[classattr]
    const MAX: f32 = Axis::<GamepadAxis>::MAX;

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            Self {
                storage: ResourceStorage::owned(Axis::default()),
            },
            PyResource,
        )
            .into()
    }

    pub fn get(&self, axis: PyGamepadAxis) -> PyResult<Option<f32>> {
        let axis_data = self.as_ref()?;
        let bevy_axis: GamepadAxis = axis.into();
        Ok(axis_data.get(bevy_axis))
    }

    pub fn get_unclamped(&self, axis: PyGamepadAxis) -> PyResult<Option<f32>> {
        let axis_data = self.as_ref()?;
        let bevy_axis: GamepadAxis = axis.into();
        Ok(axis_data.get_unclamped(bevy_axis))
    }

    pub fn all_axes(&self) -> PyResult<Vec<PyGamepadAxis>> {
        let axis_data = self.as_ref()?;
        Ok(axis_data.all_axes().map(|a| (*a).into()).collect())
    }

    pub fn all_axes_and_values(&self) -> PyResult<Vec<(PyGamepadAxis, f32)>> {
        let axis_data = self.as_ref()?;
        Ok(axis_data
            .all_axes_and_values()
            .map(|(a, v)| ((*a).into(), v))
            .collect())
    }

    fn __repr__(&self) -> String {
        "Axis(...)".to_string()
    }
}
