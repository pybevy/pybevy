use bevy::animation::{AnimationPlayer, RepeatAnimation, graph::AnimationNodeIndex};
use pybevy_core::{ComponentStorage, PyComponent, PyEntity};
use pybevy_macros::component_storage;
use pyo3::{PyRefMut, Python, exceptions::PyValueError, prelude::*};

use super::{
    animation_node_index::PyAnimationNodeIndex, animation_target_id::PyAnimationTargetId,
    repeat_animation::PyRepeatAnimation,
};

#[pyclass(name = "AnimationTarget", extends = PyComponent, eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyAnimationTarget {
    pub id: PyAnimationTargetId,

    #[pyo3(get, set)]
    pub player: PyEntity,
}

#[pymethods]
impl PyAnimationTarget {
    #[new]
    pub fn new(id: &PyAnimationTargetId, player: PyEntity) -> (Self, PyComponent) {
        (
            Self {
                id: id.clone(),
                player,
            },
            PyComponent,
        )
    }

    #[getter]
    pub fn id(&self, py: Python<'_>) -> PyResult<Py<PyAnimationTargetId>> {
        Py::new(py, PyAnimationTargetId::from_owned(self.id.0))
    }

    #[setter]
    pub fn set_id(&mut self, id: &PyAnimationTargetId) {
        self.id = id.clone();
    }

    pub fn __repr__(&self) -> String {
        format!(
            "AnimationTarget(id={}, player={})",
            self.id.__repr__(),
            self.player.__repr__()
        )
    }
}

/// Python wrapper for Bevy's ActiveAnimation.
///
/// This is a lightweight handle that borrows back into the AnimationPlayer's
/// ComponentStorage. Methods on this type mutate the ECS component directly.
#[pyclass(name = "ActiveAnimation")]
pub struct PyActiveAnimation {
    pub(crate) storage: ComponentStorage<AnimationPlayer>,
    pub(crate) node_index: AnimationNodeIndex,
}

impl PyActiveAnimation {
    fn with_animation<T, F>(&self, f: F) -> PyResult<T>
    where
        F: FnOnce(&bevy::animation::ActiveAnimation) -> PyResult<T>,
    {
        let player = self.storage.as_ref()?;
        let anim = player
            .animation(self.node_index)
            .ok_or_else(|| PyValueError::new_err("Animation not found for node index"))?;
        f(anim)
    }

    fn with_animation_mut<T, F>(&mut self, f: F) -> PyResult<T>
    where
        F: FnOnce(&mut bevy::animation::ActiveAnimation) -> PyResult<T>,
    {
        let player = self.storage.as_mut()?;
        let anim = player
            .animation_mut(self.node_index)
            .ok_or_else(|| PyValueError::new_err("Animation not found for node index"))?;
        f(anim)
    }
}

#[pymethods]
impl PyActiveAnimation {
    #[getter]
    pub fn is_finished(&self) -> PyResult<bool> {
        self.with_animation(|anim| Ok(anim.is_finished()))
    }

    pub fn replay(&mut self) -> PyResult<()> {
        self.with_animation_mut(|anim| {
            anim.replay();
            Ok(())
        })
    }

    #[getter]
    pub fn weight(&self) -> PyResult<f32> {
        self.with_animation(|anim| Ok(anim.weight()))
    }

    pub fn set_weight(mut pyself: PyRefMut<'_, Self>, weight: f32) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.set_weight(weight);
            Ok(())
        })?;
        Ok(pyself)
    }

    pub fn pause(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.pause();
            Ok(())
        })?;
        Ok(pyself)
    }

    pub fn resume(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.resume();
            Ok(())
        })?;
        Ok(pyself)
    }

    #[getter]
    pub fn is_paused(&self) -> PyResult<bool> {
        self.with_animation(|anim| Ok(anim.is_paused()))
    }

    pub fn set_repeat<'py>(
        mut pyself: PyRefMut<'py, Self>,
        repeat: &PyRepeatAnimation,
    ) -> PyResult<PyRefMut<'py, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.set_repeat(RepeatAnimation::from(repeat));
            Ok(())
        })?;
        Ok(pyself)
    }

    pub fn repeat(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.repeat();
            Ok(())
        })?;
        Ok(pyself)
    }

    pub fn repeat_mode(&self) -> PyResult<PyRepeatAnimation> {
        self.with_animation(|anim| Ok(anim.repeat_mode().into()))
    }

    #[getter]
    pub fn completions(&self) -> PyResult<u32> {
        self.with_animation(|anim| Ok(anim.completions()))
    }

    #[getter]
    pub fn is_playback_reversed(&self) -> PyResult<bool> {
        self.with_animation(|anim| Ok(anim.is_playback_reversed()))
    }

    #[getter]
    pub fn speed(&self) -> PyResult<f32> {
        self.with_animation(|anim| Ok(anim.speed()))
    }

    pub fn set_speed(mut pyself: PyRefMut<'_, Self>, speed: f32) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.set_speed(speed);
            Ok(())
        })?;
        Ok(pyself)
    }

    #[getter]
    pub fn elapsed(&self) -> PyResult<f32> {
        self.with_animation(|anim| Ok(anim.elapsed()))
    }

    #[getter]
    pub fn seek_time(&self) -> PyResult<f32> {
        self.with_animation(|anim| Ok(anim.seek_time()))
    }

    pub fn set_seek_time(
        mut pyself: PyRefMut<'_, Self>,
        seek_time: f32,
    ) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.set_seek_time(seek_time);
            Ok(())
        })?;
        Ok(pyself)
    }

    pub fn seek_to(mut pyself: PyRefMut<'_, Self>, seek_time: f32) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.seek_to(seek_time);
            Ok(())
        })?;
        Ok(pyself)
    }

    pub fn rewind(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.with_animation_mut(|anim| {
            anim.rewind();
            Ok(())
        })?;
        Ok(pyself)
    }
}

