use bevy::{
    input::touch::{Touch, Touches},
    math::Vec2,
};
use pybevy_core::{PyResource, ResourceStorage, repr::ReprLiteral, resource_initializer};
use pybevy_macros::pyresource;
use pybevy_math::{snapshot::Snapshot, vec2::PyVec2};
use pyo3::prelude::*;

use crate::force_touch::PyForceTouch;

#[pyresource(Touches, no_clone, bridge, default_insert, no_reflect, no_user_remove)]
#[pyclass(name = "Touches", module = "pybevy.input", extends = PyResource)]
pub struct PyTouches {
    pub(crate) storage: ResourceStorage<Touches>,
}

#[pymethods]
impl PyTouches {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(Touches::default()),
        })
    }

    pub fn any_just_pressed(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.any_just_pressed())
    }

    pub fn any_just_released(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.any_just_released())
    }

    pub fn any_just_canceled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.any_just_canceled())
    }

    pub fn just_pressed(&self, id: u64) -> PyResult<bool> {
        Ok(self.as_ref()?.just_pressed(id))
    }

    pub fn just_released(&self, id: u64) -> PyResult<bool> {
        Ok(self.as_ref()?.just_released(id))
    }

    pub fn just_canceled(&self, id: u64) -> PyResult<bool> {
        Ok(self.as_ref()?.just_canceled(id))
    }

    pub fn get_pressed(&self, id: u64) -> PyResult<Option<PyTouch>> {
        Ok(self.as_ref()?.get_pressed(id).map(PyTouch::from_bevy))
    }

    pub fn get_released(&self, id: u64) -> PyResult<Option<PyTouch>> {
        Ok(self.as_ref()?.get_released(id).map(PyTouch::from_bevy))
    }

    pub fn iter(&self) -> PyResult<Vec<PyTouch>> {
        Ok(self.as_ref()?.iter().map(PyTouch::from_bevy).collect())
    }

    pub fn iter_just_pressed(&self) -> PyResult<Vec<PyTouch>> {
        Ok(self
            .as_ref()?
            .iter_just_pressed()
            .map(PyTouch::from_bevy)
            .collect())
    }

    pub fn iter_just_released(&self) -> PyResult<Vec<PyTouch>> {
        Ok(self
            .as_ref()?
            .iter_just_released()
            .map(PyTouch::from_bevy)
            .collect())
    }

    pub fn iter_just_canceled(&self) -> PyResult<Vec<PyTouch>> {
        Ok(self
            .as_ref()?
            .iter_just_canceled()
            .map(PyTouch::from_bevy)
            .collect())
    }

    pub fn first_pressed_position(&self) -> PyResult<Option<PyVec2>> {
        Ok(self.as_ref()?.first_pressed_position().map(Into::into))
    }

    pub fn clear(&mut self) -> PyResult<()> {
        self.as_mut()?.clear();
        Ok(())
    }

    pub fn clear_just_pressed(&mut self, id: u64) -> PyResult<bool> {
        Ok(self.as_mut()?.clear_just_pressed(id))
    }

    pub fn clear_just_released(&mut self, id: u64) -> PyResult<bool> {
        Ok(self.as_mut()?.clear_just_released(id))
    }

    pub fn clear_just_canceled(&mut self, id: u64) -> PyResult<bool> {
        Ok(self.as_mut()?.clear_just_canceled(id))
    }

    pub fn release(&mut self, id: u64) -> PyResult<()> {
        self.as_mut()?.release(id);
        Ok(())
    }

    pub fn release_all(&mut self) -> PyResult<()> {
        self.as_mut()?.release_all();
        Ok(())
    }

    pub fn reset_all(&mut self) -> PyResult<()> {
        self.as_mut()?.reset_all();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(touches) => format!("Touches(active={})", touches.iter().count()),
            Err(_) => "Touches(<invalid>)".to_string(),
        }
    }
}

/// Owned touch snapshot returned by `Touches` accessors.
#[pyclass(name = "Touch", module = "pybevy.input", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTouch {
    id: u64,
    start_position: Vec2,
    start_force: Option<PyForceTouch>,
    previous_position: Vec2,
    previous_force: Option<PyForceTouch>,
    position: Vec2,
    force: Option<PyForceTouch>,
}

impl PyTouch {
    pub fn from_bevy(touch: &Touch) -> Self {
        PyTouch {
            id: touch.id(),
            start_position: touch.start_position(),
            start_force: touch.start_force().map(Into::into),
            previous_position: touch.previous_position(),
            previous_force: touch.previous_force().map(Into::into),
            position: touch.position(),
            force: touch.force().map(Into::into),
        }
    }
}

#[pymethods]
impl PyTouch {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn position(&self) -> Snapshot<Vec2> {
        Snapshot(self.position)
    }

    #[getter]
    fn start_position(&self) -> Snapshot<Vec2> {
        Snapshot(self.start_position)
    }

    #[getter]
    fn previous_position(&self) -> Snapshot<Vec2> {
        Snapshot(self.previous_position)
    }

    #[getter]
    fn force(&self) -> Option<PyForceTouch> {
        self.force
    }

    #[getter]
    fn start_force(&self) -> Option<PyForceTouch> {
        self.start_force
    }

    #[getter]
    fn previous_force(&self) -> Option<PyForceTouch> {
        self.previous_force
    }

    pub fn delta(&self) -> PyVec2 {
        (self.position - self.previous_position).into()
    }

    pub fn distance(&self) -> PyVec2 {
        (self.position - self.start_position).into()
    }

    fn __repr__(&self) -> String {
        format!(
            "Touch(id={}, position={}, force={})",
            self.id,
            self.position.repr_literal(),
            self.force
                .map_or_else(|| "None".to_string(), |force| force.__repr__())
        )
    }
}
