use bevy::light::Cascades;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};

use crate::cascade::PyCascade;

#[pycomponent(Cascades, no_clone, bridge)]
#[pyclass(name = "Cascades", extends = PyComponent)]
#[derive(Debug)]
pub struct PyCascades {
    pub(crate) storage: ComponentStorage<Cascades>,
}

#[pymethods]
impl PyCascades {
    #[getter]
    pub fn cascades(&self, py: Python) -> PyResult<Py<PyDict>> {
        let cascades = self.as_ref()?;
        let dict = PyDict::new(py);

        for (entity, cascade_vec) in cascades.cascades.iter() {
            let py_cascades: Vec<PyCascade> = cascade_vec.iter().map(PyCascade::from).collect();
            let py_list = PyList::new(py, py_cascades)?;
            dict.set_item(entity.index_u32(), py_list)?;
        }

        Ok(dict.into())
    }

    fn __repr__(&self) -> PyResult<String> {
        let cascades = self.as_ref()?;
        let total_views = cascades.cascades.len();
        Ok(format!("Cascades(views={})", total_views))
    }
}
