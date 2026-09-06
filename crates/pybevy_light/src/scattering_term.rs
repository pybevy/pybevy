use bevy::light::atmosphere::ScatteringTerm;
use pybevy_core::{FieldStorage, FromBorrowedStorage};
use pybevy_macros::pyfield;
use pybevy_math::vec3::PyVec3;
use pyo3::prelude::*;

use crate::{falloff::PyFalloff, phase_function::PyPhaseFunction};

#[pyfield]
#[pyclass(name = "ScatteringTerm", module = "pybevy.light", from_py_object)]
pub struct PyScatteringTerm {
    pub(crate) storage: FieldStorage<ScatteringTerm>,
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
    ) -> PyResult<Self> {
        Ok(Self::from_owned(ScatteringTerm {
            absorption: absorption.try_into()?,
            scattering: scattering.try_into()?,
            falloff: falloff.into(),
            phase: phase.into(),
        }))
    }

    #[getter]
    pub fn absorption(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|term| &term.absorption, |term| &mut term.absorption)?)
    }

    #[setter]
    pub fn set_absorption(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.absorption = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn scattering(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|term| &term.scattering, |term| &mut term.scattering)?)
    }

    #[setter]
    pub fn set_scattering(&mut self, value: PyVec3) -> PyResult<()> {
        self.as_mut()?.scattering = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn falloff(&self, py: Python<'_>) -> PyResult<Py<PyFalloff>> {
        PyFalloff::from_falloff(self.as_ref()?.falloff.clone(), py)
    }

    #[setter]
    pub fn set_falloff(&mut self, value: PyFalloff) -> PyResult<()> {
        self.as_mut()?.falloff = value.into();
        Ok(())
    }

    #[getter]
    pub fn phase(&self, py: Python<'_>) -> PyResult<Py<PyPhaseFunction>> {
        PyPhaseFunction::from_phase(self.as_ref()?.phase.clone(), py)
    }

    #[setter]
    pub fn set_phase(&mut self, value: PyPhaseFunction) -> PyResult<()> {
        self.as_mut()?.phase = value.into();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let term = self.as_ref()?;
        Ok(format!(
            "ScatteringTerm(absorption={:?}, scattering={:?}, falloff={}, phase={})",
            term.absorption,
            term.scattering,
            PyFalloff(term.falloff.clone()).__repr__(),
            PyPhaseFunction(term.phase.clone()).__repr__()
        ))
    }
}
