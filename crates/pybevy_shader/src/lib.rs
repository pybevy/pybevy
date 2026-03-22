pub mod shader;
pub mod shader_def_val;
pub mod shader_import;
pub mod shader_ref;
pub mod shader_source;
pub mod validate_shader;

use bevy::shader::Shader;
use pybevy_core::registry::global_registry;
use pybevy_macros::asset_bridge;
use pyo3::prelude::*;
pub use shader::PyShader;
pub use shader_def_val::PyShaderDefVal;
pub use shader_import::PyShaderImport;
pub use shader_ref::PyShaderRef;
pub use shader_source::PySource;
pub use validate_shader::PyValidateShader;

asset_bridge!(Shader, PyShader);

pub fn register_shader_bridges() {
    global_registry::register_asset_bridge(ShaderBridge);
}

pub fn add_shader_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_shader_bridges();

    m.add_class::<PyShader>()?;
    m.add_class::<PyShaderDefVal>()?;
    m.add_class::<PyShaderImport>()?;
    m.add_class::<PyShaderRef>()?;
    m.add_class::<PySource>()?;
    m.add_class::<PyValidateShader>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "shader")?;
    add_shader_classes(&m)?;
    parent.add_submodule(&m)
}
