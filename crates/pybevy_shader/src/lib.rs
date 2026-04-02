pub mod shader;
pub mod shader_def_val;
pub mod shader_import;
pub mod shader_ref;
pub mod shader_source;
pub mod validate_shader;

use pyo3::prelude::*;
pub use shader::PyShader;
pub use shader_def_val::PyShaderDefVal;
pub use shader_import::PyShaderImport;
pub use shader_ref::PyShaderRef;
pub use shader_source::PySource;
pub use validate_shader::PyValidateShader;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "shader")?;
    m.add_class::<PyShader>()?;
    m.add_class::<PyShaderDefVal>()?;
    m.add_class::<PyShaderImport>()?;
    m.add_class::<PyShaderRef>()?;
    m.add_class::<PySource>()?;
    m.add_class::<PyValidateShader>()?;
    parent.add_submodule(&m)
}
