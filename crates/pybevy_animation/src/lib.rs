pub mod animated_by;
pub mod animation_clip;
pub mod animation_curve;
pub mod animation_event;
pub mod animation_graph;
pub mod animation_graph_handle;
pub mod animation_node_index;
pub mod animation_player;
pub mod animation_target_id;
pub mod animation_transitions;
pub mod plugin;
pub mod repeat_animation;
pub use animated_by::PyAnimatedBy;
pub use animation_clip::{PyAnimationClip, PyVariableCurve};
pub use animation_curve::{
    PyAnimatableCurve, PyAnimatableKeyframeCurve, PyAnimatedField, PyAnimationCurve, PyWeightsCurve,
};
pub use animation_event::{PyAnimationEvent, PyAnimationEventData};
pub use animation_graph::{PyAnimationGraph, PyAnimationGraphNode, PyAnimationNodeType};
pub use animation_graph_handle::PyAnimationGraphHandle;
pub use animation_node_index::PyAnimationNodeIndex;
pub use animation_player::{PyActiveAnimation, PyAnimationPlayer, PyAnimationTarget};
pub use animation_target_id::PyAnimationTargetId;
pub use animation_transitions::PyAnimationTransitions;
use bevy::animation::{
    AnimatedBy, AnimationClip, AnimationPlayer, AnimationTargetId,
    graph::{AnimationGraph, AnimationGraphHandle},
    transition::AnimationTransitions,
};
pub use plugin::PyAnimationPlugin;
use pybevy_core::{plugin::plugin_registry, registry::global_registry};
use pybevy_macros::{asset_bridge, component_bridge, newtype_bridge, plugin_bridge};
pub use pybevy_mesh::PySkinnedMesh;
use pyo3::prelude::*;
pub use repeat_animation::PyRepeatAnimation;

component_bridge!(AnimatedBy, PyAnimatedBy);
component_bridge!(AnimationPlayer, PyAnimationPlayer);
component_bridge!(AnimationTransitions, PyAnimationTransitions);
newtype_bridge!(AnimationTargetId, PyAnimationTargetId, copy);

asset_bridge!(AnimationClip, PyAnimationClip);
asset_bridge!(AnimationGraph, PyAnimationGraph);

plugin_bridge!(PyAnimationPlugin, bevy::animation::AnimationPlugin);

pub struct AnimationGraphHandleBridge;

