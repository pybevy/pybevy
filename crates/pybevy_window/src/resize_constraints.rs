use bevy::window::WindowResizeConstraints;
use pybevy_core::{
    FromBorrowedStorage, StorageError, ValueStorage, domain::F32Input, repr::float_repr,
};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

#[pyvalue(domains = [
    min_width => [finite],
    min_height => [finite],
    max_width => [finite_or_pos_inf],
    max_height => [finite_or_pos_inf],
])]
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
            max_width: Self::check_max_width(F32Input(max_width))?.narrowed(),
            max_height: Self::check_max_height(F32Input(max_height))?.narrowed(),
        };
        Self::validate_domains(&constraints)?;
        Ok(Self::from_owned(constraints))
    }

    #[getter]
    pub fn min_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.min_width)
    }

    #[setter]
    pub fn set_min_width(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_min_width(value)?;
        self.storage.check_write_access()?;
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
        let value = Self::check_min_height(value)?;
        self.storage.check_write_access()?;
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
        let value = Self::check_max_width(F32Input(value))?.narrowed();
        self.storage.check_write_access()?;
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
        let value = Self::check_max_height(F32Input(value))?.narrowed();
        self.storage.check_write_access()?;
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

pub(crate) fn validate_constraints(
    constraints: &WindowResizeConstraints,
) -> Result<(), StorageError> {
    Ok(PyWindowResizeConstraints::validate_domains(constraints)?)
}
