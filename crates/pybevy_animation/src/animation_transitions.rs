use std::time::Duration;

use bevy::animation::transition::AnimationTransitions;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pyo3::prelude::*;

use super::{
    animation_node_index::PyAnimationNodeIndex,
    animation_player::{PyActiveAnimation, PyAnimationPlayer},
};

#[component_storage(AnimationTransitions, bridge)]
#[pyclass(name = "AnimationTransitions", extends = PyComponent)]
#[derive(Clone)]
pub struct PyAnimationTransitions {
    pub(crate) storage: ComponentStorage<AnimationTransitions>,
}

#[pymethods]
impl PyAnimationTransitions {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(AnimationTransitions::new())
    }

    pub fn play(
        &mut self,
        player: &mut PyAnimationPlayer,
        new_animation: &PyAnimationNodeIndex,
        transition_duration: f64,
    ) -> PyResult<PyActiveAnimation> {
        let duration = Duration::from_secs_f64(transition_duration);

        let bevy_player = player.as_mut()?;

        self.as_mut()?.play(bevy_player, new_animation.0, duration);

        Ok(PyActiveAnimation {
            storage: player.storage.clone(),
            node_index: new_animation.0,
        })
    }

    pub fn get_main_animation(&self) -> PyResult<Option<PyAnimationNodeIndex>> {
        Ok(self
            .as_ref()?
            .get_main_animation()
            .map(PyAnimationNodeIndex))
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref().ok().and_then(|t| t.get_main_animation()) {
            Some(index) => format!("AnimationTransitions(main_animation={})", index.index()),
            None => "AnimationTransitions(main_animation=None)".to_string(),
        }
    }
}
