use std::time::Duration;

use bevy::time::Stopwatch;
use pybevy_core::{
    DURATION_DOMAIN, check_duration,
    domain::DurationInput,
    repr::{ReprLiteral, object_repr},
};
use pyo3::prelude::*;

#[pyclass(name = "Stopwatch", module = "pybevy.time", eq, skip_from_py_object)]
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

    pub fn tick<'py>(
        mut slf: PyRefMut<'py, Self>,
        delta: DurationInput,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let delta = check_duration("Stopwatch", "delta", delta, DURATION_DOMAIN)?;
        slf.stopwatch.tick(delta);
        Ok(slf)
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

    pub fn set_elapsed(&mut self, time: DurationInput) -> PyResult<()> {
        let time = check_duration("Stopwatch", "elapsed", time, DURATION_DOMAIN)?;
        self.stopwatch.set_elapsed(time);
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
        format!(
            "Stopwatch(elapsed={}, is_paused={})",
            object_repr(self.stopwatch.elapsed()),
            self.stopwatch.is_paused().repr_literal()
        )
    }
}
