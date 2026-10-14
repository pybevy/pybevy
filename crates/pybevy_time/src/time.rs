use std::time::Duration;

use bevy::time::{Fixed, Real, Time, Virtual};
use pybevy_core::{
    PyResource, ResourceStorage,
    domain::{DurationInput, FINITE_NON_NEGATIVE, POSITIVE, TimedeltaInput, check, check_for},
    public_error::{self, DURATION_OVERFLOW, TIME_CONTEXT_TYPE_REQUIRED, time_moved_backwards},
    relative_speed,
    repr::float_repr,
    resource_initializer, timestep_from_hz,
};
use pybevy_macros::pyresource;
use pyo3::{
    PyClass, PyTypeInfo,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyType,
};

use crate::time_context::{PyFixed, PyReal, PyVirtual};

const FIXED: &str = "Time[Fixed]";
const VIRTUAL: &str = "Time[Virtual]";
const REAL: &str = "Time[Real]";

/// Bevy asserts that `advance_to` never moves time backwards.
fn check_advance_to<T: Default>(owner: &str, time: &Time<T>, elapsed: Duration) -> PyResult<()> {
    if elapsed < time.elapsed() {
        return Err(PyValueError::new_err(time_moved_backwards(
            owner,
            elapsed,
            time.elapsed(),
        )));
    }
    Ok(())
}

/// Bevy adds `delta` to the elapsed time with a panicking `Duration` add.
fn check_advance_by<T: Default>(time: &Time<T>, delta: Duration) -> PyResult<()> {
    time.elapsed()
        .checked_add(delta)
        .map(drop)
        .ok_or_else(|| PyValueError::new_err(DURATION_OVERFLOW))
}

#[pyresource(Time, bridge)]
#[pyclass(name = "Time", module = "pybevy.time", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyTime {
    pub(crate) storage: ResourceStorage<Time>,
}

#[pymethods]
impl PyTime {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Time::default().into())
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let specialized = if key.is(PyFixed::type_object(py).as_any()) {
            PyTimeFixed::type_object(py)
        } else if key.is(PyReal::type_object(py).as_any()) {
            PyTimeReal::type_object(py)
        } else if key.is(PyVirtual::type_object(py).as_any()) {
            PyTimeVirtual::type_object(py)
        } else {
            return Err(PyTypeError::new_err(TIME_CONTEXT_TYPE_REQUIRED));
        };
        Ok(specialized.unbind().into_any())
    }

    pub fn advance_by(&mut self, delta: TimedeltaInput) -> PyResult<()> {
        let delta = check_for::<Self, _>("delta", delta, FINITE_NON_NEGATIVE)?.stored();
        check_advance_by(self.as_ref()?, delta)?;
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn set_wrap_period(&mut self, wrap_period: TimedeltaInput) -> PyResult<()> {
        let wrap_period = check_for::<Self, _>("wrap_period", wrap_period, POSITIVE)?.stored();
        self.as_mut()?.set_wrap_period(wrap_period);
        Ok(())
    }

    pub fn wrap_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.wrap_period())
    }

    pub fn delta_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.delta_secs())
    }

    pub fn delta_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.delta_secs_f64())
    }

    pub fn elapsed(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed())
    }

    pub fn elapsed_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs())
    }

    pub fn elapsed_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_f64())
    }

    pub fn elapsed_wrapped(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed_wrapped())
    }

    pub fn elapsed_secs_wrapped(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs_wrapped())
    }

    pub fn elapsed_secs_wrapped_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_wrapped_f64())
    }

    pub fn delta(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.delta())
    }

    pub fn advance_to(&mut self, elapsed: TimedeltaInput) -> PyResult<()> {
        let elapsed = check_for::<Self, _>("elapsed", elapsed, FINITE_NON_NEGATIVE)?.stored();
        check_advance_to(<Self as PyClass>::NAME, self.as_ref()?, elapsed)?;
        self.as_mut()?.advance_to(elapsed);
        Ok(())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(t) => format!(
                "Time(elapsed={:.3}s, delta={:.3}s)",
                t.elapsed_secs_f64(),
                t.delta_secs_f64()
            ),
            Err(_) => "Time(<invalid>)".to_string(),
        }
    }
}

