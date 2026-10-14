use bevy::ui::ScrollPosition;
use pybevy_core::{ComponentStorage, PyComponent, repr::object_repr};
use pybevy_macros::pycomponent;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pycomponent(ScrollPosition, bridge, view_fields = [0 as offset])]
#[pyclass(name = "ScrollPosition", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyScrollPosition {
    pub(crate) storage: ComponentStorage<ScrollPosition>,
}

#[pymethods]
impl PyScrollPosition {
    #[new]
    #[pyo3(signature = (offset = PyVec2::ZERO))]
    pub fn new(offset: PyVec2) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(ScrollPosition(offset.try_into()?)).into())
    }

    #[getter]
    pub fn offset(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|s| &s.0)?)
    }

    #[setter]
    pub fn set_offset(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.0 = value.try_into()?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let pos = self.as_ref()?;
        Ok(format!(
            "ScrollPosition({})",
            object_repr(PyVec2::from(pos.0))
        ))
    }
}