impl pybevy_core::ComponentBridge for AnimationGraphHandleBridge {
    fn bevy_type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<AnimationGraphHandle>()
    }

    fn py_type_ptr(&self) -> *const pyo3::ffi::PyTypeObject {
        pyo3::Python::attach(|py| {
            <PyAnimationGraphHandle as pyo3::PyTypeInfo>::type_object(py).as_type_ptr()
        })
    }

    fn py_type<'py>(&self, py: pyo3::Python<'py>) -> pyo3::Bound<'py, pyo3::types::PyType> {
        <PyAnimationGraphHandle as pyo3::PyTypeInfo>::type_object(py)
    }

    fn name(&self) -> &'static str {
        "AnimationGraphHandle"
    }

    fn register(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
        world.register_component::<AnimationGraphHandle>()
    }

    fn extract(
        &self,
        entity: &mut bevy::ecs::world::FilteredEntityMut,
        component_id: bevy::ecs::component::ComponentId,
        _validity: pybevy_core::ValidityFlagWithMode,
        py: pyo3::Python,
    ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
        let untyped = entity.get_by_id(component_id).ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("AnimationGraphHandle not found")
        })?;

        let component = unsafe { untyped.deref::<AnimationGraphHandle>() };
        let py_component = PyAnimationGraphHandle::from(component);
        let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
        Ok(obj.into_any())
    }

    fn extract_fn(&self) -> pybevy_core::ExtractFn {
        #[inline(always)]
        fn extract_impl(
            entity: &mut bevy::ecs::world::FilteredEntityMut,
            component_id: bevy::ecs::component::ComponentId,
            _validity: pybevy_core::ValidityFlagWithMode,
            py: pyo3::Python,
        ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
            let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("AnimationGraphHandle not found")
            })?;

            let component = unsafe { untyped.deref::<AnimationGraphHandle>() };
            let py_component = PyAnimationGraphHandle::from(component);
            let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
            Ok(obj.into_any())
        }
        extract_impl
    }

    fn insert(
        &self,
        world: &mut bevy::ecs::world::World,
        entity: bevy::ecs::entity::Entity,
        component: &pyo3::Bound<pyo3::PyAny>,
    ) -> pyo3::PyResult<()> {
        let py_component = component.extract::<pyo3::PyRef<PyAnimationGraphHandle>>()?;
        let native: AnimationGraphHandle = AnimationGraphHandle::try_from(py_component.clone())?;

        world.entity_mut(entity).insert(native);
        Ok(())
    }

    fn insert_into_entity(
        &self,
        entity: &mut bevy::ecs::world::EntityWorldMut,
        component: &pyo3::Bound<pyo3::PyAny>,
    ) -> pyo3::PyResult<()> {
        let py_component = component.extract::<pyo3::PyRef<PyAnimationGraphHandle>>()?;
        let native: AnimationGraphHandle = AnimationGraphHandle::try_from(py_component.clone())?;

        entity.insert(native);
        Ok(())
    }

    fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
        entity.contains::<AnimationGraphHandle>()
    }

    fn extract_from_entity_ref(
        &self,
        entity: &bevy::ecs::world::EntityRef,
        _validity: pybevy_core::ValidityFlagWithMode,
        py: pyo3::Python,
    ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
        if let Some(component) = entity.get::<AnimationGraphHandle>() {
            let py_component = PyAnimationGraphHandle::from(component);
            let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }

    fn extract_from_entity_mut(
        &self,
        entity: &mut bevy::ecs::world::EntityWorldMut,
        _validity: pybevy_core::ValidityFlagWithMode,
        py: pyo3::Python,
    ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
        if let Some(component) = entity.get::<AnimationGraphHandle>() {
            let py_component = PyAnimationGraphHandle::from(component);
            let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
            Ok(Some(obj.into_any()))
        } else {
            Ok(None)
        }
    }
}

pub fn register_animation_bridges() {
    global_registry::register_component_bridge(AnimatedByBridge);
    global_registry::register_component_bridge(AnimationPlayerBridge);
    global_registry::register_component_bridge(AnimationTransitionsBridge);
    global_registry::register_component_bridge(AnimationGraphHandleBridge);
    global_registry::register_component_bridge(AnimationTargetIdBridge);

    global_registry::register_asset_bridge(AnimationClipBridge);
    global_registry::register_asset_bridge(AnimationGraphBridge);

    plugin_registry::register_plugin_bridge(AnimationPluginBridge);
}

pub fn add_animation_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_animation_bridges();

    m.add_class::<PyAnimationPlugin>()?;
    m.add_class::<PyAnimatedBy>()?;
    m.add_class::<PyAnimationPlayer>()?;
    m.add_class::<PyAnimationTransitions>()?;
    m.add_class::<PyAnimationGraphHandle>()?;
    m.add_class::<PySkinnedMesh>()?;
    m.add_class::<PyAnimationTarget>()?;
    m.add_class::<PyActiveAnimation>()?;
    m.add_class::<PyAnimationClip>()?;
    m.add_class::<PyAnimationGraph>()?;
    m.add_class::<PyAnimationGraphNode>()?;
    m.add_class::<PyAnimationNodeType>()?;
    m.add_class::<PyAnimationEvent>()?;
    m.add_class::<PyAnimationEventData>()?;
    m.add_class::<PyAnimationCurve>()?;
    m.add_class::<PyVariableCurve>()?;
    m.add_class::<PyAnimatableCurve>()?;
    m.add_class::<PyAnimatableKeyframeCurve>()?;
    m.add_class::<PyWeightsCurve>()?;
    m.add_class::<PyAnimatedField>()?;
    m.add_class::<PyRepeatAnimation>()?;
    m.add_class::<PyAnimationNodeIndex>()?;
    m.add_class::<PyAnimationTargetId>()?;
    Ok(())
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "animation")?;
    add_animation_classes(&m)?;
    parent.add_submodule(&m)
}
