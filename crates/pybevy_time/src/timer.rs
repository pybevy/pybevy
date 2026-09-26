use std::time::Duration;

use bevy::time::{Timer, TimerMode};
use pybevy_core::{
    duration_from_py, duration_from_secs_f64, public_error::TIMER_ELAPSED_PAST_DURATION,
};
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyValueError, prelude::*};

#[pyenum(TimerMode)]
#[pyclass(
    name = "TimerMode",
    module = "pybevy.time",
    eq,
    from_py_object,
    frozen,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyTimerMode {
    Once,
    Repeating,
}

#[pymethods]
impl PyTimerMode {
    pub fn __str__(&self) -> &'static str {
        match self {
            PyTimerMode::Once => "once",
            PyTimerMode::Repeating => "repeating",
        }
    }
}

#[pyclass(name = "Timer", module = "pybevy.time", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyTimer {
    pub(crate) timer: Timer,
    count_overflowed: bool,
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
            count_overflowed: false,
        })
    }

    #[staticmethod]
    pub fn from_seconds(duration: f32, mode: PyTimerMode) -> PyResult<Self> {
        Ok(Self {
            timer: Timer::new(duration_from_secs_f64(duration.into())?, mode.into()),
            count_overflowed: false,
        })
    }

    pub fn tick<'py>(
        mut slf: PyRefMut<'py, Self>,
        delta: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let delta = duration_from_py(delta)?;
        slf.count_overflowed = slf.would_overflow_finishes(delta);
        slf.timer.tick(delta);
        Ok(slf)
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
        self.count_overflowed = false;
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

    pub fn remaining(&self) -> PyResult<Duration> {
        self.checked_remaining()
    }

    pub fn remaining_secs(&self) -> PyResult<f32> {
        Ok(self.checked_remaining()?.as_secs_f32())
    }

    pub fn times_finished_this_tick(&self) -> u32 {
        if self.count_overflowed {
            u32::MAX
        } else {
            self.timer.times_finished_this_tick()
        }
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
        self.count_overflowed = false;
    }

    pub fn almost_finish(&mut self) {
        self.timer.almost_finish();
        self.count_overflowed = false;
    }
}

impl PyTimer {
    fn would_overflow_finishes(&self, delta: Duration) -> bool {
        if self.timer.mode() != TimerMode::Repeating || self.timer.is_paused() {
            return false;
        }
        let period = self.timer.duration().as_nanos();
        period != 0
            && self.timer.elapsed().saturating_add(delta).as_nanos() / period > u32::MAX as u128
    }

    // set_elapsed() accepts any value, and Bevy's Duration subtraction panics past the duration.
    fn checked_remaining(&self) -> PyResult<Duration> {
        self.timer
            .duration()
            .checked_sub(self.timer.elapsed())
            .ok_or_else(|| PyValueError::new_err(TIMER_ELAPSED_PAST_DURATION))
    }
}
