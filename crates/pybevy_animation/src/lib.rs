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

use pyo3::prelude::*;

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "animation")?;
    m.add_class::<plugin::PyAnimationPlugin>()?;
    m.add_class::<animated_by::PyAnimatedBy>()?;
    m.add_class::<animation_player::PyAnimationPlayer>()?;
    m.add_class::<animation_transitions::PyAnimationTransitions>()?;
    m.add_class::<animation_graph_handle::PyAnimationGraphHandle>()?;
    m.add_class::<animation_player::PyActiveAnimation>()?;
    m.add_class::<animation_clip::PyAnimationClip>()?;
    m.add_class::<animation_graph::PyAnimationGraph>()?;
    m.add_class::<animation_graph::PyAnimationGraphNode>()?;
    m.add_class::<animation_graph::PyAnimationNodeType>()?;
    m.add_class::<animation_event::PyAnimationEvent>()?;
    m.add_class::<animation_event::PyAnimationEventData>()?;
    m.add_class::<animation_curve::PyAnimationCurve>()?;
    m.add_class::<animation_clip::PyVariableCurve>()?;
    m.add_class::<animation_curve::PyAnimatableCurve>()?;
    m.add_class::<animation_curve::PyAnimatableKeyframeCurve>()?;
    m.add_class::<animation_curve::PyWeightsCurve>()?;
    m.add_class::<animation_curve::PyAnimatedField>()?;
    m.add_class::<repeat_animation::PyRepeatAnimation>()?;
    m.add_class::<animation_node_index::PyAnimationNodeIndex>()?;
    m.add_class::<animation_target_id::PyAnimationTargetId>()?;
    parent.add_submodule(&m)
}
