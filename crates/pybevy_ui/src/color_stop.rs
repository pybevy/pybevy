use bevy::{color::Color, ui::ColorStop};
use pybevy_color::color::PyColor;
use pybevy_core::{FromBorrowedStorage, ValueStorage, public_error::value_out_of_range};
use pybevy_macros::pyvalue;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::val::PyVal;

fn check_hint(hint: f32) -> PyResult<f32> {
    if (0.0..=1.0).contains(&hint) {
        Ok(hint)
    } else {
        Err(PyValueError::new_err(value_out_of_range(
            "hint", 0.0, 1.0, hint,
        )))
    }
}

#[pyvalue]
#[pyclass(name = "ColorStop", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyColorStop {
    pub(crate) storage: ValueStorage<ColorStop>,
}

impl PartialEq for PyColorStop {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.to_bevy(), other.to_bevy()), (Ok(left), Ok(right)) if left == right)
    }
}

impl From<ColorStop> for PyColorStop {
    fn from(value: ColorStop) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyColorStop> for ColorStop {
    type Error = PyErr;

    fn try_from(value: PyColorStop) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyColorStop> for ColorStop {
    type Error = PyErr;

    fn try_from(value: &PyColorStop) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyColorStop {
    #[new]
    #[pyo3(signature = (color = None, point = PyVal::default(), *, hint = 0.5))]
    pub fn new(color: Option<PyColor>, point: PyVal, hint: f32) -> PyResult<Self> {
        let color = color
            .map(Color::try_from)
            .transpose()?
            .unwrap_or(Color::WHITE);
        Ok(Self::from_owned(ColorStop {
            color,
            point: point.into(),
            hint: check_hint(hint)?,
        }))
    }

    #[staticmethod]
    pub fn auto(color: PyColor) -> PyResult<Self> {
        Ok(Self::from_owned(ColorStop::auto(Color::try_from(color)?)))
    }

    #[staticmethod]
    pub fn px(color: PyColor, px: f32) -> PyResult<Self> {
        Ok(Self::from_owned(ColorStop::px(Color::try_from(color)?, px)))
    }

    #[staticmethod]
    pub fn percent(color: PyColor, percent: f32) -> PyResult<Self> {
        Ok(Self::from_owned(ColorStop::percent(
            Color::try_from(color)?,
            percent,
        )))
    }

    pub fn with_hint(&self, hint: f32) -> PyResult<Self> {
        Ok(Self::from_owned(
            self.to_bevy()?.with_hint(check_hint(hint)?),
        ))
    }

    #[getter]
    pub fn color(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        PyColor::from_storage(self.storage.borrow_field(|stop| &stop.color)?, py)
    }

    #[setter]
    pub fn set_color(&mut self, value: PyColor) -> PyResult<()> {
        let value = value.try_into()?;
        self.as_mut()?.color = value;
        Ok(())
    }

    #[getter]
    pub fn point(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.point.into())
    }

    #[setter]
    pub fn set_point(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.point = value.into();
        Ok(())
    }

    #[getter]
    pub fn hint(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.hint)
    }

    #[setter]
    pub fn set_hint(&mut self, value: f32) -> PyResult<()> {
        let value = check_hint(value)?;
        self.as_mut()?.hint = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let stop = self.as_ref()?;
        Ok(format!(
            "ColorStop(color={:?}, point={:?}, hint={})",
            stop.color, stop.point, stop.hint
        ))
    }
}
