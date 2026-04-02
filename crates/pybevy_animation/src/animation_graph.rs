use bevy::animation::graph::{AnimationGraph, AnimationGraphNode, AnimationNodeType};
use pybevy_core::{AssetStorage, PyAsset, PyHandle, extract_handle_from_any};
use pybevy_macros::asset_storage;
use pyo3::{
    PyRefMut,
    prelude::*,
    types::{PyList, PyTuple},
};

use crate::animation_node_index::PyAnimationNodeIndex;

#[asset_storage(AnimationGraph, bridge)]
#[pyclass(name = "AnimationGraph", extends = PyAsset)]
#[derive(Debug)]
pub struct PyAnimationGraph {
    pub(crate) storage: AssetStorage<AnimationGraph>,
}

#[pymethods]
impl PyAnimationGraph {
    #[new]
    pub fn new() -> (Self, PyAsset) {
        (Self::from(AnimationGraph::default()), PyAsset)
    }

    #[staticmethod]
    pub fn from_clip<'py>(
        clip: &Bound<'_, PyAny>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        if clip.is_none() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "clip cannot be None",
            ));
        }
        let handle = extract_handle_from_any(clip)?;
        let (graph, animation_node_index) = AnimationGraph::from_clip((&handle).try_into()?);

        PyTuple::new(
            py,
            [
                Py::new(py, (Self::from(graph), PyAsset))?.into_any(),
                Py::new(py, PyAnimationNodeIndex(animation_node_index))?.into_any(),
            ],
        )
    }

    #[staticmethod]
    pub fn from_clips<'py>(
        clips: Bound<'_, PyAny>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let clips = clips
            .try_iter()?
            .map(|clip| {
                clip.and_then(|c| {
                    let handle = extract_handle_from_any(&c)?;
                    (&handle).try_into()
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let (graph, indices) = AnimationGraph::from_clips(clips);
        let indices: Vec<_> = indices.into_iter().map(PyAnimationNodeIndex).collect();

        PyTuple::new(
            py,
            [
                Py::new(py, (Self::from(graph), PyAsset))?.into_any(),
                PyList::new(py, indices)?.into_any().unbind(),
            ],
        )
    }

    #[getter]
    pub fn root(&self) -> PyResult<PyAnimationNodeIndex> {
        Ok(PyAnimationNodeIndex(self.as_ref()?.root))
    }

    pub fn add_blend(
        &mut self,
        weight: f32,
        parent: &PyAnimationNodeIndex,
    ) -> PyResult<PyAnimationNodeIndex> {
        Ok(PyAnimationNodeIndex(
            self.as_mut()?.add_blend(weight, parent.0),
        ))
    }

    pub fn add_clip(
        &mut self,
        clip: &Bound<'_, PyAny>,
        weight: f32,
        parent: &PyAnimationNodeIndex,
    ) -> PyResult<PyAnimationNodeIndex> {
        if clip.is_none() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "clip cannot be None",
            ));
        }
        let handle = extract_handle_from_any(clip)?;
        Ok(PyAnimationNodeIndex(self.as_mut()?.add_clip(
            (&handle).try_into()?,
            weight,
            parent.0,
        )))
    }

    #[pyo3(signature = (from_, to))]
    pub fn add_edge(
        &mut self,
        from_: &PyAnimationNodeIndex,
        to: &PyAnimationNodeIndex,
    ) -> PyResult<()> {
        self.as_mut()?.add_edge(from_.0, to.0);
        Ok(())
    }

    #[pyo3(signature = (from_, to))]
    pub fn remove_edge(
        &mut self,
        from_: &PyAnimationNodeIndex,
        to: &PyAnimationNodeIndex,
    ) -> PyResult<bool> {
        Ok(self.as_mut()?.remove_edge(from_.0, to.0))
    }

    pub fn get(&self, animation: &PyAnimationNodeIndex) -> PyResult<Option<PyAnimationGraphNode>> {
        Ok(self
            .as_ref()?
            .get(animation.0)
            .map(|index| PyAnimationGraphNode(index.clone())))
    }

    pub fn nodes(&self) -> PyResult<Vec<PyAnimationNodeIndex>> {
        Ok(self.as_ref()?.nodes().map(PyAnimationNodeIndex).collect())
    }
}

#[pyclass(name = "AnimationNodeType", eq)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyAnimationNodeType {
    Clip(PyHandle),
    Blend(),
    Add(),
}

impl From<&AnimationNodeType> for PyAnimationNodeType {
    fn from(node_type: &AnimationNodeType) -> Self {
        match node_type {
            AnimationNodeType::Clip(handle) => PyAnimationNodeType::Clip(PyHandle::from(handle)),
            AnimationNodeType::Blend => PyAnimationNodeType::Blend(),
            AnimationNodeType::Add => PyAnimationNodeType::Add(),
        }
    }
}

#[pymethods]
impl PyAnimationNodeType {
    pub fn __repr__(&self) -> String {
        match self {
            PyAnimationNodeType::Clip(handle) => {
                format!("AnimationNodeType.Clip({:?})", handle)
            }
            PyAnimationNodeType::Blend() => "AnimationNodeType.Blend".to_string(),
            PyAnimationNodeType::Add() => "AnimationNodeType.Add".to_string(),
        }
    }
}

#[pyclass(name = "AnimationGraphNode")]
#[derive(Debug, Clone)]
pub struct PyAnimationGraphNode(pub AnimationGraphNode);

#[pymethods]
impl PyAnimationGraphNode {
    #[getter]
    pub fn node_type(&self) -> PyAnimationNodeType {
        PyAnimationNodeType::from(&self.0.node_type)
    }

    #[getter]
    pub fn mask(&self) -> u64 {
        self.0.mask
    }

    #[setter]
    pub fn set_mask(&mut self, mask: u64) {
        self.0.mask = mask;
    }

    #[getter]
    pub fn weight(&self) -> f32 {
        self.0.weight
    }

    #[setter]
    pub fn set_weight(&mut self, weight: f32) {
        self.0.weight = weight;
    }

    pub fn add_mask(mut slf: PyRefMut<'_, Self>, mask: u64) -> PyRefMut<'_, Self> {
        slf.0.add_mask(mask);
        slf
    }

    pub fn remove_mask(mut slf: PyRefMut<'_, Self>, mask: u64) -> PyRefMut<'_, Self> {
        slf.0.remove_mask(mask);
        slf
    }

    pub fn add_mask_group(mut slf: PyRefMut<'_, Self>, group: u32) -> PyRefMut<'_, Self> {
        slf.0.add_mask_group(group);
        slf
    }

    pub fn remove_mask_group(mut slf: PyRefMut<'_, Self>, group: u32) -> PyRefMut<'_, Self> {
        slf.0.remove_mask_group(group);
        slf
    }
}
