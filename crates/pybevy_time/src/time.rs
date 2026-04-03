use std::time::Duration;

use bevy::time::{Fixed, Real, Time, Virtual};
use pybevy_core::{PyResource, ResourceStorage};
use pybevy_macros::pyresource;
use pyo3::prelude::*;

#[pyresource(Time, bridge)]
#[pyclass(name = "Time", extends = PyResource)]
#[derive(Debug)]
pub struct PyTime {
    pub storage: ResourceStorage<Time>,
}

#[pymethods]
impl PyTime {
    #[new]
    pub fn new() -> (Self, PyResource) {
        (Time::default().into(), PyResource)
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
        format!("{:?}", self)
    }
}

#[pyresource(Time<Fixed>, bridge, "TimeFixed")]
#[pyclass(name = "TimeFixed", extends = PyResource)]
#[derive(Debug)]
pub struct PyTimeFixed {
    pub storage: ResourceStorage<Time<Fixed>>,
}

#[pymethods]
impl PyTimeFixed {
    #[new]
    pub fn new() -> (Self, PyResource) {
        (Time::<Fixed>::default().into(), PyResource)
    }

    #[staticmethod]
    pub fn from_duration(py: Python<'_>, timestep: Duration) -> PyResult<Py<PyTimeFixed>> {
        Py::new(
            py,
            (Time::<Fixed>::from_duration(timestep).into(), PyResource),
        )
    }

    #[staticmethod]
    pub fn from_hz(py: Python<'_>, hz: f64) -> PyResult<Py<PyTimeFixed>> {
        Py::new(py, (Time::<Fixed>::from_hz(hz).into(), PyResource))
    }

    #[staticmethod]
    pub fn from_seconds(py: Python<'_>, seconds: f64) -> PyResult<Py<PyTimeFixed>> {
        Py::new(
            py,
            (Time::<Fixed>::from_seconds(seconds).into(), PyResource),
        )
    }

    pub fn timestep(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.timestep())
    }

    pub fn set_timestep(&mut self, timestep: Duration) -> PyResult<()> {
        self.as_mut()?.set_timestep(timestep);
        Ok(())
    }

    pub fn set_timestep_seconds(&mut self, seconds: f64) -> PyResult<()> {
        self.as_mut()?.set_timestep_seconds(seconds);
        Ok(())
    }

    pub fn set_timestep_hz(&mut self, hz: f64) -> PyResult<()> {
        self.as_mut()?.set_timestep_hz(hz);
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
        format!("{:?}", self)
    }
}

#[pyresource(Time<Virtual>, bridge, "TimeVirtual")]
#[pyclass(name = "TimeVirtual", extends = PyResource)]
#[derive(Debug)]
pub struct PyTimeVirtual {
    pub storage: ResourceStorage<Time<Virtual>>,
}

#[pymethods]
impl PyTimeVirtual {
    #[new]
    pub fn new() -> (Self, PyResource) {
        (Time::<Virtual>::default().into(), PyResource)
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
        format!("{:?}", self)
    }
}

#[pyresource(Time<Real>)]
#[pyclass(name = "TimeReal", extends = PyResource)]
#[derive(Debug)]
pub struct PyTimeReal {
    pub storage: ResourceStorage<Time<Real>>,
}

#[pymethods]
impl PyTimeReal {
    #[new]
    pub fn new() -> (Self, PyResource) {
        (Time::<Real>::default().into(), PyResource)
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
        format!("{:?}", self)
    }
}
