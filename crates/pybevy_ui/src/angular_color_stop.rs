use bevy::{color::Color, ui::AngularColorStop};
use pybevy_color::color::PyColor;
use pybevy_core::{FromBorrowedStorage, ValueStorage, public_error::value_out_of_range};
use pybevy_macros::pyvalue;
use pyo3::{exceptions::PyValueError, prelude::*};

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
#[pyclass(name = "AngularColorStop", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyAngularColorStop {
    pub(crate) storage: ValueStorage<AngularColorStop>,
}

impl PartialEq for PyAngularColorStop {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.to_bevy(), other.to_bevy()), (Ok(left), Ok(right)) if left == right)
    }
}

impl From<AngularColorStop> for PyAngularColorStop {
    fn from(value: AngularColorStop) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyAngularColorStop> for AngularColorStop {
    type Error = PyErr;

    fn try_from(value: PyAngularColorStop) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyAngularColorStop> for AngularColorStop {
    type Error = PyErr;

    fn try_from(value: &PyAngularColorStop) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyAngularColorStop {
    #[new]
    #[pyo3(signature = (color = None, angle = None, *, hint = 0.5))]
    pub fn new(color: Option<PyColor>, angle: Option<f32>, hint: f32) -> PyResult<Self> {
        let color = color
            .map(Color::try_from)
            .transpose()?
            .unwrap_or(Color::WHITE);
        Ok(Self::from_owned(AngularColorStop {
            color,
            angle,
            hint: check_hint(hint)?,
        }))
    }

    #[staticmethod]
    pub fn auto(color: PyColor) -> PyResult<Self> {
        Ok(Self::from_owned(AngularColorStop::auto(Color::try_from(
            color,
        )?)))
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
    pub fn angle(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.angle)
    }

    #[setter]
    pub fn set_angle(&mut self, value: Option<f32>) -> PyResult<()> {
        self.as_mut()?.angle = value;
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
            "AngularColorStop(color={:?}, angle={:?}, hint={})",
            stop.color, stop.angle, stop.hint
        ))
    }
}