/// Every frame runs `FixedUpdate` once per elapsed timestep, so a tiny one stalls the loop.
const MIN_FIXED_TIMESTEP: Duration = Duration::from_micros(100);

fn playable_timestep(timestep: Duration) -> PyResult<Duration> {
    if timestep < MIN_FIXED_TIMESTEP {
        return Err(PyValueError::new_err(
            public_error::fixed_timestep_too_small(timestep.as_secs_f64()),
        ));
    }
    Ok(timestep)
}

#[pyresource(Time<Fixed>, bridge, "_TimeFixed")]
#[pyclass(
    name = "_TimeFixed",
    module = "pybevy.time",
    extends = PyResource,
    from_py_object
)]
#[derive(Debug)]
pub struct PyTimeFixed {
    pub(crate) storage: ResourceStorage<Time<Fixed>>,
}

#[pymethods]
impl PyTimeFixed {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Time::<Fixed>::default().into())
    }

    #[staticmethod]
    pub fn from_duration(py: Python<'_>, timestep: TimedeltaInput) -> PyResult<Py<PyTimeFixed>> {
        let timestep = playable_timestep(check(FIXED, "timestep", timestep, POSITIVE)?.stored())?;
        Py::new(
            py,
            resource_initializer(Time::<Fixed>::from_duration(timestep).into()),
        )
    }

    #[staticmethod]
    pub fn from_hz(py: Python<'_>, hz: f64) -> PyResult<Py<PyTimeFixed>> {
        let timestep = playable_timestep(timestep_from_hz(FIXED, hz)?)?;
        Py::new(
            py,
            resource_initializer(Time::<Fixed>::from_duration(timestep).into()),
        )
    }

    #[staticmethod]
    pub fn from_seconds(py: Python<'_>, seconds: f64) -> PyResult<Py<PyTimeFixed>> {
        let timestep = playable_timestep(
            check(FIXED, "timestep", DurationInput::Seconds(seconds), POSITIVE)?.stored(),
        )?;
        Py::new(
            py,
            resource_initializer(Time::<Fixed>::from_duration(timestep).into()),
        )
    }

    pub fn timestep(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.timestep())
    }

    pub fn set_timestep(&mut self, timestep: TimedeltaInput) -> PyResult<()> {
        let timestep = playable_timestep(check(FIXED, "timestep", timestep, POSITIVE)?.stored())?;
        self.as_mut()?.set_timestep(timestep);
        Ok(())
    }

    pub fn set_timestep_seconds(&mut self, seconds: f64) -> PyResult<()> {
        let timestep = playable_timestep(
            check(FIXED, "timestep", DurationInput::Seconds(seconds), POSITIVE)?.stored(),
        )?;
        self.as_mut()?.set_timestep(timestep);
        Ok(())
    }

    pub fn set_timestep_hz(&mut self, hz: f64) -> PyResult<()> {
        let timestep = playable_timestep(timestep_from_hz(FIXED, hz)?)?;
        self.as_mut()?.set_timestep(timestep);
        Ok(())
    }

    pub fn overstep(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.overstep())
    }

    pub fn discard_overstep(&mut self, discard: TimedeltaInput) -> PyResult<()> {
        let discard = check(FIXED, "discard", discard, FINITE_NON_NEGATIVE)?.stored();
        self.as_mut()?.discard_overstep(discard);
        Ok(())
    }

    pub fn overstep_fraction(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.overstep_fraction())
    }

    pub fn overstep_fraction_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.overstep_fraction_f64())
    }

    pub fn delta_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.delta_secs())
    }

    pub fn delta_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.delta_secs_f64())
    }

    pub fn elapsed(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed())
    }

    pub fn elapsed_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs())
    }

    pub fn elapsed_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_f64())
    }

    pub fn delta(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.delta())
    }

    pub fn advance_to(&mut self, elapsed: TimedeltaInput) -> PyResult<()> {
        let elapsed = check(FIXED, "elapsed", elapsed, FINITE_NON_NEGATIVE)?.stored();
        check_advance_to(FIXED, self.as_ref()?, elapsed)?;
        self.as_mut()?.advance_to(elapsed);
        Ok(())
    }

    pub fn advance_by(&mut self, delta: TimedeltaInput) -> PyResult<()> {
        let delta = check(FIXED, "delta", delta, FINITE_NON_NEGATIVE)?.stored();
        check_advance_by(self.as_ref()?, delta)?;
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn wrap_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.wrap_period())
    }

    pub fn set_wrap_period(&mut self, wrap_period: TimedeltaInput) -> PyResult<()> {
        let wrap_period = check(FIXED, "wrap_period", wrap_period, POSITIVE)?.stored();
        self.as_mut()?.set_wrap_period(wrap_period);
        Ok(())
    }

    pub fn elapsed_wrapped(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed_wrapped())
    }

    pub fn elapsed_secs_wrapped(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs_wrapped())
    }

    pub fn elapsed_secs_wrapped_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_wrapped_f64())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(t) => format!(
                "Time[Fixed](elapsed={:.3}s, timestep={:.3}s)",
                t.elapsed_secs_f64(),
                t.timestep().as_secs_f64()
            ),
            Err(_) => "Time[Fixed](<invalid>)".to_string(),
        }
    }
}

