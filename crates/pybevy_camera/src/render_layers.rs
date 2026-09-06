use bevy::camera::visibility::RenderLayers;
use pybevy_core::{PyComponent, pycomponent::ComponentStorage};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pycomponent(RenderLayers, bridge)]
#[pyclass(name = "RenderLayers", module = "pybevy.camera", extends = PyComponent, eq)]
#[derive(Debug)]
pub struct PyRenderLayers {
    pub(crate) storage: ComponentStorage<RenderLayers>,
}

impl PartialEq for PyRenderLayers {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

#[pymethods]
impl PyRenderLayers {
    #[staticmethod]
    pub fn all(py: Python<'_>) -> PyResult<Py<Self>> {
        let mut layers = RenderLayers::none();
        for i in 0..32 {
            layers = layers.with(i);
        }
        Py::new(py, (PyRenderLayers::from(layers), PyComponent))
    }

    #[staticmethod]
    pub fn none(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (PyRenderLayers::from(RenderLayers::none()), PyComponent),
        )
    }

    #[staticmethod]
    pub fn layer(py: Python<'_>, layer: usize) -> PyResult<Py<Self>> {
        if layer >= usize::BITS as usize {
            return Err(PyValueError::new_err(format!(
                "layer must be less than {} for RenderLayers.layer(); use from_layers() for higher layers",
                usize::BITS
            )));
        }
        Py::new(
            py,
            (
                PyRenderLayers::from(RenderLayers::layer(layer)),
                PyComponent,
            ),
        )
    }

    #[pyo3(name = "with_")]
    pub fn with_(&self, py: Python<'_>, layer: usize) -> PyResult<Py<Self>> {
        let layers = self.as_ref()?.clone().with(layer);
        Py::new(py, (PyRenderLayers::from(layers), PyComponent))
    }

    pub fn without(&self, py: Python<'_>, layer: usize) -> PyResult<Py<Self>> {
        let layers = self.as_ref()?.clone().without(layer);
        Py::new(py, (PyRenderLayers::from(layers), PyComponent))
    }

    pub fn intersects(&self, other: &PyRenderLayers) -> PyResult<bool> {
        Ok(self.as_ref()?.intersects(other.as_ref()?.reborrow()))
    }

    #[getter]
    pub fn bits(&self) -> PyResult<Vec<u64>> {
        Ok(self.as_ref()?.bits().to_vec())
    }

    #[getter]
    pub fn active_layers(&self) -> PyResult<Vec<usize>> {
        Ok(self.as_ref()?.iter().collect())
    }

    #[pyo3(name = "iter")]
    pub fn iter_compat(&self) -> PyResult<Vec<usize>> {
        self.active_layers()
    }

    pub fn intersection(&self, py: Python<'_>, other: &PyRenderLayers) -> PyResult<Py<Self>> {
        let result = self.as_ref()?.intersection(other.as_ref()?.reborrow());
        Py::new(py, (PyRenderLayers::from(result), PyComponent))
    }

    pub fn union(&self, py: Python<'_>, other: &PyRenderLayers) -> PyResult<Py<Self>> {
        let result = self.as_ref()?.union(other.as_ref()?.reborrow());
        Py::new(py, (PyRenderLayers::from(result), PyComponent))
    }

    pub fn symmetric_difference(
        &self,
        py: Python<'_>,
        other: &PyRenderLayers,
    ) -> PyResult<Py<Self>> {
        let result = self
            .as_ref()?
            .symmetric_difference(other.as_ref()?.reborrow());
        Py::new(py, (PyRenderLayers::from(result), PyComponent))
    }

    #[staticmethod]
    pub fn from_layers(py: Python<'_>, layers: Vec<usize>) -> PyResult<Py<Self>> {
        let render_layers = RenderLayers::from_layers(&layers);
        Py::new(py, (PyRenderLayers::from(render_layers), PyComponent))
    }

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (RenderLayers::default().into(), PyComponent).into()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("RenderLayers({:?})", self.as_ref()?))
    }

    pub fn __and__(&self, py: Python<'_>, other: &PyRenderLayers) -> PyResult<Py<Self>> {
        self.intersection(py, other)
    }

    pub fn __or__(&self, py: Python<'_>, other: &PyRenderLayers) -> PyResult<Py<Self>> {
        self.union(py, other)
    }

    pub fn __xor__(&self, py: Python<'_>, other: &PyRenderLayers) -> PyResult<Py<Self>> {
        self.symmetric_difference(py, other)
    }
}
