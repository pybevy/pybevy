use bevy::{
    math::IVec2,
    window::{
        WindowBackendScaleFactorChanged, WindowCreated, WindowDestroyed, WindowMoved,
        WindowOccluded, WindowScaleFactorChanged, WindowThemeChanged,
    },
};
use pybevy_core::{PyEntity, PyMessage, repr::ReprLiteral};
use pybevy_macros::pymessage;
use pybevy_math::snapshot::Snapshot;
use pyo3::prelude::*;

use crate::window_theme::PyWindowTheme;

#[pymessage(WindowCreated)]
#[pyclass(name = "WindowCreated", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowCreated {
    pub window: PyEntity,
}

impl From<&WindowCreated> for PyWindowCreated {
    fn from(event: &WindowCreated) -> Self {
        PyWindowCreated {
            window: event.window.into(),
        }
    }
}

impl From<&PyWindowCreated> for WindowCreated {
    fn from(value: &PyWindowCreated) -> Self {
        WindowCreated {
            window: value.window.into(),
        }
    }
}

#[pymethods]
impl PyWindowCreated {
    #[new]
    #[pyo3(signature = (*, window))]
    fn new(window: PyEntity) -> PyClassInitializer<Self> {
        (PyWindowCreated { window }, PyMessage).into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        "WindowCreated()".to_string()
    }
}

#[pymessage(WindowDestroyed)]
#[pyclass(name = "WindowDestroyed", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowDestroyed {
    pub window: PyEntity,
}

impl From<&WindowDestroyed> for PyWindowDestroyed {
    fn from(event: &WindowDestroyed) -> Self {
        PyWindowDestroyed {
            window: event.window.into(),
        }
    }
}

impl From<&PyWindowDestroyed> for WindowDestroyed {
    fn from(value: &PyWindowDestroyed) -> Self {
        WindowDestroyed {
            window: value.window.into(),
        }
    }
}

#[pymethods]
impl PyWindowDestroyed {
    #[new]
    #[pyo3(signature = (*, window))]
    fn new(window: PyEntity) -> PyClassInitializer<Self> {
        (PyWindowDestroyed { window }, PyMessage).into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    fn __repr__(&self) -> String {
        "WindowDestroyed()".to_string()
    }
}

#[pymessage(WindowMoved)]
#[pyclass(name = "WindowMoved", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowMoved {
    pub window: PyEntity,
    pub position: IVec2,
}

impl From<&WindowMoved> for PyWindowMoved {
    fn from(event: &WindowMoved) -> Self {
        PyWindowMoved {
            window: event.window.into(),
            position: event.position,
        }
    }
}

impl From<&PyWindowMoved> for WindowMoved {
    fn from(value: &PyWindowMoved) -> Self {
        WindowMoved {
            window: value.window.into(),
            position: value.position,
        }
    }
}

