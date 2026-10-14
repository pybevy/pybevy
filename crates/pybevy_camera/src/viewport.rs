use bevy::{camera::Viewport, math::UVec2};
use pybevy_core::{
    FromBorrowedStorage, field_storage::FieldStorage, public_error::UNSUPPORTED_COMPARISON,
    repr::float_repr,
};
use pybevy_macros::pyfield;
use pybevy_math::{range::PyRange, uvec2::PyUVec2};
use pyo3::{basic::CompareOp, exceptions::PyTypeError, prelude::*};

// wgpu rejects a zero-sized texture or a depth range outside 0.0..=1.0 or inverted;
// Bevy adds position and size in u32, so together they must not overflow.
#[pyfield(domains = [
    physical_position => [range_inclusive(0.0, 2147483647.0)],
    physical_size => [range_inclusive(1.0, 2147483648.0)],
    depth => [range_inclusive(0.0, 1.0), ordered],
])]
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
        let physical_position = Self::check_physical_position(UVec2::try_from(physical_position)?)?;
        let physical_size = Self::check_physical_size(UVec2::try_from(physical_size)?)?;
        let depth = Self::check_depth(depth.try_into()?)?;
        Ok(Self::from_owned(Viewport {
            physical_position,
            physical_size,
            depth,
        }))
    }

    #[getter]
    pub fn physical_position(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.physical_position,
            |v| &mut v.physical_position,
            Self::validate_physical_position,
        )?)
    }

    #[setter]
    pub fn set_physical_position(&mut self, value: PyUVec2) -> PyResult<()> {
        let position = Self::check_physical_position(UVec2::try_from(value)?)?;
        self.as_mut()?.physical_position = position;
        Ok(())
    }

    #[getter]
    pub fn physical_size(&self) -> PyResult<PyUVec2> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.physical_size,
            |v| &mut v.physical_size,
            Self::validate_physical_size,
        )?)
    }

    #[setter]
    pub fn set_physical_size(&mut self, value: PyUVec2) -> PyResult<()> {
        let size = Self::check_physical_size(UVec2::try_from(value)?)?;
        self.as_mut()?.physical_size = size;
        Ok(())
    }

    #[getter]
    pub fn depth(&self) -> PyResult<PyRange> {
        Ok(self.storage.borrow_validated_field_as(
            |v| &v.depth,
            |v| &mut v.depth,
            Self::validate_depth,
        )?)
    }

    #[setter]
    pub fn set_depth(&mut self, value: &PyRange) -> PyResult<()> {
        let depth = Self::check_depth(value.try_into()?)?;
        self.as_mut()?.depth = depth;
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
            float_repr(viewport.depth.start),
            float_repr(viewport.depth.end)
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
        let mut viewport = self.storage.get()?;
        viewport.clamp_to_size(size.try_into()?);
        Self::validate_domains(&viewport)?;
        *self.as_mut()? = viewport;
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
                vp.physical_size = Self::check_physical_size(UVec2::try_from(override_size)?)?;
            }
        }

        Ok(result_viewport.map(PyViewport::from_owned))
    }
}
