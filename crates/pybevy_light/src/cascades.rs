use bevy::light::Cascades;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity, computed_owned};
use pybevy_macros::pycomponent;
use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};

use crate::cascade::PyCascade;

#[pycomponent(Cascades, bridge)]
#[pyclass(name = "Cascades", module = "pybevy.light", extends = PyComponent)]
#[derive(Debug)]
pub struct PyCascades {
    pub(crate) storage: ComponentStorage<Cascades>,
}

#[pymethods]
impl PyCascades {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(Cascades::default()).into()
    }

    #[getter]
    pub fn cascades(&self, py: Python) -> PyResult<Py<PyDict>> {
        let cascades = self.as_ref()?;
        let dict = PyDict::new(py);

        for (entity, cascade_vec) in cascades.cascades.iter() {
            let py_cascades: Vec<PyCascade> = cascade_vec.iter().map(PyCascade::from).collect();
            let py_list = PyList::new(py, py_cascades)?;
            dict.set_item(PyEntity(*entity), py_list)?;
        }

        Ok(computed_owned(dict.into()))
    }

    fn __repr__(&self) -> PyResult<String> {
        let cascades = self.as_ref()?;
        let total_views = cascades.cascades.len();
        Ok(format!("Cascades(views={})", total_views))
    }
}
