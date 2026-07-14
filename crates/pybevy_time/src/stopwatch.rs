use std::time::Duration;

use bevy::time::Stopwatch;
use pybevy_core::duration_from_py;
use pyo3::prelude::*;

#[pyclass(name = "Stopwatch", eq, skip_from_py_object)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PyStopwatch {
    pub(crate) stopwatch: Stopwatch,
}

#[pymethods]
impl PyStopwatch {
    #[new]
    pub fn new() -> Self {
        Self {
            stopwatch: Stopwatch::new(),
        }
    }

    pub fn tick(&mut self, delta: &Bound<'_, PyAny>) -> PyResult<()> {
        self.stopwatch.tick(duration_from_py(delta)?);
        Ok(())
    }

    pub fn elapsed(&self) -> Duration {
        self.stopwatch.elapsed()
    }

    pub fn elapsed_secs(&self) -> f32 {
        self.stopwatch.elapsed_secs()
    }

    pub fn elapsed_secs_f64(&self) -> f64 {
        self.stopwatch.elapsed_secs_f64()
    }

    pub fn set_elapsed(&mut self, time: &Bound<'_, PyAny>) -> PyResult<()> {
        self.stopwatch.set_elapsed(duration_from_py(time)?);
        Ok(())
    }

    pub fn pause(&mut self) {
        self.stopwatch.pause();
    }

    pub fn unpause(&mut self) {
        self.stopwatch.unpause();
    }

    pub fn is_paused(&self) -> bool {
        self.stopwatch.is_paused()
    }

    pub fn reset(&mut self) {
        self.stopwatch.reset();
    }

    pub fn __repr__(&self) -> String {
        format!("{:?}", self.stopwatch)
    }
}
