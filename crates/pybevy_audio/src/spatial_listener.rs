use bevy::audio::SpatialListener;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pybevy_math::PyVec3;
use pyo3::prelude::*;

#[component_storage(SpatialListener)]
#[pyclass(name = "SpatialListener", extends = PyComponent)]
#[derive(Clone)]
pub struct PySpatialListener {
    pub(crate) storage: ComponentStorage<SpatialListener>,
}

#[pymethods]
impl PySpatialListener {
    #[new]
    #[pyo3(signature = (gap=4.0, *, left_ear_offset=None, right_ear_offset=None))]
    pub fn new(
        gap: f32,
        left_ear_offset: Option<PyVec3>,
        right_ear_offset: Option<PyVec3>,
    ) -> (Self, PyComponent) {
        let mut listener = SpatialListener::new(gap);
        if let Some(offset) = left_ear_offset {
            listener.left_ear_offset = offset.into();
        }
        if let Some(offset) = right_ear_offset {
            listener.right_ear_offset = offset.into();
        }
        Self::from_owned(listener)
    }

    #[getter]
    pub fn left_ear_offset(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_field_as(|l| &l.left_ear_offset)?)
    }

    #[setter]
    pub fn set_left_ear_offset(&mut self, offset: PyVec3) -> PyResult<()> {
        self.as_mut()?.left_ear_offset = offset.into();
        Ok(())
    }

    #[getter]
    pub fn right_ear_offset(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_field_as(|l| &l.right_ear_offset)?)
    }

    #[setter]
    pub fn set_right_ear_offset(&mut self, offset: PyVec3) -> PyResult<()> {
        self.as_mut()?.right_ear_offset = offset.into();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(listener) => format!(
                "SpatialListener(left_ear={:?}, right_ear={:?})",
                listener.left_ear_offset, listener.right_ear_offset
            ),
            Err(_) => "SpatialListener(<invalid>)".to_string(),
        }
    }
}
