use pybevy_audio::PyAudioPlugin;
use pybevy_image::plugin::PyImagePlugin;
use pybevy_render::plugin::PyRenderPlugin;
use pybevy_window::window_plugin::PyWindowPlugin;
use pybevy_winit::plugin::PyWinitPlugin;
use pyo3::{PyTypeInfo, exceptions::PyTypeError, prelude::*, types::PyType};

use crate::app::task_pool::PyTaskPoolPlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginConfigType {
    Audio,
    Image,
    Render,
    TaskPool,
    Window,
    Winit,
}

impl PluginConfigType {
    pub fn from_py_type(py: Python<'_>, py_type: &Bound<'_, PyType>) -> PyResult<Self> {
        // FIXME: auto-generate this mapping from the plugin type definitions to avoid hardcoding
        if py_type.is(<PyWindowPlugin as PyTypeInfo>::type_object(py)) {
            return Ok(PluginConfigType::Window);
        }
        if py_type.is(<PyAudioPlugin as PyTypeInfo>::type_object(py)) {
            return Ok(PluginConfigType::Audio);
        }
        if py_type.is(<PyTaskPoolPlugin as PyTypeInfo>::type_object(py)) {
            return Ok(PluginConfigType::TaskPool);
        }
        if py_type.is(<PyRenderPlugin as PyTypeInfo>::type_object(py)) {
            return Ok(PluginConfigType::Render);
        }
        if py_type.is(<PyImagePlugin as PyTypeInfo>::type_object(py)) {
            return Ok(PluginConfigType::Image);
        }
        if py_type.is(<PyWinitPlugin as PyTypeInfo>::type_object(py)) {
            return Ok(PluginConfigType::Winit);
        }

        let type_name = py_type.name()?.to_string();
        Err(PyTypeError::new_err(format!(
            "Unknown plugin type: {}",
            type_name
        )))
    }
}
