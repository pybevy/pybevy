use std::collections::{HashMap, HashSet};

use bevy::animation::{
    graph::{AnimationGraph, AnimationGraphNode, AnimationNodeType},
    prelude::AnimationNodeIndex,
};
use pybevy_core::{
    AssetStorage, PyAsset, PyHandle,
    domain::{FINITE, IntegerInput, MASK_BIT_DOMAIN, check_for},
    extract_handle_from_any,
    public_error::{
        ANIMATION_GRAPH_CYCLE, ANIMATION_GRAPH_IN_USE, ANIMATION_GRAPH_NODE_MISSING,
        ANIMATION_GRAPH_NODE_READ_ONLY,
    },
    repr::{float_repr, object_repr},
};
use pybevy_macros::{pyasset, pyenum};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyList, PyTuple},
};

use crate::animation_node_index::PyAnimationNodeIndex;

const CLIP_CANNOT_BE_NONE: &str = "clip cannot be None";
const MAX_REACHABLE_DEPTH: usize = 512;

// Reentrant Python or another thread may hold the graph; raise instead of panicking.
fn graph_ref<'py>(graph: &Bound<'py, PyAnimationGraph>) -> PyResult<PyRef<'py, PyAnimationGraph>> {
    graph
        .try_borrow()
        .map_err(|_| PyRuntimeError::new_err(ANIMATION_GRAPH_IN_USE))
}

fn graph_mut<'py>(
    graph: &Bound<'py, PyAnimationGraph>,
) -> PyResult<PyRefMut<'py, PyAnimationGraph>> {
    graph
        .try_borrow_mut()
        .map_err(|_| PyRuntimeError::new_err(ANIMATION_GRAPH_IN_USE))
}

fn validate_node(graph: &AnimationGraph, index: AnimationNodeIndex) -> PyResult<()> {
    if graph.get(index).is_none() {
        return Err(PyValueError::new_err(ANIMATION_GRAPH_NODE_MISSING));
    }
    Ok(())
}

fn edge_would_cycle(
    graph: &AnimationGraph,
    from: AnimationNodeIndex,
    to: AnimationNodeIndex,
) -> bool {
    let mut pending = vec![to];
    let mut visited = HashSet::new();
    while let Some(node) = pending.pop() {
        if node == from {
            return true;
        }
        if visited.insert(node) {
            pending.extend(graph.graph.neighbors(node));
        }
    }
    false
}

fn validate_reachable_depth(
    graph: &AnimationGraph,
    root: AnimationNodeIndex,
    added_edge: Option<(AnimationNodeIndex, AnimationNodeIndex)>,
    appended_to: Option<AnimationNodeIndex>,
) -> PyResult<()> {
    let mut longest_seen = HashMap::new();
    let mut pending = vec![(root, 1_usize)];
    while let Some((node, depth)) = pending.pop() {
        if depth > MAX_REACHABLE_DEPTH {
            return Err(PyValueError::new_err(format!(
                "AnimationGraph path exceeds {MAX_REACHABLE_DEPTH} nodes"
            )));
        }
        if longest_seen.get(&node).is_some_and(|seen| *seen >= depth) {
            continue;
        }
        longest_seen.insert(node, depth);
        if appended_to == Some(node) && depth == MAX_REACHABLE_DEPTH {
            return Err(PyValueError::new_err(format!(
                "AnimationGraph path exceeds {MAX_REACHABLE_DEPTH} nodes"
            )));
        }
        pending.extend(graph.graph.neighbors(node).map(|child| (child, depth + 1)));
        if let Some((from, to)) = added_edge
            && from == node
        {
            pending.push((to, depth + 1));
        }
    }
    Ok(())
}

