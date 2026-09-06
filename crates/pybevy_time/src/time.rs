use std::time::Duration;

use bevy::time::{Fixed, Real, Time, Virtual};
use pybevy_core::{
    PyResource, ResourceStorage, duration_from_hz, positive_duration_from_secs_f64,
    public_error::{DURATION_ZERO, TIME_CONTEXT_TYPE_REQUIRED},
    resource_initializer,
};
use pybevy_macros::pyresource;
use pyo3::{PyTypeInfo, exceptions::PyTypeError, prelude::*, types::PyType};

use crate::time_context::{PyFixed, PyReal, PyVirtual};

fn require_positive_duration(duration: Duration) -> PyResult<Duration> {
    if duration.is_zero() {
        return Err(PyTypeError::new_err(DURATION_ZERO));
    }
    Ok(duration)
}

#[pyresource(Time, bridge)]
#[pyclass(name = "Time", module = "pybevy.time", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyTime {
    pub storage: ResourceStorage<Time>,
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

    pub fn advance_by(&mut self, delta: Duration) -> PyResult<()> {
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn set_wrap_period(&mut self, wrap_period: Duration) -> PyResult<()> {
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

    pub fn advance_to(&mut self, elapsed: Duration) -> PyResult<()> {
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

#[pyresource(Time<Fixed>, bridge, "_TimeFixed")]
#[pyclass(name = "_TimeFixed", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyTimeFixed {
    pub storage: ResourceStorage<Time<Fixed>>,
}

#[pymethods]
impl PyTimeFixed {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Time::<Fixed>::default().into())
    }

    #[staticmethod]
    pub fn from_duration(py: Python<'_>, timestep: Duration) -> PyResult<Py<PyTimeFixed>> {
        let timestep = require_positive_duration(timestep)?;
        Py::new(
            py,
            resource_initializer(Time::<Fixed>::from_duration(timestep).into()),
        )
    }

    #[staticmethod]
    pub fn from_hz(py: Python<'_>, hz: f64) -> PyResult<Py<PyTimeFixed>> {
        let timestep = duration_from_hz(hz)?;
        Py::new(
            py,
            resource_initializer(Time::<Fixed>::from_duration(timestep).into()),
        )
    }

    #[staticmethod]
    pub fn from_seconds(py: Python<'_>, seconds: f64) -> PyResult<Py<PyTimeFixed>> {
        let timestep = positive_duration_from_secs_f64(seconds)?;
        Py::new(
            py,
            resource_initializer(Time::<Fixed>::from_duration(timestep).into()),
        )
    }

    pub fn timestep(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.timestep())
    }

    pub fn set_timestep(&mut self, timestep: Duration) -> PyResult<()> {
        self.as_mut()?
            .set_timestep(require_positive_duration(timestep)?);
        Ok(())
    }

    pub fn set_timestep_seconds(&mut self, seconds: f64) -> PyResult<()> {
        self.as_mut()?
            .set_timestep(positive_duration_from_secs_f64(seconds)?);
        Ok(())
    }

    pub fn set_timestep_hz(&mut self, hz: f64) -> PyResult<()> {
        self.as_mut()?.set_timestep(duration_from_hz(hz)?);
        Ok(())
    }

    pub fn overstep(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.overstep())
    }

    pub fn discard_overstep(&mut self, discard: Duration) -> PyResult<()> {
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

    pub fn advance_to(&mut self, elapsed: Duration) -> PyResult<()> {
        self.as_mut()?.advance_to(elapsed);
        Ok(())
    }

    pub fn advance_by(&mut self, delta: Duration) -> PyResult<()> {
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn wrap_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.wrap_period())
    }

    pub fn set_wrap_period(&mut self, wrap_period: Duration) -> PyResult<()> {
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
#[pyclass(name = "_TimeVirtual", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyTimeVirtual {
    pub storage: ResourceStorage<Time<Virtual>>,
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
        self.as_mut()?.set_relative_speed(ratio);
        Ok(())
    }

    pub fn set_relative_speed_f64(&mut self, ratio: f64) -> PyResult<()> {
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

    pub fn set_max_delta(&mut self, max_delta: Duration) -> PyResult<()> {
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

    pub fn advance_to(&mut self, elapsed: Duration) -> PyResult<()> {
        self.as_mut()?.advance_to(elapsed);
        Ok(())
    }

    pub fn advance_by(&mut self, delta: Duration) -> PyResult<()> {
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn wrap_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.wrap_period())
    }

    pub fn set_wrap_period(&mut self, wrap_period: Duration) -> PyResult<()> {
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
                t.relative_speed(),
                if t.is_paused() { "True" } else { "False" }
            ),
            Err(_) => "Time[Virtual](<invalid>)".to_string(),
        }
    }
}

#[pyresource(Time<Real>, bridge, "_TimeReal")]
#[pyclass(name = "_TimeReal", extends = PyResource, from_py_object)]
#[derive(Debug)]
pub struct PyTimeReal {
    pub storage: ResourceStorage<Time<Real>>,
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

    pub fn advance_to(&mut self, elapsed: Duration) -> PyResult<()> {
        self.as_mut()?.advance_to(elapsed);
        Ok(())
    }

    pub fn advance_by(&mut self, delta: Duration) -> PyResult<()> {
        self.as_mut()?.advance_by(delta);
        Ok(())
    }

    pub fn wrap_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.wrap_period())
    }

    pub fn set_wrap_period(&mut self, wrap_period: Duration) -> PyResult<()> {
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
