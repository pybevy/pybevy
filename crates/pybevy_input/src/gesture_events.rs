use bevy::{
    input::gestures::{DoubleTapGesture, PanGesture, PinchGesture, RotationGesture},
    math::Vec2,
};
pub use pybevy_core::PyMessage;
use pybevy_macros::pymessage;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pymessage(PinchGesture)]
#[pyclass(name = "PinchGesture", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyPinchGesture(pub f32);

impl PyPinchGesture {
    pub fn from_bevy(event: &PinchGesture) -> (Self, PyMessage) {
        (PyPinchGesture(event.0), PyMessage)
    }
}

impl From<&PinchGesture> for PyPinchGesture {
    fn from(event: &PinchGesture) -> Self {
        PyPinchGesture(event.0)
    }
}

#[pymethods]
impl PyPinchGesture {
    #[new]
    fn new(value: f32) -> PyClassInitializer<Self> {
        (PyPinchGesture(value), PyMessage).into()
    }

    #[getter]
    fn value(&self) -> f32 {
        self.0
    }

    fn __repr__(&self) -> String {
        format!("PinchGesture({})", self.0)
    }
}

#[pymessage(RotationGesture)]
#[pyclass(name = "RotationGesture", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyRotationGesture(pub f32);

impl PyRotationGesture {
    pub fn from_bevy(event: &RotationGesture) -> (Self, PyMessage) {
        (PyRotationGesture(event.0), PyMessage)
    }
}

impl From<&RotationGesture> for PyRotationGesture {
    fn from(event: &RotationGesture) -> Self {
        PyRotationGesture(event.0)
    }
}

#[pymethods]
impl PyRotationGesture {
    #[new]
    fn new(value: f32) -> PyClassInitializer<Self> {
        (PyRotationGesture(value), PyMessage).into()
    }

    #[getter]
    fn value(&self) -> f32 {
        self.0
    }

    fn __repr__(&self) -> String {
        format!("RotationGesture({})", self.0)
    }
}

#[pymessage(DoubleTapGesture)]
#[pyclass(name = "DoubleTapGesture", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyDoubleTapGesture;

impl PyDoubleTapGesture {
    pub fn from_bevy(_event: &DoubleTapGesture) -> (Self, PyMessage) {
        (PyDoubleTapGesture, PyMessage)
    }
}

impl From<&DoubleTapGesture> for PyDoubleTapGesture {
    fn from(_event: &DoubleTapGesture) -> Self {
        PyDoubleTapGesture
    }
}

#[pymethods]
impl PyDoubleTapGesture {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        (PyDoubleTapGesture, PyMessage).into()
    }

    fn __repr__(&self) -> String {
        "DoubleTapGesture()".to_string()
    }
}

#[pymessage(PanGesture)]
#[pyclass(name = "PanGesture", module = "pybevy.input", extends = PyMessage, eq, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyPanGesture(pub f32, pub f32);

impl PyPanGesture {
    pub fn from_bevy(event: &PanGesture) -> (Self, PyMessage) {
        (PyPanGesture(event.0.x, event.0.y), PyMessage)
    }
}

impl From<&PanGesture> for PyPanGesture {
    fn from(event: &PanGesture) -> Self {
        PyPanGesture(event.0.x, event.0.y)
    }
}

#[pymethods]
impl PyPanGesture {
    #[new]
    fn new(x: f32, y: f32) -> PyClassInitializer<Self> {
        (PyPanGesture(x, y), PyMessage).into()
    }

    #[getter]
    fn x(&self) -> f32 {
        self.0
    }

    #[getter]
    fn y(&self) -> f32 {
        self.1
    }

    #[getter]
    fn delta(&self) -> PyVec2 {
        Vec2::new(self.0, self.1).into()
    }

    fn __repr__(&self) -> String {
        format!("PanGesture(x={}, y={})", self.0, self.1)
    }
}
