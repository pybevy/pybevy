//! Message base types for PyBevy
//!
//! This module provides the base classes for Bevy messages exposed to Python.

use std::any::Any;

use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};

/// Base class for all PyBevy messages.
///
/// Python user-defined messages should inherit from this class:
///
/// ```python
/// from dataclasses import dataclass
/// from pybevy import Message
///
/// @dataclass
/// class MyMessage(Message):
///     value: int
///     text: str
/// ```
#[pyclass(name = "Message", subclass, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMessage;

#[pymethods]
impl PyMessage {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn new(_args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        PyMessage
    }
}

/// A unique identifier for a sent message.
///
/// Returned by `MessageWriter.write()` and can be used to track message delivery.
#[pyclass(name = "MessageId", frozen)]
#[derive(Debug)]
pub struct PyMessageId(#[allow(dead_code)] pub(crate) Box<dyn Any + Send + Sync>);

impl PyMessageId {
    /// Create a new message ID from any type that is Send + Sync.
    pub fn new<T: Any + Send + Sync>(id: T) -> Self {
        PyMessageId(Box::new(id))
    }

    /// Create a new message ID from a boxed value.
    pub fn from_boxed(id: Box<dyn Any + Send + Sync>) -> Self {
        PyMessageId(id)
    }
}
