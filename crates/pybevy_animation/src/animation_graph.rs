use bevy::animation::{
    graph::{AnimationGraph, AnimationGraphNode, AnimationNodeType},
    prelude::AnimationNodeIndex,
};
use pybevy_core::{
    AssetStorage, PyAsset, PyHandle, extract_handle_from_any,
    public_error::{ANIMATION_GRAPH_NODE_MISSING, ANIMATION_GRAPH_NODE_READ_ONLY},
};
use pybevy_macros::pyasset;
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyList, PyTuple},
};

use crate::animation_node_index::PyAnimationNodeIndex;

#[pyasset(AnimationGraph, bridge)]
#[pyclass(name = "AnimationGraph", extends = PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyAnimationGraph {
    pub(crate) storage: AssetStorage<AnimationGraph>,
}

#[pymethods]
impl PyAnimationGraph {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (Self::from(AnimationGraph::default()), PyAsset).into()
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
        let weight = validate_weight(weight)?;
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
        let handle = (&handle).try_into()?;
        let weight = validate_weight(weight)?;
        Ok(PyAnimationNodeIndex(
            self.as_mut()?.add_clip(handle, weight, parent.0),
        ))
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

    pub fn get(
        slf: &Bound<'_, Self>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<Option<PyAnimationGraphNode>> {
        let present = PyAnimationGraph::as_ref(&slf.borrow())?
            .get(animation.0)
            .is_some();
        Ok(present.then(|| PyAnimationGraphNode::view(slf.clone().unbind(), animation.0, false)))
    }

    pub fn get_mut(
        slf: &Bound<'_, Self>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<Option<PyAnimationGraphNode>> {
        let present = PyAnimationGraph::as_ref(&slf.borrow())?
            .get(animation.0)
            .is_some();
        Ok(present.then(|| PyAnimationGraphNode::view(slf.clone().unbind(), animation.0, true)))
    }

    pub fn nodes(&self) -> PyResult<Vec<PyAnimationNodeIndex>> {
        Ok(self.as_ref()?.nodes().map(PyAnimationNodeIndex).collect())
    }
}

fn validate_weight(weight: f32) -> PyResult<f32> {
    if weight.is_finite() {
        Ok(weight)
    } else {
        Err(PyValueError::new_err(format!(
            "weight must be finite (got {weight})"
        )))
    }
}

#[pyclass(name = "AnimationNodeType", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyAnimationNodeType {
    Clip { handle: PyHandle },
    Blend(),
    Add(),
}

impl From<&AnimationNodeType> for PyAnimationNodeType {
    fn from(node_type: &AnimationNodeType) -> Self {
        match node_type {
            AnimationNodeType::Clip(handle) => PyAnimationNodeType::Clip {
                handle: PyHandle::from(handle),
            },
            AnimationNodeType::Blend => PyAnimationNodeType::Blend(),
            AnimationNodeType::Add => PyAnimationNodeType::Add(),
        }
    }
}

#[pymethods]
impl PyAnimationNodeType {
    pub fn __repr__(&self) -> String {
        match self {
            PyAnimationNodeType::Clip { handle } => {
                format!("AnimationNodeType.Clip({:?})", handle)
            }
            PyAnimationNodeType::Blend() => "AnimationNodeType.Blend".to_string(),
            PyAnimationNodeType::Add() => "AnimationNodeType.Add".to_string(),
        }
    }
}

#[pyclass(name = "AnimationGraphNode", skip_from_py_object)]
pub struct PyAnimationGraphNode {
    graph: Py<PyAnimationGraph>,
    index: AnimationNodeIndex,
    mutable: bool,
}

impl PyAnimationGraphNode {
    fn view(graph: Py<PyAnimationGraph>, index: AnimationNodeIndex, mutable: bool) -> Self {
        Self {
            graph,
            index,
            mutable,
        }
    }

    fn with_node<R>(
        &self,
        py: Python<'_>,
        read: impl FnOnce(&AnimationGraphNode) -> R,
    ) -> PyResult<R> {
        let graph = self.graph.borrow(py);
        let graph = PyAnimationGraph::as_ref(&graph)?;
        let node = graph
            .get(self.index)
            .ok_or_else(|| PyValueError::new_err(ANIMATION_GRAPH_NODE_MISSING))?;
        Ok(read(node))
    }

    fn with_node_mut<R>(
        &self,
        py: Python<'_>,
        write: impl FnOnce(&mut AnimationGraphNode) -> R,
    ) -> PyResult<R> {
        if !self.mutable {
            return Err(PyRuntimeError::new_err(ANIMATION_GRAPH_NODE_READ_ONLY));
        }
        let mut graph = self.graph.borrow_mut(py);
        PyAnimationGraph::as_ref(&graph)?
            .get(self.index)
            .ok_or_else(|| PyValueError::new_err(ANIMATION_GRAPH_NODE_MISSING))?;
        let mut graph = PyAnimationGraph::as_mut(&mut graph)?;
        let node = graph
            .get_mut(self.index)
            .expect("node existence was preflighted without Python reentry");
        Ok(write(node))
    }
}

#[pymethods]
impl PyAnimationGraphNode {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.graph)
    }

    #[getter]
    pub fn node_type(&self, py: Python<'_>) -> PyResult<PyAnimationNodeType> {
        self.with_node(py, |node| PyAnimationNodeType::from(&node.node_type))
    }

    #[getter]
    pub fn mask(&self, py: Python<'_>) -> PyResult<u64> {
        self.with_node(py, |node| node.mask)
    }

    #[setter]
    pub fn set_mask(&mut self, py: Python<'_>, mask: u64) -> PyResult<()> {
        self.with_node_mut(py, |node| node.mask = mask)
    }

    #[getter]
    pub fn weight(&self, py: Python<'_>) -> PyResult<f32> {
        self.with_node(py, |node| node.weight)
    }

    #[setter]
    pub fn set_weight(&mut self, py: Python<'_>, weight: f32) -> PyResult<()> {
        self.with_node_mut(py, |node| node.weight = weight)
    }

    pub fn add_mask<'py>(slf: PyRef<'py, Self>, mask: u64) -> PyResult<PyRef<'py, Self>> {
        slf.with_node_mut(slf.py(), |node| {
            node.add_mask(mask);
        })?;
        Ok(slf)
    }

    pub fn remove_mask<'py>(slf: PyRef<'py, Self>, mask: u64) -> PyResult<PyRef<'py, Self>> {
        slf.with_node_mut(slf.py(), |node| {
            node.remove_mask(mask);
        })?;
        Ok(slf)
    }

    pub fn add_mask_group<'py>(slf: PyRef<'py, Self>, group: u32) -> PyResult<PyRef<'py, Self>> {
        slf.with_node_mut(slf.py(), |node| {
            node.add_mask_group(group);
        })?;
        Ok(slf)
    }

    pub fn remove_mask_group<'py>(slf: PyRef<'py, Self>, group: u32) -> PyResult<PyRef<'py, Self>> {
        slf.with_node_mut(slf.py(), |node| {
            node.remove_mask_group(group);
        })?;
        Ok(slf)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        self.with_node(py, |node| {
            format!(
                "AnimationGraphNode(node_type={:?}, mask={}, weight={})",
                node.node_type, node.mask, node.weight
            )
        })
    }
}
