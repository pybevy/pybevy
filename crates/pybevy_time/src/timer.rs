use std::time::Duration;

use bevy::time::{Timer, TimerMode};
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyenum(TimerMode)]
#[pyclass(name = "TimerMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyTimerMode {
    Once,
    Repeating,
}

#[pymethods]
impl PyTimerMode {
    #[classattr]
    pub const ONCE: Self = PyTimerMode::Once;

    #[classattr]
    pub const REPEATING: Self = PyTimerMode::Repeating;

    pub fn __str__(&self) -> &'static str {
        match self {
            PyTimerMode::Once => "once",
            PyTimerMode::Repeating => "repeating",
        }
    }
}

fn duration_from_py(py_duration: &Bound<'_, PyAny>) -> PyResult<Duration> {
    if let Ok(duration) = py_duration.extract::<Duration>() {
        return Ok(duration);
    }

    if let Ok(seconds) = py_duration.extract::<f64>() {
        if seconds < 0.0 {
            return Err(PyTypeError::new_err("Duration cannot be negative"));
        }
        return Ok(Duration::from_secs_f64(seconds));
    }

    if let Ok(seconds) = py_duration.extract::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }

    Err(PyTypeError::new_err(
        "Duration must be a Duration object, float (seconds), or int (seconds)",
    ))
}

#[pyclass(name = "Timer", eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyTimer {
    pub(crate) timer: Timer,
}

#[pymethods]
impl PyTimer {
    #[new]
    #[pyo3(signature = (duration = None, mode = PyTimerMode::Once))]
    pub fn new(duration: Option<&Bound<'_, PyAny>>, mode: PyTimerMode) -> PyResult<Self> {
        let dur = match duration {
            Some(d) => duration_from_py(d)?,
            None => Duration::ZERO,
        };
        Ok(Self {
            timer: Timer::new(dur, mode.into()),
        })
    }

    #[staticmethod]
    pub fn from_seconds(duration: f32, mode: PyTimerMode) -> Self {
        Self {
            timer: Timer::from_seconds(duration, mode.into()),
        }
    }

    pub fn tick(&mut self, delta: &Bound<'_, PyAny>) -> PyResult<()> {
        self.timer.tick(duration_from_py(delta)?);
        Ok(())
    }

    pub fn finished(&self) -> bool {
        self.timer.is_finished()
    }

    pub fn is_finished(&self) -> bool {
        self.timer.is_finished()
    }

    pub fn just_finished(&self) -> bool {
        self.timer.just_finished()
    }

    pub fn elapsed(&self) -> Duration {
        self.timer.elapsed()
    }

    pub fn duration(&self) -> Duration {
        self.timer.duration()
    }

    pub fn set_duration(&mut self, duration: &Bound<'_, PyAny>) -> PyResult<()> {
        self.timer.set_duration(duration_from_py(duration)?);
        Ok(())
    }

    pub fn reset(&mut self) {
        self.timer.reset();
    }

    pub fn pause(&mut self) {
        self.timer.pause();
    }

    pub fn unpause(&mut self) {
        self.timer.unpause();
    }

    pub fn paused(&self) -> bool {
        self.timer.is_paused()
    }

    pub fn is_paused(&self) -> bool {
        self.timer.is_paused()
    }

    pub fn fraction(&self) -> f32 {
        self.timer.fraction()
    }

    pub fn fraction_remaining(&self) -> f32 {
        self.timer.fraction_remaining()
    }

    pub fn remaining(&self) -> Duration {
        self.timer.remaining()
    }

    pub fn remaining_secs(&self) -> f32 {
        self.timer.remaining_secs()
    }

    pub fn times_finished_this_tick(&self) -> u32 {
        self.timer.times_finished_this_tick()
    }

    pub fn elapsed_secs(&self) -> f32 {
        self.timer.elapsed_secs()
    }

    pub fn elapsed_secs_f64(&self) -> f64 {
        self.timer.elapsed_secs_f64()
    }

    pub fn set_elapsed(&mut self, time: &Bound<'_, PyAny>) -> PyResult<()> {
        self.timer.set_elapsed(duration_from_py(time)?);
        Ok(())
    }

    pub fn mode(&self) -> PyTimerMode {
        self.timer.mode().into()
    }

    pub fn set_mode(&mut self, mode: PyTimerMode) {
        self.timer.set_mode(mode.into());
    }

    pub fn finish(&mut self) {
        self.timer.finish();
    }

    pub fn almost_finish(&mut self) {
        self.timer.almost_finish();
    }
}
