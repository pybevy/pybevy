use bevy::light::atmosphere::ScatteringTerm;
use pybevy_core::FieldStorage;
use pyo3::{prelude::*, types::PyList};
use smallvec::SmallVec;

use crate::scattering_term::PyScatteringTerm;

type NativeScatteringTerms = SmallVec<[ScatteringTerm; 1]>;

#[pyclass(name = "_ScatteringTerms", skip_from_py_object)]
#[derive(Clone)]
pub struct PyScatteringTerms {
    storage: FieldStorage<NativeScatteringTerms>,
}

pybevy_core::impl_live_field_list!(
    PyScatteringTerms,
    "_ScatteringTerms",
    NativeScatteringTerms,
    ScatteringTerm,
    PyScatteringTerm,
    FieldStorage<ScatteringTerm>
);