#[pyresource(Time<Virtual>, bridge, "_TimeVirtual")]
#[pyclass(
    name = "_TimeVirtual",
    module = "pybevy.time",
    extends = PyResource,
    from_py_object
)]
#[derive(Debug)]
pub struct PyTimeVirtual {
    pub(crate) storage: ResourceStorage<Time<Virtual>>,
}

#[pymethods]
impl PyTimeVirtual {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Time::<Virtual>::default().into())
    }

    pub fn pause(&mut self) -> PyResult<()> {
        self.as_mut()?.pause();
        Ok(())
    }

    pub fn unpause(&mut self) -> PyResult<()> {
        self.as_mut()?.unpause();
        Ok(())
    }

    pub fn is_paused(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_paused())
    }

    pub fn was_paused(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.was_paused())
    }

    pub fn set_relative_speed(&mut self, ratio: f32) -> PyResult<()> {
        let ratio = relative_speed(VIRTUAL, "relative_speed", ratio, self.as_ref()?.max_delta())?;
        self.as_mut()?.set_relative_speed_f64(ratio);
        Ok(())
    }

    pub fn set_relative_speed_f64(&mut self, ratio: f64) -> PyResult<()> {
        let ratio = relative_speed(VIRTUAL, "relative_speed", ratio, self.as_ref()?.max_delta())?;
        self.as_mut()?.set_relative_speed_f64(ratio);
        Ok(())
    }

    pub fn relative_speed(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.relative_speed())
    }

    pub fn relative_speed_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.relative_speed_f64())
    }

    pub fn effective_speed(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.effective_speed())
    }

    pub fn effective_speed_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.effective_speed_f64())
    }

    pub fn max_delta(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.max_delta())
    }

    pub fn set_max_delta(&mut self, max_delta: TimedeltaInput) -> PyResult<()> {
        let max_delta = check(VIRTUAL, "max_delta", max_delta, POSITIVE)?.stored();
        // Revalidate the stored speed against the new bound.
        let speed = self.as_ref()?.relative_speed_f64();
        relative_speed(VIRTUAL, "relative_speed", speed, max_delta)?;
        self.as_mut()?.set_max_delta(max_delta);
        Ok(())
    }

    pub fn delta(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.delta())
    }

    pub fn delta_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.delta_secs())
    }

    pub fn delta_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.delta_secs_f64())
    }

    pub fn elapsed(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed())
    }

    pub fn elapsed_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs())
    }

    pub fn elapsed_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_f64())
    }

    pub fn advance_to(&mut self, elapsed: TimedeltaInput) -> PyResult<()> {
        let elapsed = check(VIRTUAL, "elapsed", elapsed, FINITE_NON_NEGATIVE)?.stored();
        check_advance_to(VIRTUAL, self.as_ref()?, elapsed)?;
        self.as_mut()?.advance_to(elapsed);
        Ok(())
    }

    pub fn advance_by(&mut self, delta: TimedeltaInput) -> PyResult<()> {
        let delta = check(VIRTUAL, "delta", delta, FINITE_NON_NEGATIVE)?.stored();
        check_advance_by(self.as_ref()?, delta)?;
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn wrap_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.wrap_period())
    }

    pub fn set_wrap_period(&mut self, wrap_period: TimedeltaInput) -> PyResult<()> {
        let wrap_period = check(VIRTUAL, "wrap_period", wrap_period, POSITIVE)?.stored();
        self.as_mut()?.set_wrap_period(wrap_period);
        Ok(())
    }

    pub fn elapsed_wrapped(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed_wrapped())
    }

    pub fn elapsed_secs_wrapped(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs_wrapped())
    }

    pub fn elapsed_secs_wrapped_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_wrapped_f64())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(t) => format!(
                "Time[Virtual](elapsed={:.3}s, relative_speed={}, paused={})",
                t.elapsed_secs_f64(),
                float_repr(t.relative_speed_f64()),
                if t.is_paused() { "True" } else { "False" }
            ),
            Err(_) => "Time[Virtual](<invalid>)".to_string(),
        }
    }
}

