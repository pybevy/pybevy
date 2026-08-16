use bevy::input::touch::{ForceTouch, Touch, Touches};
use pybevy_core::{PyResource, ResourceStorage, resource_initializer};
use pybevy_macros::pyresource;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

#[pyresource(Touches, no_clone, bridge, default_insert, no_reflect)]
#[pyclass(name = "Touches", extends = PyResource)]
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

/// Owned snapshot of touch data. Can't wrap Bevy's `Touch` directly because its fields are private
/// with no public constructor.
#[pyclass(name = "Touch", skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyTouch {
    id: u64,
    start_position: PyVec2,
    start_force: Option<f64>,
    previous_position: PyVec2,
    previous_force: Option<f64>,
    position: PyVec2,
    force: Option<f64>,
}

impl PyTouch {
    pub fn from_bevy(touch: &Touch) -> Self {
        PyTouch {
            id: touch.id(),
            start_position: touch.start_position().into(),
            start_force: touch.start_force().and_then(extract_force_value),
            previous_position: touch.previous_position().into(),
            previous_force: touch.previous_force().and_then(extract_force_value),
            position: touch.position().into(),
            force: touch.force().and_then(extract_force_value),
        }
    }
}

fn extract_force_value(force: ForceTouch) -> Option<f64> {
    match force {
        ForceTouch::Calibrated {
            force,
            max_possible_force,
            altitude_angle: _,
        } => Some(force / max_possible_force),
        ForceTouch::Normalized(value) => Some(value),
    }
}

#[pymethods]
impl PyTouch {
    #[new]
    fn new(id: u64, position: PyVec2) -> Self {
        PyTouch {
            id,
            start_position: position.clone(),
            start_force: None,
            previous_position: position.clone(),
            previous_force: None,
            position,
            force: None,
        }
    }

    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn position(&self) -> PyVec2 {
        self.position.clone()
    }

    #[getter]
    fn start_position(&self) -> PyVec2 {
        self.start_position.clone()
    }

    #[getter]
    fn previous_position(&self) -> PyVec2 {
        self.previous_position.clone()
    }

    #[getter]
    fn force(&self) -> Option<f64> {
        self.force
    }

    #[getter]
    fn start_force(&self) -> Option<f64> {
        self.start_force
    }

    #[getter]
    fn previous_force(&self) -> Option<f64> {
        self.previous_force
    }

    pub fn delta(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::new(
            self.position.x()? - self.previous_position.x()?,
            self.position.y()? - self.previous_position.y()?,
        ))
    }

    pub fn distance(&self) -> PyResult<PyVec2> {
        Ok(PyVec2::new(
            self.position.x()? - self.start_position.x()?,
            self.position.y()? - self.start_position.y()?,
        ))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Touch(id={}, position=({}, {}), force={:?})",
            self.id,
            self.position.x()?,
            self.position.y()?,
            self.force
        ))
    }
}
