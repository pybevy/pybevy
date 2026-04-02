pub use pybevy_macros::{
    PyComponent, native_asset, native_component, native_field, native_resource, pybevy_app,
};
use pyo3::{prelude::*, types::IntoPyDict};

pub mod prelude {
    pub use pybevy_macros::{
        PyComponent, native_asset, native_component, native_field, native_resource, pybevy_app,
    };

    pub use crate::app::app::PyApp;
}

pub use app::PyStage;
pub use pybevy_core;
pub use pyo3;

#[cfg(feature = "native-plugin")]
pub mod plugin;
#[cfg(feature = "native-plugin")]
pub use plugin::PyBevyPlugin;

pub(crate) mod app;
pub(crate) mod assets;
pub(crate) mod ecs;
pub(crate) mod render;

#[pyfunction]
fn _color_materialize(color: &pybevy_color::PyColor, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let material = bevy::pbr::StandardMaterial {
        base_color: color.0,
        ..Default::default()
    };
    let py_material: Py<pybevy_pbr::PyStandardMaterial> =
        Py::new(py, (material.into(), pybevy_core::PyAsset))?;
    Ok(py_material.into_any())
}

/// Populates a PyModule with all pybevy submodules and classes.
/// Called by both the cdylib (pybevy_python) and native plugin (append_to_inittab).
pub fn init_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Collect all inventory-registered bridges from feature crates
    pybevy_core::bridge_inventory::collect_all();

    // Register base classes from pybevy_core FIRST before any modules that use them
    // This ensures classes like GlobalVolume (extends PyResource) use the same base class
    m.add_class::<pybevy_core::PyResource>()?;
    m.add_class::<pybevy_core::PyComponent>()?;
    m.add_class::<pybevy_core::PyPlugin>()?;

    // Feature crate modules (fully owned by crates)
    pybevy_a11y::add_module(m)?;
    pybevy_animation::add_module(m)?;
    pybevy_audio::add_module(m)?;
    pybevy_camera::add_module(m)?;
    pybevy_core_pipeline::add_module(m)?;
    pybevy_color::add_module(m)?;
    pybevy_gltf::add_module(m)?;
    pybevy_image::add_module(m)?;
    pybevy_input::add_module(m)?;
    pybevy_light::add_module(m)?;
    pybevy_math::add_module(m)?;
    pybevy_mesh::add_module(m)?;
    pybevy_pbr::add_module(m)?;
    pybevy_post_process::add_module(m)?;
    pybevy_render::add_module(m)?;
    pybevy_shader::add_module(m)?;
    pybevy_wgpu::add_module(m)?;
    pybevy_scene::add_module(m)?;
    pybevy_sprite::add_module(m)?;
    pybevy_text::add_module(m)?;
    pybevy_time::add_module(m)?;
    pybevy_ui::add_module(m)?;
    pybevy_transform::add_module(m)?;
    pybevy_window::add_module(m)?;
    pybevy_window::add_winit_module(m)?;

    #[cfg(feature = "mcp")]
    {
        pybevy_control::add_module(m)?;
        pybevy_control::register_world_wrapper_hook(ecs::world::create_world_wrapper);
    }

    // Main crate modules (local code)
    app::add_module(m)?;
    assets::add_module(m)?;
    ecs::add_module(m)?;
    render::add_module(m)?;

    // Enrich "math" module with meshable primitives from pybevy_mesh.
    //
    // In Bevy, types like Circle and CircularSector are defined in bevy_math and
    // made meshable via `impl Meshable for Circle` in bevy_mesh. Rust's trait
    // system allows this cross-crate impl without moving the type.
    //
    // PyO3 has no equivalent: `#[pyclass(extends = PyMeshable)]` must be on the
    // struct definition itself, so these wrappers must live in pybevy_mesh (where
    // PyMeshable is defined). But users expect `from pybevy.math import Circle`,
    // so we inject them into the already-created math module here.
    {
        let math = m.getattr("math")?.cast_into::<PyModule>()?;
        pybevy_mesh::add_math_primitives(&math)?;
    }

    // Enrich "color" module with materialize monkey-patch
    // (Color.materialize() creates StandardMaterial, used by Assets[StandardMaterial].add(Color))
    {
        let py = m.py();
        let color_mod = m.getattr("color")?.cast_into::<PyModule>()?;
        color_mod.add_function(wrap_pyfunction!(_color_materialize, &color_mod)?)?;
        let color_class = color_mod.getattr("Color")?;
        let func = color_mod.getattr("_color_materialize")?;
        py.run(
            c"Color.materialize = lambda self: _func(self)",
            Some(&[("Color", &color_class), ("_func", &func)].into_py_dict(py)?),
            None,
        )?;
    }

    // Register atexit handler to clean up Apps before Python's TLS destructors run
    // This prevents crashes when Apps with Python resources try to access
    // already-destroyed PyO3 thread-locals during cleanup
    let py = m.py();
    let atexit = py.import("atexit")?;
    let cleanup_fn = wrap_pyfunction!(app::app::cleanup_apps_on_shutdown, m)?;
    atexit.call_method1("register", (cleanup_fn,))?;

    Ok(())
}

/// The pymodule entry point — used by append_to_inittab! in native plugin mode.
///
/// Gated behind `native-plugin` feature to avoid generating a duplicate `PyInit__pybevy`
/// symbol when the rlib is linked into pybevy-python (cdylib), which defines its own.
#[cfg(feature = "native-plugin")]
#[pymodule(gil_used = false)]
pub fn _pybevy(m: &Bound<'_, PyModule>) -> PyResult<()> {
    init_module(m)
}