/// Python wrapper for Bevy's AnimationPlayer component.
///
/// Uses ComponentStorage for proper ECS mutation persistence.
#[component_storage(AnimationPlayer)]
#[pyclass(name = "AnimationPlayer", extends = PyComponent)]
#[derive(Clone)]
pub struct PyAnimationPlayer {
    pub(crate) storage: ComponentStorage<AnimationPlayer>,
}

#[pymethods]
impl PyAnimationPlayer {
    #[new]
    pub fn new() -> (Self, PyComponent) {
        Self::from_owned(AnimationPlayer::default())
    }

    pub fn play(
        &mut self,
        py: Python<'_>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<Py<PyActiveAnimation>> {
        self.as_mut()?.play(animation.0);
        Py::new(
            py,
            PyActiveAnimation {
                storage: self.storage.clone(),
                node_index: animation.0,
            },
        )
    }

    pub fn start(
        &mut self,
        py: Python<'_>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<Py<PyActiveAnimation>> {
        self.as_mut()?.start(animation.0);
        Py::new(
            py,
            PyActiveAnimation {
                storage: self.storage.clone(),
                node_index: animation.0,
            },
        )
    }

    pub fn stop<'py>(
        mut pyself: PyRefMut<'py, Self>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<PyRefMut<'py, Self>> {
        pyself.as_mut()?.stop(animation.0);
        Ok(pyself)
    }

    pub fn stop_all(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.as_mut()?.stop_all();
        Ok(pyself)
    }

    pub fn pause_all(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.as_mut()?.pause_all();
        Ok(pyself)
    }

    pub fn resume_all(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.as_mut()?.resume_all();
        Ok(pyself)
    }

    pub fn set_speed(mut pyself: PyRefMut<'_, Self>, speed: f32) -> PyResult<PyRefMut<'_, Self>> {
        pyself.as_mut()?.adjust_speeds(speed);
        Ok(pyself)
    }

    pub fn seek_all_by(
        mut pyself: PyRefMut<'_, Self>,
        amount: f32,
    ) -> PyResult<PyRefMut<'_, Self>> {
        pyself.as_mut()?.seek_all_by(amount);
        Ok(pyself)
    }

    pub fn is_playing_animation(&self, animation: &PyAnimationNodeIndex) -> PyResult<bool> {
        Ok(self.as_ref()?.is_playing_animation(animation.0))
    }

    #[getter]
    pub fn all_finished(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.all_finished())
    }

    #[getter]
    pub fn all_paused(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.all_paused())
    }

    pub fn animation(
        &self,
        py: Python<'_>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<Option<Py<PyActiveAnimation>>> {
        if !self.as_ref()?.is_playing_animation(animation.0) {
            return Ok(None);
        }
        let py_anim = Py::new(
            py,
            PyActiveAnimation {
                storage: self.storage.clone(),
                node_index: animation.0,
            },
        )?;
        Ok(Some(py_anim))
    }

    pub fn animation_mut(
        &self,
        py: Python<'_>,
        animation: &PyAnimationNodeIndex,
    ) -> PyResult<Option<Py<PyActiveAnimation>>> {
        self.animation(py, animation)
    }

    pub fn rewind_all(mut pyself: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        pyself.as_mut()?.rewind_all();
        Ok(pyself)
    }

    pub fn adjust_speeds(
        mut pyself: PyRefMut<'_, Self>,
        factor: f32,
    ) -> PyResult<PyRefMut<'_, Self>> {
        pyself.as_mut()?.adjust_speeds(factor);
        Ok(pyself)
    }
}
