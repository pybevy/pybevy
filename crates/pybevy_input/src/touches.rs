use bevy::input::touch::{ForceTouch, Touch, Touches};
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_math::PyVec2;
use pyo3::prelude::*;

#[pyclass(name = "Touches", extends = PyResource)]
#[derive(Clone)]
pub struct PyTouches {
    storage: Option<ResourceStorage<Touches>>,
}

impl PyTouches {
    pub fn from_borrowed(storage: ResourceStorage<Touches>) -> (Self, PyResource) {
        (
            PyTouches {
                storage: Some(storage),
            },
            PyResource,
        )
    }

    fn get_touches(&self) -> PyResult<&Touches> {
        match &self.storage {
            Some(storage) => Ok(storage.as_ref()?),
            None => {
                static EMPTY_TOUCHES: std::sync::OnceLock<Touches> = std::sync::OnceLock::new();
                Ok(EMPTY_TOUCHES.get_or_init(Touches::default))
            }
        }
    }

    fn get_touches_mut(&mut self) -> PyResult<&mut Touches> {
        match &mut self.storage {
            Some(storage) => Ok(storage.as_mut()?),
            None => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Cannot mutate uninitialized Touches resource",
            )),
        }
    }
}

#[pymethods]
impl PyTouches {
    #[new]
    pub fn new() -> (Self, PyResource) {
        (PyTouches { storage: None }, PyResource)
    }

    pub fn any_just_pressed(&self) -> PyResult<bool> {
        let touches = self.get_touches()?;
        Ok(touches.any_just_pressed())
    }

    pub fn any_just_released(&self) -> PyResult<bool> {
        let touches = self.get_touches()?;
        Ok(touches.any_just_released())
    }

    pub fn any_just_canceled(&self) -> PyResult<bool> {
        let touches = self.get_touches()?;
        Ok(touches.any_just_canceled())
    }

    pub fn just_pressed(&self, id: u64) -> PyResult<bool> {
        let touches = self.get_touches()?;
        Ok(touches.just_pressed(id))
    }

    pub fn just_released(&self, id: u64) -> PyResult<bool> {
        let touches = self.get_touches()?;
        Ok(touches.just_released(id))
    }

    pub fn just_canceled(&self, id: u64) -> PyResult<bool> {
        let touches = self.get_touches()?;
        Ok(touches.just_canceled(id))
    }

    pub fn get_pressed(&self, id: u64) -> PyResult<Option<PyTouch>> {
        let touches = self.get_touches()?;
        Ok(touches.get_pressed(id).map(PyTouch::from_bevy))
    }

    pub fn get_released(&self, id: u64) -> PyResult<Option<PyTouch>> {
        let touches = self.get_touches()?;
        Ok(touches.get_released(id).map(PyTouch::from_bevy))
    }

    pub fn iter(&self) -> PyResult<Vec<PyTouch>> {
        let touches = self.get_touches()?;
        Ok(touches.iter().map(PyTouch::from_bevy).collect())
    }

    pub fn iter_just_pressed(&self) -> PyResult<Vec<PyTouch>> {
        let touches = self.get_touches()?;
        Ok(touches
            .iter_just_pressed()
            .map(PyTouch::from_bevy)
            .collect())
    }

    pub fn iter_just_released(&self) -> PyResult<Vec<PyTouch>> {
        let touches = self.get_touches()?;
        Ok(touches
            .iter_just_released()
            .map(PyTouch::from_bevy)
            .collect())
    }

    pub fn iter_just_canceled(&self) -> PyResult<Vec<PyTouch>> {
        let touches = self.get_touches()?;
        Ok(touches
            .iter_just_canceled()
            .map(PyTouch::from_bevy)
            .collect())
    }

    pub fn first_pressed_position(&self) -> PyResult<Option<PyVec2>> {
        let touches = self.get_touches()?;
        Ok(touches.first_pressed_position().map(Into::into))
    }

    pub fn clear(&mut self) -> PyResult<()> {
        let touches = self.get_touches_mut()?;
        touches.clear();
        Ok(())
    }

    pub fn clear_just_pressed(&mut self, id: u64) -> PyResult<bool> {
        let touches = self.get_touches_mut()?;
        Ok(touches.clear_just_pressed(id))
    }

    pub fn clear_just_released(&mut self, id: u64) -> PyResult<bool> {
        let touches = self.get_touches_mut()?;
        Ok(touches.clear_just_released(id))
    }

    pub fn clear_just_canceled(&mut self, id: u64) -> PyResult<bool> {
        let touches = self.get_touches_mut()?;
        Ok(touches.clear_just_canceled(id))
    }

    pub fn release(&mut self, id: u64) -> PyResult<()> {
        let touches = self.get_touches_mut()?;
        touches.release(id);
        Ok(())
    }

    pub fn release_all(&mut self) -> PyResult<()> {
        let touches = self.get_touches_mut()?;
        touches.release_all();
        Ok(())
    }

    pub fn reset_all(&mut self) -> PyResult<()> {
        let touches = self.get_touches_mut()?;
        touches.reset_all();
        Ok(())
    }

    fn __repr__(&self) -> String {
        if self.storage.is_some() {
            "Touches(initialized)".to_string()
        } else {
            "Touches(uninitialized)".to_string()
        }
    }
}

#[pyclass(name = "Touch")]
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
