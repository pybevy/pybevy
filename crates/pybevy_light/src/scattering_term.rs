use bevy::light::atmosphere::ScatteringTerm;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

use crate::{falloff::PyFalloff, phase_function::PyPhaseFunction};

#[pyclass(name = "ScatteringTerm", from_py_object)]
#[derive(Clone)]
pub struct PyScatteringTerm {
    pub(crate) inner: ScatteringTerm,
}

impl From<ScatteringTerm> for PyScatteringTerm {
    fn from(term: ScatteringTerm) -> Self {
        PyScatteringTerm { inner: term }
    }
}

impl From<PyScatteringTerm> for ScatteringTerm {
    fn from(py_term: PyScatteringTerm) -> Self {
        py_term.inner
    }
}

#[pymethods]
impl PyScatteringTerm {
    #[new]
    #[pyo3(signature = (
        absorption = PyVec3::ZERO,
        scattering = PyVec3::ZERO,
        falloff = PyFalloff::linear(),
        phase = PyPhaseFunction::mie(0.8),
    ))]
    pub fn new(
        absorption: PyVec3,
        scattering: PyVec3,
        falloff: PyFalloff,
        phase: PyPhaseFunction,
    ) -> Self {
        PyScatteringTerm {
            inner: ScatteringTerm {
                absorption: absorption.into(),
                scattering: scattering.into(),
                falloff: falloff.into(),
                phase: phase.into(),
            },
        }
    }

    #[getter]
    pub fn absorption(&self) -> PyVec3 {
        self.inner.absorption.into()
    }

    #[setter]
    pub fn set_absorption(&mut self, value: PyVec3) {
        self.inner.absorption = value.into();
    }

    #[getter]
    pub fn scattering(&self) -> PyVec3 {
        self.inner.scattering.into()
    }

    #[setter]
    pub fn set_scattering(&mut self, value: PyVec3) {
        self.inner.scattering = value.into();
    }

    #[getter]
    pub fn falloff(&self) -> PyFalloff {
        self.inner.falloff.clone().into()
    }

    #[setter]
    pub fn set_falloff(&mut self, value: PyFalloff) {
        self.inner.falloff = value.into();
    }

    #[getter]
    pub fn phase(&self) -> PyPhaseFunction {
        self.inner.phase.clone().into()
    }

    #[setter]
    pub fn set_phase(&mut self, value: PyPhaseFunction) {
        self.inner.phase = value.into();
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ScatteringTerm(absorption={:?}, scattering={:?}, falloff={}, phase={})",
            self.inner.absorption,
            self.inner.scattering,
            PyFalloff(self.inner.falloff.clone()).__repr__(),
            PyPhaseFunction(self.inner.phase.clone()).__repr__()
        )
    }
}
