use bevy::{
    input::gestures::{DoubleTapGesture, PanGesture, PinchGesture, RotationGesture},
    math::Vec2,
};
pub use pybevy_core::PyMessage;
use pybevy_macros::message_bridge;
use pybevy_math::PyVec2;
use pyo3::prelude::*;

// ============================================================================
// PinchGesture
// ============================================================================

#[pyclass(name = "PinchGesture", extends = PyMessage, eq)]
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
    fn new(value: f32) -> (Self, PyMessage) {
        (PyPinchGesture(value), PyMessage)
    }

    #[getter]
    fn value(&self) -> f32 {
        self.0
    }

    fn __repr__(&self) -> String {
        format!("PinchGesture({})", self.0)
    }
}

// ============================================================================
// RotationGesture
// ============================================================================

#[pyclass(name = "RotationGesture", extends = PyMessage, eq)]
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
    fn new(value: f32) -> (Self, PyMessage) {
        (PyRotationGesture(value), PyMessage)
    }

    #[getter]
    fn value(&self) -> f32 {
        self.0
    }

    fn __repr__(&self) -> String {
        format!("RotationGesture({})", self.0)
    }
}

// ============================================================================
// DoubleTapGesture
// ============================================================================

#[pyclass(name = "DoubleTapGesture", extends = PyMessage, eq)]
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
    fn new() -> (Self, PyMessage) {
        (PyDoubleTapGesture, PyMessage)
    }

    fn __repr__(&self) -> String {
        "DoubleTapGesture()".to_string()
    }
}

// ============================================================================
// PanGesture
// ============================================================================

#[pyclass(name = "PanGesture", extends = PyMessage, eq)]
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
    fn new(x: f32, y: f32) -> (Self, PyMessage) {
        (PyPanGesture(x, y), PyMessage)
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

// Message bridges
message_bridge!(PinchGesture, PyPinchGesture);
message_bridge!(RotationGesture, PyRotationGesture);
message_bridge!(DoubleTapGesture, PyDoubleTapGesture);
message_bridge!(PanGesture, PyPanGesture);
