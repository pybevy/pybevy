use bevy::shader::{ShaderDefVal, ShaderImport};
use pybevy_core::FieldStorage;
use pyo3::{prelude::*, types::PyList};

use crate::{shader_def_val::PyShaderDefVal, shader_import::PyShaderImport};

#[pyclass(name = "_ShaderImports", module = "pybevy.shader", skip_from_py_object)]
#[derive(Clone)]
pub struct PyShaderImports {
    storage: FieldStorage<Vec<ShaderImport>>,
}

pybevy_core::impl_live_value_list!(
    PyShaderImports,
    "_ShaderImports",
    Vec<ShaderImport>,
    ShaderImport,
    PyShaderImport
);

#[pyclass(name = "_ShaderDefs", module = "pybevy.shader", skip_from_py_object)]
#[derive(Clone)]
pub struct PyShaderDefs {
    storage: FieldStorage<Vec<ShaderDefVal>>,
}

pybevy_core::impl_live_value_list!(
    PyShaderDefs,
    "_ShaderDefs",
    Vec<ShaderDefVal>,
    ShaderDefVal,
    PyShaderDefVal
);
