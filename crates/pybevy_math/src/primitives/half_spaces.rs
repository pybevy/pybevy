use bevy::shape::HalfSpace;
use pybevy_core::FieldStorage;
use pyo3::{prelude::*, types::PyList};

use super::half_space::PyHalfSpace;

/// The six half-spaces of a `ViewFrustum`, assignable by index.
#[pyclass(name = "_HalfSpaces", module = "pybevy.math", skip_from_py_object)]
#[derive(Clone)]
pub struct PyHalfSpaces {
    storage: FieldStorage<[HalfSpace; 6]>,
}

pybevy_core::impl_live_value_array!(
    PyHalfSpaces,
    "_HalfSpaces",
    [HalfSpace; 6],
    HalfSpace,
    PyHalfSpace,
    fixed = "ViewFrustum.half_spaces"
);
