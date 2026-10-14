//! Message base types for PyBevy
//!
//! This module provides the base classes for Bevy messages exposed to Python.

use std::{
    any::TypeId,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use pyo3::{
    IntoPyObjectExt,
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
#[pyclass(name = "Message", module = "pybevy.ecs", subclass, skip_from_py_object)]
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

/// A Bevy-style identifier for a sent message.
///
/// Returned by `MessageWriter.write()` and can be used to track message delivery.
#[pyclass(name = "MessageId", module = "pybevy.ecs", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMessageId {
    identity: MessageIdentity,
    message_type: String,
    id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MessageIdentity {
    Native(TypeId),
    Custom(String),
}

impl PyMessageId {
    pub fn new(message_type: impl Into<String>, id: u64) -> Self {
        let message_type = message_type.into();
        PyMessageId {
            identity: MessageIdentity::Custom(message_type.clone()),
            message_type,
            id,
        }
    }

    pub fn native(type_id: TypeId, name: &str, id: u64) -> Self {
        Self {
            identity: MessageIdentity::Native(type_id),
            message_type: name.to_owned(),
            id,
        }
    }
}

#[pymethods]
impl PyMessageId {
    /// Order in which this message was written to its World and message type.
    #[getter]
    pub fn id(&self) -> u64 {
        self.id
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (&self.identity, self.id).hash(&mut hasher);
        hasher.finish()
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: pyo3::basic::CompareOp,
    ) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let Ok(other) = other.cast::<PyMessageId>() else {
            return py.NotImplemented().into_py_any(py);
        };
        let other = other.borrow();
        let result = match op {
            pyo3::basic::CompareOp::Eq => self.identity == other.identity && self.id == other.id,
            pyo3::basic::CompareOp::Ne => self.identity != other.identity || self.id != other.id,
            _ if self.identity != other.identity => {
                return py.NotImplemented().into_py_any(py);
            }
            pyo3::basic::CompareOp::Lt => self.id < other.id,
            pyo3::basic::CompareOp::Le => self.id <= other.id,
            pyo3::basic::CompareOp::Gt => self.id > other.id,
            pyo3::basic::CompareOp::Ge => self.id >= other.id,
        };
        result.into_py_any(py)
    }

    fn __repr__(&self) -> String {
        let message_type = self
            .message_type
            .rsplit('.')
            .next()
            .unwrap_or(&self.message_type);
        format!("message<{message_type}>#{}", self.id)
    }
}

/// An enum payload that stores a native message and appears in Python as the
/// message's own wrapper class; `materialize = path` overrides the projection.
#[macro_export]
macro_rules! message_payload {
    ($payload:ident, $native:ty, $wrapper:ty) => {
        $crate::message_payload!(@define $payload, $native, $wrapper, |py, event| {
            ::pyo3::Py::new(py, (<$wrapper>::from(event), $crate::PyMessage))
                .map(::pyo3::Py::into_any)
        });
    };
    ($payload:ident, $native:ty, $wrapper:ty, materialize = $materialize:path) => {
        $crate::message_payload!(@define $payload, $native, $wrapper, $materialize);
    };
    (@define $payload:ident, $native:ty, $wrapper:ty, $materialize:expr) => {
        pub struct $payload($native);

        impl From<$native> for $payload {
            fn from(value: $native) -> Self {
                Self(value)
            }
        }

        impl From<$payload> for $native {
            fn from(value: $payload) -> Self {
                value.0
            }
        }

        impl<'py> ::pyo3::IntoPyObject<'py> for $payload {
            type Target = ::pyo3::PyAny;
            type Output = ::pyo3::Bound<'py, ::pyo3::PyAny>;
            type Error = ::pyo3::PyErr;

            fn into_pyobject(self, py: ::pyo3::Python<'py>) -> ::pyo3::PyResult<Self::Output> {
                let materialize: fn(
                    ::pyo3::Python<'_>,
                    &$native,
                ) -> ::pyo3::PyResult<::pyo3::Py<::pyo3::PyAny>> = $materialize;
                Ok(materialize(py, &self.0)?.into_bound(py))
            }
        }

        impl ::pyo3::FromPyObject<'_, '_> for $payload {
            type Error = ::pyo3::PyErr;

            fn extract(value: ::pyo3::Borrowed<'_, '_, ::pyo3::PyAny>) -> ::pyo3::PyResult<Self> {
                let wrapper = value.extract::<::pyo3::PyRef<'_, $wrapper>>()?;
                Ok(Self(<$native>::try_from(&*wrapper)?))
            }
        }
    };
}
