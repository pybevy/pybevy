use bevy::shader::{Shader, ShaderDefVal};
use naga::ShaderStage;
use pybevy_core::AssetStorage;
use pybevy_macros::pyasset;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{
    shader_def_val::PyShaderDefVal, shader_import::PyShaderImport, shader_source::PySource,
    validate_shader::PyValidateShader,
};

#[pyasset(Shader, bridge)]
#[pyclass(name = "Shader", extends = pybevy_core::PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyShader {
    storage: AssetStorage<Shader>,
}

#[pymethods]
impl PyShader {
    #[staticmethod]
    pub fn from_wgsl(py: Python<'_>, source: String, path: String) -> PyResult<Py<PyShader>> {
        let shader = Shader::from_wgsl(source, path);
        Py::new(py, Self::from_owned(shader))
    }

    #[staticmethod]
    pub fn from_wgsl_with_defs(
        py: Python<'_>,
        source: String,
        path: String,
        shader_defs: Vec<PyShaderDefVal>,
    ) -> PyResult<Py<PyShader>> {
        let defs: Vec<ShaderDefVal> = shader_defs.into_iter().map(|d| d.into()).collect();
        let shader = Shader::from_wgsl_with_defs(source, path, defs);
        Py::new(py, Self::from_owned(shader))
    }

    #[staticmethod]
    pub fn from_glsl(
        py: Python<'_>,
        source: String,
        stage: String,
        path: String,
    ) -> PyResult<Py<PyShader>> {
        let shader_stage = match stage.as_str() {
            "vertex" => ShaderStage::Vertex,
            "fragment" => ShaderStage::Fragment,
            "compute" => ShaderStage::Compute,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "Invalid shader stage: {}. Must be 'vertex', 'fragment', or 'compute'",
                    stage
                )));
            }
        };

        let shader = Shader::from_glsl(source, shader_stage, path);
        Py::new(py, Self::from_owned(shader))
    }

    #[staticmethod]
    pub fn from_spirv(py: Python<'_>, source: Vec<u8>, path: String) -> PyResult<Py<PyShader>> {
        let shader = Shader::from_spirv(source, path);
        Py::new(py, Self::from_owned(shader))
    }

    #[getter]
    pub fn path(&self) -> PyResult<String> {
        Ok(self.as_ref()?.path.clone())
    }

    #[getter]
    pub fn source(&self) -> PyResult<PySource> {
        Ok((&self.as_ref()?.source).into())
    }

    #[getter]
    pub fn import_path(&self) -> PyResult<PyShaderImport> {
        Ok((&self.as_ref()?.import_path).into())
    }

    #[setter]
    pub fn set_import_path(&mut self, import_path: PyShaderImport) -> PyResult<()> {
        self.as_mut()?.import_path = import_path.into();
        Ok(())
    }

    #[getter]
    pub fn imports(&self) -> PyResult<Vec<PyShaderImport>> {
        Ok(self.as_ref()?.imports.iter().map(|i| i.into()).collect())
    }

    #[getter]
    pub fn shader_defs(&self) -> PyResult<Vec<PyShaderDefVal>> {
        Ok(self
            .as_ref()?
            .shader_defs
            .iter()
            .map(|d| d.into())
            .collect())
    }

    #[getter]
    pub fn validate_shader(&self) -> PyResult<PyValidateShader> {
        Ok((&self.as_ref()?.validate_shader).into())
    }

    #[setter]
    pub fn set_validate_shader(&mut self, validate: PyValidateShader) -> PyResult<()> {
        self.as_mut()?.validate_shader = validate.into();
        Ok(())
    }

    fn __repr__(&self) -> PyResult<String> {
        let shader = self.as_ref()?;
        Ok(format!("Shader(path=\"{}\")", shader.path))
    }
}
