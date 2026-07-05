use bevy::light::ParallaxCorrection;
use pybevy_core::PyComponent;
use pybevy_macros::pywrap;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pywrap(ParallaxCorrection, bridge, copy)]
#[pyclass(from_py_object, name = "ParallaxCorrection", extends = PyComponent, frozen, eq)]
#[derive(Clone, Copy)]
pub struct PyParallaxCorrection(pub(crate) ParallaxCorrection);

impl PartialEq for PyParallaxCorrection {
    fn eq(&self, other: &Self) -> bool {
        match (self.0, other.0) {
            (ParallaxCorrection::None, ParallaxCorrection::None) => true,
            (ParallaxCorrection::Auto, ParallaxCorrection::Auto) => true,
            (ParallaxCorrection::Custom(a), ParallaxCorrection::Custom(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyParallaxCorrection {
    #[new]
    pub fn new(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ParallaxCorrection::default()))
    }

    #[classattr]
    #[pyo3(name = "NONE")]
    pub fn none(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ParallaxCorrection::None))
    }

    #[classattr]
    #[pyo3(name = "AUTO")]
    pub fn auto(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, Self::from_owned(ParallaxCorrection::Auto))
    }

    #[staticmethod]
    pub fn custom(py: Python, half_extents: PyVec3) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(ParallaxCorrection::Custom(half_extents.into())),
        )
    }

    #[getter]
    pub fn custom_half_extents(&self) -> Option<PyVec3> {
        match self.0 {
            ParallaxCorrection::Custom(v) => Some(v.into()),
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            ParallaxCorrection::None => "ParallaxCorrection.NONE".to_string(),
            ParallaxCorrection::Auto => "ParallaxCorrection.AUTO".to_string(),
            ParallaxCorrection::Custom(v) => {
                format!("ParallaxCorrection.custom(Vec3({}, {}, {}))", v.x, v.y, v.z)
            }
        }
    }
}
