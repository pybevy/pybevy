use bevy::sprite_render::AlphaMode2d;
use pyo3::prelude::*;

#[pyclass(name = "AlphaMode2d", from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyAlphaMode2d(AlphaMode2d);

#[pymethods]
impl PyAlphaMode2d {
    #[staticmethod]
    pub fn opaque() -> Self {
        PyAlphaMode2d(AlphaMode2d::Opaque)
    }

    #[staticmethod]
    pub fn blend() -> Self {
        PyAlphaMode2d(AlphaMode2d::Blend)
    }

    #[staticmethod]
    pub fn mask(threshold: f32) -> Self {
        PyAlphaMode2d(AlphaMode2d::Mask(threshold))
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            AlphaMode2d::Opaque => "AlphaMode2d.opaque()".to_string(),
            AlphaMode2d::Blend => "AlphaMode2d.blend()".to_string(),
            AlphaMode2d::Mask(threshold) => format!("AlphaMode2d.mask({})", threshold),
        }
    }
}

impl Default for PyAlphaMode2d {
    fn default() -> Self {
        PyAlphaMode2d(AlphaMode2d::Opaque)
    }
}

impl From<PyAlphaMode2d> for AlphaMode2d {
    fn from(py_mode: PyAlphaMode2d) -> Self {
        py_mode.0
    }
}

impl From<AlphaMode2d> for PyAlphaMode2d {
    fn from(mode: AlphaMode2d) -> Self {
        PyAlphaMode2d(mode)
    }
}
