use bevy::ui::ComputedStackIndex;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(ComputedStackIndex, no_clone, bridge)]
#[pyclass(name = "ComputedStackIndex", extends = PyComponent)]
pub struct PyComputedStackIndex {
    pub(crate) storage: ComponentStorage<ComputedStackIndex>,
}

#[pymethods]
impl PyComputedStackIndex {
    #[getter]
    pub fn value(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.0)
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(v) => format!("ComputedStackIndex({})", v.0),
            Err(_) => "ComputedStackIndex(<invalid>)".to_string(),
        }
    }
}