#[pyasset(AnimationGraph, bridge)]
#[pyclass(name = "AnimationGraph", module = "pybevy.animation", extends = PyAsset, skip_from_py_object)]
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
            return Err(PyTypeError::new_err(CLIP_CANNOT_BE_NONE));
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

    #[setter]
    pub fn set_root(&mut self, value: &PyAnimationNodeIndex) -> PyResult<()> {
        {
            let graph = self.as_ref()?;
            validate_node(&graph, value.0)?;
            validate_reachable_depth(&graph, value.0, None, None)?;
        }
        self.as_mut()?.root = value.0;
        Ok(())
    }

    pub fn add_blend(
        &mut self,
        weight: f32,
        parent: &PyAnimationNodeIndex,
    ) -> PyResult<PyAnimationNodeIndex> {
        let weight = check_for::<Self, _>("weight", weight, FINITE)?;
        {
            let graph = self.as_ref()?;
            validate_node(&graph, parent.0)?;
            validate_reachable_depth(&graph, graph.root, None, Some(parent.0))?;
        }
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
            return Err(PyTypeError::new_err(CLIP_CANNOT_BE_NONE));
        }
        let handle = extract_handle_from_any(clip)?;
        let handle = (&handle).try_into()?;
        let weight = check_for::<Self, _>("weight", weight, FINITE)?;
        {
            let graph = self.as_ref()?;
            validate_node(&graph, parent.0)?;
            validate_reachable_depth(&graph, graph.root, None, Some(parent.0))?;
        }
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
        {
            let graph = self.as_ref()?;
            validate_node(&graph, from_.0)?;
            validate_node(&graph, to.0)?;
            if edge_would_cycle(&graph, from_.0, to.0) {
                return Err(PyValueError::new_err(ANIMATION_GRAPH_CYCLE));
            }
            validate_reachable_depth(&graph, graph.root, Some((from_.0, to.0)), None)?;
        }
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
        let graph = graph_ref(slf)?;
        let present = PyAnimationGraph::as_ref(&graph)?.get(animation.0).is_some();
        Ok(present.then(|| PyAnimationGraphNode::view(slf.clone().unbind(), animation.0, false)))
    }

    pub fn get_mut(
        slf: &Bound<'_, Self>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<Option<PyAnimationGraphNode>> {
        let graph = graph_ref(slf)?;
        let present = PyAnimationGraph::as_ref(&graph)?.get(animation.0).is_some();
        Ok(present.then(|| PyAnimationGraphNode::view(slf.clone().unbind(), animation.0, true)))
    }

    pub fn nodes(&self) -> PyResult<Vec<PyAnimationNodeIndex>> {
        Ok(self.as_ref()?.nodes().map(PyAnimationNodeIndex).collect())
    }
}

#[pyenum(AnimationNodeType, manual)]
#[pyclass(
    name = "AnimationNodeType",
    module = "pybevy.animation",
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyAnimationNodeType {
    Clip { value: PyHandle },
    Blend(),
    Add(),
}

impl From<&AnimationNodeType> for PyAnimationNodeType {
    fn from(node_type: &AnimationNodeType) -> Self {
        match node_type {
            AnimationNodeType::Clip(handle) => PyAnimationNodeType::Clip {
                value: PyHandle::from(handle),
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
            PyAnimationNodeType::Clip { value } => {
                format!("AnimationNodeType.Clip({})", object_repr(value.clone()))
            }
            PyAnimationNodeType::Blend() => "AnimationNodeType.Blend".to_string(),
            PyAnimationNodeType::Add() => "AnimationNodeType.Add".to_string(),
        }
    }
}

#[pyclass(
    name = "AnimationGraphNode",
    module = "pybevy.animation",
    skip_from_py_object
)]
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
        let graph = graph_ref(self.graph.bind(py))?;
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
        let mut graph = graph_mut(self.graph.bind(py))?;
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

    #[setter]
    pub fn set_node_type(
        &mut self,
        py: Python<'_>,
        value: PyRef<'_, PyAnimationNodeType>,
    ) -> PyResult<()> {
        let value = match &*value {
            PyAnimationNodeType::Clip { value } => AnimationNodeType::Clip(value.try_into()?),
            PyAnimationNodeType::Blend() => AnimationNodeType::Blend,
            PyAnimationNodeType::Add() => AnimationNodeType::Add,
        };
        self.with_node_mut(py, |node| node.node_type = value)
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
        let weight = check_for::<Self, _>("weight", weight, FINITE)?;
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

    pub fn add_mask_group<'py>(
        slf: PyRef<'py, Self>,
        group: IntegerInput<u32>,
    ) -> PyResult<PyRef<'py, Self>> {
        let group = check_for::<Self, _>("group", group, MASK_BIT_DOMAIN)?.stored();
        slf.with_node_mut(slf.py(), |node| {
            node.add_mask_group(group);
        })?;
        Ok(slf)
    }

    pub fn remove_mask_group<'py>(
        slf: PyRef<'py, Self>,
        group: IntegerInput<u32>,
    ) -> PyResult<PyRef<'py, Self>> {
        let group = check_for::<Self, _>("group", group, MASK_BIT_DOMAIN)?.stored();
        slf.with_node_mut(slf.py(), |node| {
            node.remove_mask_group(group);
        })?;
        Ok(slf)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        self.with_node(py, |node| {
            format!(
                "AnimationGraphNode(node_type={}, mask={}, weight={})",
                PyAnimationNodeType::from(&node.node_type).__repr__(),
                node.mask,
                float_repr(node.weight)
            )
        })
    }
}
