use bevy::ui::{CornerRadius, Val};
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::object_repr};
use pybevy_macros::pyvalue;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyTuple};

use crate::val::PyVal;

#[pyvalue]
#[pyclass(name = "CornerRadius", module = "pybevy.ui", eq, from_py_object)]
#[derive(Clone, Debug)]
pub struct PyCornerRadius {
    pub(crate) storage: ValueStorage<CornerRadius>,
}

impl PartialEq for PyCornerRadius {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.as_ref(), other.as_ref()), (Ok(left), Ok(right)) if *left == *right)
    }
}

impl From<CornerRadius> for PyCornerRadius {
    fn from(value: CornerRadius) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyCornerRadius> for CornerRadius {
    type Error = PyErr;

    fn try_from(value: PyCornerRadius) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyCornerRadius> for CornerRadius {
    type Error = PyErr;

    fn try_from(value: &PyCornerRadius) -> PyResult<Self> {
        value.to_bevy()
    }
}

/// Mirrors bevy's `impl Into<CornerRadius>` parameters: a `CornerRadius`, a `Val`
/// (circular), a `(Val, Val)` tuple or a `[Val, Val]` list.
pub fn extract_corner_radius_from_any(obj: &Bound<'_, PyAny>) -> PyResult<CornerRadius> {
    if let Ok(radius) = obj.cast::<PyCornerRadius>() {
        return radius.borrow().to_bevy();
    }
    if let Ok(value) = obj.extract::<PyVal>() {
        return Ok(CornerRadius::from(Val::from(value)));
    }
    if obj.is_instance_of::<PyTuple>() {
        if let Ok((x, y)) = obj.extract::<(PyVal, PyVal)>() {
            return Ok(CornerRadius::from((Val::from(x), Val::from(y))));
        }
    } else if let Ok([x, y]) = obj.extract::<[PyVal; 2]>() {
        return Ok(CornerRadius::from([Val::from(x), Val::from(y)]));
    }
    Err(PyTypeError::new_err(format!(
        "expected CornerRadius, Val, or a pair of Val values, got {}",
        obj.get_type().name()?
    )))
}

/// A Python argument accepted wherever bevy takes `impl Into<CornerRadius>`.
#[derive(Debug, Clone, Copy)]
pub struct IntoCornerRadius(pub CornerRadius);

impl FromPyObject<'_, '_> for IntoCornerRadius {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        extract_corner_radius_from_any(&value).map(Self)
    }
}

#[pymethods]
impl PyCornerRadius {
    #[new]
    #[pyo3(signature = (x = PyVal::Auto(), y = PyVal::Auto()))]
    pub fn new(x: PyVal, y: PyVal) -> Self {
        Self::from_owned(CornerRadius::new(x.into(), y.into()))
    }

    #[staticmethod]
    #[pyo3(name = "MAX")]
    pub fn max() -> Self {
        Self::from_owned(CornerRadius::MAX)
    }

    #[staticmethod]
    #[pyo3(name = "MAX_ELLIPTICAL")]
    pub fn max_elliptical() -> Self {
        Self::from_owned(CornerRadius::MAX_ELLIPTICAL)
    }

    #[staticmethod]
    #[pyo3(name = "ZERO")]
    pub fn zero() -> Self {
        Self::from_owned(CornerRadius::ZERO)
    }

    #[staticmethod]
    pub fn circular(radius: PyVal) -> Self {
        Self::from_owned(CornerRadius::circular(radius.into()))
    }

    #[staticmethod]
    pub fn all(radius: PyVal) -> Self {
        Self::from_owned(CornerRadius::all(radius.into()))
    }

    #[getter]
    pub fn x(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.x.into())
    }

    #[setter]
    pub fn set_x(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.x = value.into();
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.y.into())
    }

    #[setter]
    pub fn set_y(&mut self, value: PyVal) -> PyResult<()> {
        self.as_mut()?.y = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let radius = self.as_ref()?;
        Ok(format!(
            "CornerRadius(x={}, y={})",
            object_repr(PyVal::from(radius.x)),
            object_repr(PyVal::from(radius.y))
        ))
    }
}