#[pyresource(Time<Real>, bridge, "_TimeReal")]
#[pyclass(
    name = "_TimeReal",
    module = "pybevy.time",
    extends = PyResource,
    from_py_object
)]
#[derive(Debug)]
pub struct PyTimeReal {
    pub(crate) storage: ResourceStorage<Time<Real>>,
}

#[pymethods]
impl PyTimeReal {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Time::<Real>::default().into())
    }

    pub fn delta(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.delta())
    }

    pub fn delta_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.delta_secs())
    }

    pub fn delta_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.delta_secs_f64())
    }

    pub fn elapsed(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed())
    }

    pub fn elapsed_secs(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs())
    }

    pub fn elapsed_secs_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_f64())
    }

    pub fn advance_to(&mut self, elapsed: TimedeltaInput) -> PyResult<()> {
        let elapsed = check(REAL, "elapsed", elapsed, FINITE_NON_NEGATIVE)?.stored();
        check_advance_to(REAL, self.as_ref()?, elapsed)?;
        self.as_mut()?.advance_to(elapsed);
        Ok(())
    }

    pub fn advance_by(&mut self, delta: TimedeltaInput) -> PyResult<()> {
        let delta = check(REAL, "delta", delta, FINITE_NON_NEGATIVE)?.stored();
        check_advance_by(self.as_ref()?, delta)?;
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn wrap_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.wrap_period())
    }

    pub fn set_wrap_period(&mut self, wrap_period: TimedeltaInput) -> PyResult<()> {
        let wrap_period = check(REAL, "wrap_period", wrap_period, POSITIVE)?.stored();
        self.as_mut()?.set_wrap_period(wrap_period);
        Ok(())
    }

    pub fn elapsed_wrapped(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.elapsed_wrapped())
    }

    pub fn elapsed_secs_wrapped(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.elapsed_secs_wrapped())
    }

    pub fn elapsed_secs_wrapped_f64(&self) -> PyResult<f64> {
        Ok(self.as_ref()?.elapsed_secs_wrapped_f64())
    }

    pub fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(t) => format!(
                "Time[Real](elapsed={:.3}s, delta={:.3}s)",
                t.elapsed_secs_f64(),
                t.delta_secs_f64()
            ),
            Err(_) => "Time[Real](<invalid>)".to_string(),
        }
    }
}
