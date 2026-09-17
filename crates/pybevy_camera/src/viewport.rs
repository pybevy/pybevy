use bevy::camera::Viewport;
use pybevy_core::{
    FromBorrowedStorage,
    field_storage::FieldStorage,
    public_error::{UNSUPPORTED_COMPARISON, inverted_range, value_out_of_range},
};
use pybevy_macros::pyfield;
use pybevy_math::uvec2::PyUVec2;
use pyo3::{
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

/// wgpu rejects a depth range outside 0.0..=1.0, one validation error per frame.
fn check_depth(depth: (f32, f32)) -> PyResult<(f32, f32)> {
    for (name, value) in [("depth near", depth.0), ("depth far", depth.1)] {
        if !(0.0..=1.0).contains(&value) {
            return Err(PyValueError::new_err(value_out_of_range(
                name, 0.0, 1.0, value,
            )));
        }
    }
    if depth.0 > depth.1 {
        return Err(PyValueError::new_err(inverted_range(
            "depth", depth.0, depth.1,
        )));
    }
    Ok(depth)
}

#[pyfield]
#[pyclass(name = "Viewport", module = "pybevy.camera", skip_from_py_object)]
#[derive(Debug)]
pub struct PyViewport {
    storage: FieldStorage<Viewport>,
}

#[pymethods]
impl PyViewport {
    #[new]
    #[pyo3(signature = (*, physical_position = PyUVec2::ZERO, physical_size = PyUVec2::ONE, depth=(0.0, 1.0)))]
    pub fn new(
        physical_position: PyUVec2,
        physical_size: PyUVec2,
        depth: (f32, f32),
    ) -> PyResult<Self> {
        let depth = check_depth(depth)?;
        Ok(Self::from_owned(Viewport {
            physical_position: physical_position.try_into()?,
            physical_size: physical_size.try_into()?,
            depth: depth.0..depth.1,
        }))
    }

    #[getter]
    pub fn physical_position(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_field_as(|v| &v.physical_position)?)
    }

    #[setter]
    pub fn set_physical_position(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.physical_position = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn physical_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_field_as(|v| &v.physical_size)?)
    }

    #[setter]
    pub fn set_physical_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.physical_size = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn depth(&self) -> PyResult<(f32, f32)> {
        let viewport = self.as_ref()?;
        Ok((viewport.depth.start, viewport.depth.end))
    }

    #[setter]
    pub fn set_depth(&mut self, value: (f32, f32)) -> PyResult<()> {
        // Authority first: an expired viewport must report that, not a
        // complaint about the value it was never allowed to store.
        let mut viewport = self.as_mut()?;
        let depth = check_depth(value)?;
        viewport.depth = depth.0..depth.1;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let viewport = self.as_ref()?;
        Ok(format!(
            "Viewport(physical_position=UVec2({}, {}), physical_size=UVec2({}, {}), depth=({}, {}))",
            viewport.physical_position.x,
            viewport.physical_position.y,
            viewport.physical_size.x,
            viewport.physical_size.y,
            viewport.depth.start,
            viewport.depth.end
        ))
    }

    pub fn __richcmp__(&self, other: &PyViewport, op: CompareOp) -> PyResult<bool> {
        let self_viewport = self.as_ref()?;
        let other_viewport = other.as_ref()?;
        match op {
            CompareOp::Eq => Ok(self_viewport.physical_position
                == other_viewport.physical_position
                && self_viewport.physical_size == other_viewport.physical_size
                && self_viewport.depth == other_viewport.depth),
            CompareOp::Ne => Ok(self_viewport.physical_position
                != other_viewport.physical_position
                || self_viewport.physical_size != other_viewport.physical_size
                || self_viewport.depth != other_viewport.depth),
            _ => Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        }
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.storage.get()?))
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    pub fn clamp_to_size(&mut self, size: PyUVec2) -> PyResult<()> {
        self.as_mut()?.clamp_to_size(size.try_into()?);
        Ok(())
    }

    #[staticmethod]
    pub fn from_viewport_and_override(
        viewport: Option<&PyViewport>,
        main_pass_resolution_override: Option<PyUVec2>,
    ) -> PyResult<Option<PyViewport>> {
        let mut result_viewport = viewport.map(|v| v.storage.get()).transpose()?;

        if let Some(override_size) = main_pass_resolution_override {
            if result_viewport.is_none() {
                result_viewport = Some(Viewport::default());
            }
            if let Some(ref mut vp) = result_viewport {
                vp.physical_size = override_size.try_into()?;
            }
        }

        Ok(result_viewport.map(PyViewport::from_owned))
    }
}
