use std::ops::Range;

use bevy::camera::Viewport;
use pybevy_core::{
    FromBorrowedStorage, StorageError,
    field_storage::FieldStorage,
    public_error::{UNSUPPORTED_COMPARISON, inverted_range, value_out_of_range},
};
use pybevy_macros::pyfield;
use pybevy_math::{range::PyRange, uvec2::PyUVec2};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

/// wgpu rejects a depth range outside 0.0..=1.0, one validation error per frame.
fn check_depth(depth: &Range<f32>) -> Result<(), StorageError> {
    for (name, value) in [("depth near", depth.start), ("depth far", depth.end)] {
        if !(0.0..=1.0).contains(&value) {
            return Err(StorageError::InvalidValue(value_out_of_range(
                name, 0.0, 1.0, value,
            )));
        }
    }
    if depth.start > depth.end {
        return Err(StorageError::InvalidValue(inverted_range(
            "depth",
            depth.start,
            depth.end,
        )));
    }
    Ok(())
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
    #[pyo3(signature = (*, physical_position = PyUVec2::ZERO, physical_size = PyUVec2::ONE, depth = PyRange::from_range(0.0, 1.0)))]
    pub fn new(
        physical_position: PyUVec2,
        physical_size: PyUVec2,
        depth: PyRange,
    ) -> PyResult<Self> {
        let depth = depth.try_into()?;
        check_depth(&depth)?;
        Ok(Self::from_owned(Viewport {
            physical_position: physical_position.try_into()?,
            physical_size: physical_size.try_into()?,
            depth,
        }))
    }

    #[getter]
    pub fn physical_position(&self) -> PyResult<PyUVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|v| &v.physical_position, |v| &mut v.physical_position)?)
    }

    #[setter]
    pub fn set_physical_position(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.physical_position = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn physical_size(&self) -> PyResult<PyUVec2> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|v| &v.physical_size, |v| &mut v.physical_size)?)
    }

    #[setter]
    pub fn set_physical_size(&mut self, value: PyUVec2) -> PyResult<()> {
        self.as_mut()?.physical_size = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn depth(&self) -> PyResult<PyRange> {
        Ok(self
            .storage
            .borrow_validated_field_as(|v| &v.depth, |v| &mut v.depth, check_depth)?)
    }

    #[setter]
    pub fn set_depth(&mut self, value: &PyRange) -> PyResult<()> {
        let depth = value.try_into()?;
        let mut viewport = self.as_mut()?;
        check_depth(&depth)?;
        viewport.depth = depth;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let viewport = self.as_ref()?;
        Ok(format!(
            "Viewport(physical_position=UVec2({}, {}), physical_size=UVec2({}, {}), depth=Range({}, {}))",
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
