pub mod shader;
pub mod shader_def_val;
pub mod shader_import;
pub mod shader_ref;
pub mod shader_source;
pub mod validate_shader;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::shader::PyShader;
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "shader")?;
    m.add_class::<shader::PyShader>()?;
    m.add_class::<shader_def_val::PyShaderDefVal>()?;
    m.add_class::<shader_import::PyShaderImport>()?;
    m.add_class::<shader_ref::PyShaderRef>()?;
    m.add_class::<shader_source::PySource>()?;
    m.add_class::<validate_shader::PyValidateShader>()?;
    parent.add_submodule(&m)
}
