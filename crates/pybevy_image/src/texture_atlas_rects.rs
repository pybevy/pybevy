use bevy::math::URect;
use pybevy_core::{FieldStorage, ValueStorage};
use pybevy_math::urect::PyURect;
use pyo3::{prelude::*, types::PyList};

#[pyclass(name = "_TextureAtlasRects", skip_from_py_object)]
#[derive(Clone)]
pub struct PyTextureAtlasRects {
    storage: FieldStorage<Vec<URect>>,
}

pybevy_core::impl_live_field_list!(
    PyTextureAtlasRects,
    "_TextureAtlasRects",
    Vec<URect>,
    URect,
    PyURect,
    ValueStorage<URect>
);
