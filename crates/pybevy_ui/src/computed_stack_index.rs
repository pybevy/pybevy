use bevy::ui::ComputedStackIndex;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

fn clone_computed_stack_index(value: &ComputedStackIndex) -> ComputedStackIndex {
    ComputedStackIndex(value.0)
}

#[pycomponent(
    ComputedStackIndex,
    no_clone,
    bridge,
    clone_with = clone_computed_stack_index
)]
#[pyclass(name = "ComputedStackIndex", module = "pybevy.ui", extends = PyComponent)]
pub struct PyComputedStackIndex {
    pub(crate) storage: ComponentStorage<ComputedStackIndex>,
}

#[pymethods]
impl PyComputedStackIndex {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (
            Self {
                storage: ComponentStorage::owned(ComputedStackIndex::default()),
            },
            PyComponent,
        )
            .into()
    }

    #[getter]
    pub fn value(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(v) => format!("ComputedStackIndex({})", v.0),
            Err(_) => "ComputedStackIndex(<invalid>)".to_string(),
        }
    }
}
