use bevy::{light::ParallaxCorrection, math::Vec3};
use pybevy_core::ComponentStorage;
use pybevy_macros::pyenum;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

#[pyenum(ParallaxCorrection, component)]
#[pyclass(name = "ParallaxCorrection", module = "pybevy.light")]
pub enum PyParallaxCorrection {
    #[pyo3(name = "None_")]
    None(),
    Auto(),
    #[py_bevy(tuple)]
    Custom {
        #[py_type(PyVec3)]
        #[py_try_into]
        value: Vec3,
    },
}

#[pymethods]
impl PyParallaxCorrection {
    pub fn __repr__(&self) -> PyResult<String> {
        match &*self.as_ref()? {
            ParallaxCorrection::None => Ok("ParallaxCorrection.None_()".to_string()),
            ParallaxCorrection::Auto => Ok("ParallaxCorrection.Auto()".to_string()),
            ParallaxCorrection::Custom(value) => Ok(format!(
                "ParallaxCorrection.Custom(value=Vec3({}, {}, {}))",
                value.x, value.y, value.z
            )),
        }
    }
}
