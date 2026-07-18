use bevy::window::{MonitorSelection, WindowPosition};
use pybevy_macros::pyenum;
use pybevy_math::ivec2::PyIVec2;
use pyo3::prelude::*;

use crate::monitor_selection::PyMonitorSelection;

#[pyenum(WindowPosition, manual)]
#[pyclass(
    name = "WindowPosition",
    module = "pybevy.window",
    eq,
    frozen,
    subclass,
    from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowPosition(pub(crate) WindowPosition);

impl From<WindowPosition> for PyWindowPosition {
    fn from(value: WindowPosition) -> Self {
        Self(value)
    }
}

impl From<PyWindowPosition> for WindowPosition {
    fn from(value: PyWindowPosition) -> Self {
        value.0
    }
}

#[pymethods]
impl PyWindowPosition {
    #[new]
    pub fn new() -> PyResult<Self> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "WindowPosition is an enum base; construct a nested variant",
        ))
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            WindowPosition::Automatic => "WindowPosition.Automatic()".to_string(),
            WindowPosition::Centered(monitor) => {
                format!(
                    "WindowPosition.Centered(value={})",
                    monitor_selection_repr(monitor)
                )
            }
            WindowPosition::At(position) => format!(
                "WindowPosition.At(value=IVec2({}, {}))",
                position.x, position.y
            ),
        }
    }
}

#[pyclass(
    name = "Automatic",
    module = "pybevy.window",
    extends = PyWindowPosition,
    frozen
)]
pub struct PyWindowPositionAutomatic;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyWindowPositionAutomatic {
    #[classattr]
    const __qualname__: &'static str = "WindowPosition.Automatic";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyWindowPosition(WindowPosition::Automatic)).add_subclass(Self)
    }
}

#[pyclass(
    name = "Centered",
    module = "pybevy.window",
    extends = PyWindowPosition,
    frozen
)]
pub struct PyWindowPositionCentered;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyWindowPositionCentered {
    #[classattr]
    const __qualname__: &'static str = "WindowPosition.Centered";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyMonitorSelection) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyWindowPosition(WindowPosition::Centered(value.into())))
            .add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyResult<PyMonitorSelection> {
        let base = slf.into_super();
        match &base.0 {
            WindowPosition::Centered(value) => Ok((*value).into()),
            _ => unreachable!("WindowPosition.Centered changed discriminant"),
        }
    }
}

#[pyclass(
    name = "At",
    module = "pybevy.window",
    extends = PyWindowPosition,
    frozen
)]
pub struct PyWindowPositionAt;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyWindowPositionAt {
    #[classattr]
    const __qualname__: &'static str = "WindowPosition.At";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: &PyIVec2) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyWindowPosition(WindowPosition::At(value.into())))
            .add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyResult<PyIVec2> {
        let base = slf.into_super();
        match &base.0 {
            WindowPosition::At(value) => Ok((*value).into()),
            _ => unreachable!("WindowPosition.At changed discriminant"),
        }
    }
}

pub fn materialize_window_position(py: Python<'_>, value: WindowPosition) -> PyResult<Py<PyAny>> {
    match value {
        WindowPosition::Automatic => Ok(Py::new(
            py,
            PyClassInitializer::from(PyWindowPosition(WindowPosition::Automatic))
                .add_subclass(PyWindowPositionAutomatic),
        )?
        .into_any()),
        WindowPosition::Centered(monitor) => Ok(Py::new(
            py,
            PyClassInitializer::from(PyWindowPosition(WindowPosition::Centered(monitor)))
                .add_subclass(PyWindowPositionCentered),
        )?
        .into_any()),
        WindowPosition::At(position) => Ok(Py::new(
            py,
            PyClassInitializer::from(PyWindowPosition(WindowPosition::At(position)))
                .add_subclass(PyWindowPositionAt),
        )?
        .into_any()),
    }
}

pub fn register_window_position_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("WindowPosition")?;
    base.setattr("Automatic", py.get_type::<PyWindowPositionAutomatic>())?;
    base.setattr("Centered", py.get_type::<PyWindowPositionCentered>())?;
    base.setattr("At", py.get_type::<PyWindowPositionAt>())?;
    Ok(())
}

fn monitor_selection_repr(value: &MonitorSelection) -> String {
    match value {
        MonitorSelection::Current => "MonitorSelection.Current()".to_string(),
        MonitorSelection::Primary => "MonitorSelection.Primary()".to_string(),
        MonitorSelection::Index(index) => format!("MonitorSelection.Index({index})"),
        MonitorSelection::Entity(_) => "MonitorSelection.Entity(...)".to_string(),
    }
}
