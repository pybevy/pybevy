use bevy::window::WindowResizeConstraints;
use pybevy_core::{
    FromBorrowedStorage, StorageError, ValueStorage, public_error, repr::float_repr,
};
use pybevy_macros::pyvalue;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pyvalue]
#[pyclass(
    name = "WindowResizeConstraints",
    module = "pybevy.window",
    eq,
    from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyWindowResizeConstraints {
    pub(crate) storage: ValueStorage<WindowResizeConstraints>,
}

impl PartialEq for PyWindowResizeConstraints {
    fn eq(&self, other: &Self) -> bool {
        match (self.to_bevy(), other.to_bevy()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

impl From<WindowResizeConstraints> for PyWindowResizeConstraints {
    fn from(value: WindowResizeConstraints) -> Self {
        PyWindowResizeConstraints::from_owned(value)
    }
}

impl Default for PyWindowResizeConstraints {
    fn default() -> Self {
        Self::from_owned(WindowResizeConstraints::default())
    }
}

impl TryFrom<PyWindowResizeConstraints> for WindowResizeConstraints {
    type Error = PyErr;

    fn try_from(value: PyWindowResizeConstraints) -> PyResult<Self> {
        let constraints = value.storage.get()?;
        validate_constraints(&constraints)?;
        Ok(constraints)
    }
}

impl TryFrom<&PyWindowResizeConstraints> for WindowResizeConstraints {
    type Error = PyErr;

    fn try_from(value: &PyWindowResizeConstraints) -> PyResult<Self> {
        let constraints = value.storage.get()?;
        validate_constraints(&constraints)?;
        Ok(constraints)
    }
}

#[pymethods]
impl PyWindowResizeConstraints {
    #[new]
    #[pyo3(signature = (
        *,
        min_width = 180.0,
        min_height = 120.0,
        max_width = f64::INFINITY,
        max_height = f64::INFINITY
    ))]
    pub fn new(min_width: f32, min_height: f32, max_width: f64, max_height: f64) -> PyResult<Self> {
        let constraints = WindowResizeConstraints {
            min_width,
            min_height,
            max_width: parse_maximum(max_width, "max_width")?,
            max_height: parse_maximum(max_height, "max_height")?,
        };
        validate_constraints(&constraints)?;
        Ok(Self::from_owned(constraints))
    }

    #[getter]
    pub fn min_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_width)
    }

    #[setter]
    pub fn set_min_width(&mut self, value: f32) -> PyResult<()> {
        self.storage.check_write_access()?;
        let mut candidate = self.to_bevy()?;
        candidate.min_width = value;
        validate_constraints(&candidate)?;
        self.storage
            .update(|constraints| constraints.min_width = value)?;
        Ok(())
    }

    #[getter]
    pub fn min_height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_height)
    }

    #[setter]
    pub fn set_min_height(&mut self, value: f32) -> PyResult<()> {
        self.storage.check_write_access()?;
        let mut candidate = self.to_bevy()?;
        candidate.min_height = value;
        validate_constraints(&candidate)?;
        self.storage
            .update(|constraints| constraints.min_height = value)?;
        Ok(())
    }

    #[getter]
    pub fn max_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_width)
    }

    #[setter]
    pub fn set_max_width(&mut self, value: f64) -> PyResult<()> {
        self.storage.check_write_access()?;
        let value = parse_maximum(value, "max_width")?;
        let mut candidate = self.to_bevy()?;
        candidate.max_width = value;
        validate_constraints(&candidate)?;
        self.storage
            .update(|constraints| constraints.max_width = value)?;
        Ok(())
    }

    #[getter]
    pub fn max_height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.max_height)
    }

    #[setter]
    pub fn set_max_height(&mut self, value: f64) -> PyResult<()> {
        self.storage.check_write_access()?;
        let value = parse_maximum(value, "max_height")?;
        let mut candidate = self.to_bevy()?;
        candidate.max_height = value;
        validate_constraints(&candidate)?;
        self.storage
            .update(|constraints| constraints.max_height = value)?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let c = self.to_bevy()?;
        Ok(format!(
            "WindowResizeConstraints(min_width={}, min_height={}, max_width={}, max_height={})",
            float_repr(c.min_width),
            float_repr(c.min_height),
            float_repr(c.max_width),
            float_repr(c.max_height)
        ))
    }
}

fn parse_maximum(value: f64, field: &str) -> PyResult<f32> {
    if value == f64::INFINITY {
        return Ok(f32::INFINITY);
    }
    if !value.is_finite() || !(f32::MIN as f64..=f32::MAX as f64).contains(&value) {
        return Err(PyValueError::new_err(
            public_error::window_resize_max_domain(field),
        ));
    }
    Ok(value as f32)
}

pub(crate) fn validate_constraints(
    constraints: &WindowResizeConstraints,
) -> Result<(), StorageError> {
    for (field, value) in [
        ("min_width", constraints.min_width),
        ("min_height", constraints.min_height),
    ] {
        if !value.is_finite() {
            return Err(StorageError::InvalidValue(
                public_error::window_resize_min_domain(field),
            ));
        }
    }
    for (field, value) in [
        ("max_width", constraints.max_width),
        ("max_height", constraints.max_height),
    ] {
        if !value.is_finite() && value != f32::INFINITY {
            return Err(StorageError::InvalidValue(
                public_error::window_resize_max_domain(field),
            ));
        }
    }
    Ok(())
}
