use bevy::{camera::MainPassResolutionOverride, math::UVec2};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::uvec2::PyUVec2;
use pyo3::prelude::*;

fn clone_main_pass_resolution_override(
    value: &MainPassResolutionOverride,
) -> MainPassResolutionOverride {
    MainPassResolutionOverride(value.0)
}

#[pycomponent(
    MainPassResolutionOverride,
    no_clone,
    bridge,
    clone_with = clone_main_pass_resolution_override
)]
#[pyclass(name = "MainPassResolutionOverride", extends = PyComponent, frozen)]
#[derive(Debug)]
pub struct PyMainPassResolutionOverride {
    pub(crate) storage: ComponentStorage<MainPassResolutionOverride>,
}

#[pymethods]
impl PyMainPassResolutionOverride {
    #[new]
    pub fn new(value: &PyUVec2) -> PyResult<PyClassInitializer<Self>> {
        let value: UVec2 = value.try_into()?;
        Ok((
            Self {
                storage: ComponentStorage::owned(MainPassResolutionOverride(value)),
            },
            PyComponent,
        )
            .into())
    }

    #[getter]
    pub fn value(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.snapshot_field_as(|resolution| &resolution.0)?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let resolution = self.as_ref()?.0;
        Ok(format!(
            "MainPassResolutionOverride({}x{})",
            resolution.x, resolution.y
        ))
    }
}