#[pymethods]
impl PyWindowMoved {
    #[new]
    #[pyo3(signature = (*, window, position))]
    fn new(window: PyEntity, position: Snapshot<IVec2>) -> PyClassInitializer<Self> {
        (
            PyWindowMoved {
                window,
                position: position.0,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[getter]
    fn position(&self) -> Snapshot<IVec2> {
        Snapshot(self.position)
    }

    fn __repr__(&self) -> String {
        format!(
            "WindowMoved(position=IVec2({}, {}))",
            self.position.x, self.position.y
        )
    }
}

#[pymessage(WindowOccluded)]
#[pyclass(name = "WindowOccluded", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowOccluded {
    pub window: PyEntity,
    pub occluded: bool,
}

impl From<&WindowOccluded> for PyWindowOccluded {
    fn from(event: &WindowOccluded) -> Self {
        PyWindowOccluded {
            window: event.window.into(),
            occluded: event.occluded,
        }
    }
}

impl From<&PyWindowOccluded> for WindowOccluded {
    fn from(value: &PyWindowOccluded) -> Self {
        WindowOccluded {
            window: value.window.into(),
            occluded: value.occluded,
        }
    }
}

#[pymethods]
impl PyWindowOccluded {
    #[new]
    #[pyo3(signature = (*, window, occluded))]
    fn new(window: PyEntity, occluded: bool) -> PyClassInitializer<Self> {
        (PyWindowOccluded { window, occluded }, PyMessage).into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[getter]
    fn occluded(&self) -> bool {
        self.occluded
    }

    fn __repr__(&self) -> String {
        format!("WindowOccluded(occluded={})", self.occluded.repr_literal())
    }
}

#[pymessage(WindowScaleFactorChanged)]
#[pyclass(name = "WindowScaleFactorChanged", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowScaleFactorChanged {
    pub window: PyEntity,
    pub scale_factor: f64,
}

impl From<&WindowScaleFactorChanged> for PyWindowScaleFactorChanged {
    fn from(event: &WindowScaleFactorChanged) -> Self {
        PyWindowScaleFactorChanged {
            window: event.window.into(),
            scale_factor: event.scale_factor,
        }
    }
}

impl From<&PyWindowScaleFactorChanged> for WindowScaleFactorChanged {
    fn from(value: &PyWindowScaleFactorChanged) -> Self {
        WindowScaleFactorChanged {
            window: value.window.into(),
            scale_factor: value.scale_factor,
        }
    }
}

#[pymethods]
impl PyWindowScaleFactorChanged {
    #[new]
    #[pyo3(signature = (*, window, scale_factor))]
    fn new(window: PyEntity, scale_factor: f64) -> PyClassInitializer<Self> {
        (
            PyWindowScaleFactorChanged {
                window,
                scale_factor,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[getter]
    fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    fn __repr__(&self) -> String {
        format!(
            "WindowScaleFactorChanged(scale_factor={})",
            self.scale_factor.repr_literal()
        )
    }
}

#[pymessage(WindowBackendScaleFactorChanged)]
#[pyclass(name = "WindowBackendScaleFactorChanged", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowBackendScaleFactorChanged {
    pub window: PyEntity,
    pub scale_factor: f64,
}

impl From<&WindowBackendScaleFactorChanged> for PyWindowBackendScaleFactorChanged {
    fn from(event: &WindowBackendScaleFactorChanged) -> Self {
        PyWindowBackendScaleFactorChanged {
            window: event.window.into(),
            scale_factor: event.scale_factor,
        }
    }
}

impl From<&PyWindowBackendScaleFactorChanged> for WindowBackendScaleFactorChanged {
    fn from(value: &PyWindowBackendScaleFactorChanged) -> Self {
        WindowBackendScaleFactorChanged {
            window: value.window.into(),
            scale_factor: value.scale_factor,
        }
    }
}

#[pymethods]
impl PyWindowBackendScaleFactorChanged {
    #[new]
    #[pyo3(signature = (*, window, scale_factor))]
    fn new(window: PyEntity, scale_factor: f64) -> PyClassInitializer<Self> {
        (
            PyWindowBackendScaleFactorChanged {
                window,
                scale_factor,
            },
            PyMessage,
        )
            .into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[getter]
    fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    fn __repr__(&self) -> String {
        format!(
            "WindowBackendScaleFactorChanged(scale_factor={})",
            self.scale_factor.repr_literal()
        )
    }
}

#[pymessage(WindowThemeChanged)]
#[pyclass(name = "WindowThemeChanged", module = "pybevy.window", extends = PyMessage, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWindowThemeChanged {
    pub window: PyEntity,
    pub theme: PyWindowTheme,
}

impl From<&WindowThemeChanged> for PyWindowThemeChanged {
    fn from(event: &WindowThemeChanged) -> Self {
        PyWindowThemeChanged {
            window: event.window.into(),
            theme: event.theme.into(),
        }
    }
}

impl From<&PyWindowThemeChanged> for WindowThemeChanged {
    fn from(value: &PyWindowThemeChanged) -> Self {
        WindowThemeChanged {
            window: value.window.into(),
            theme: value.theme.into(),
        }
    }
}

#[pymethods]
impl PyWindowThemeChanged {
    #[new]
    #[pyo3(signature = (*, window, theme))]
    fn new(window: PyEntity, theme: PyWindowTheme) -> PyClassInitializer<Self> {
        (PyWindowThemeChanged { window, theme }, PyMessage).into()
    }

    #[getter]
    fn window(&self) -> PyEntity {
        self.window
    }

    #[getter]
    fn theme(&self) -> PyWindowTheme {
        self.theme
    }

    fn __repr__(&self) -> String {
        format!("WindowThemeChanged(theme=WindowTheme.{:?})", self.theme)
    }
}
