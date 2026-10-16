use bevy::camera::visibility::RenderLayers;
use pybevy_core::{
    PyComponent,
    domain::{FieldDomain, IntegerInput, check},
    pycomponent::ComponentStorage,
};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

/// `RenderLayers::layer` builds a single-word mask; `from_layers` takes more.
const LAYER_DOMAIN: &[FieldDomain] = &[FieldDomain::Range { lo: 0.0, hi: 64.0 }];

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
    pub fn none(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            (PyRenderLayers::from(RenderLayers::none()), PyComponent),
        )
    }

    #[staticmethod]
    pub fn layer(py: Python<'_>, layer: IntegerInput<u64>) -> PyResult<Py<Self>> {
        let layer = check("RenderLayers", "layer", layer, LAYER_DOMAIN)?.stored() as usize;
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

    pub fn iter(&self) -> PyResult<Vec<usize>> {
        Ok(self.as_ref()?.iter().collect())
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
        let layers: Vec<String> = self
            .as_ref()?
            .iter()
            .map(|layer| layer.to_string())
            .collect();
        Ok(format!("RenderLayers.from_layers([{}])", layers.join(", ")))
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
