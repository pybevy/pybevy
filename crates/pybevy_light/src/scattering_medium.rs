use std::borrow::Cow;

use bevy::{
    asset::Handle,
    image::Image,
    light::atmosphere::{ScatteringMedium, ScatteringTerm},
};
use pybevy_core::{AssetStorage, PyAsset, extract_handle_from_any};
use pybevy_macros::pyasset;
use pyo3::prelude::*;

use crate::scattering_term::PyScatteringTerm;

#[pyasset(ScatteringMedium, bridge, not_loadable)]
#[pyclass(name = "ScatteringMedium", extends = PyAsset, skip_from_py_object)]
pub struct PyScatteringMedium {
    pub(crate) storage: AssetStorage<ScatteringMedium>,
}

#[pymethods]
impl PyScatteringMedium {
    #[new]
    #[pyo3(signature = (falloff_resolution = 256, phase_resolution = 256, terms = None))]
    pub fn new(
        falloff_resolution: u32,
        phase_resolution: u32,
        terms: Option<Vec<PyScatteringTerm>>,
    ) -> PyClassInitializer<Self> {
        // terms omitted keeps bevy's Default: ScatteringMedium::earth(256, 256)
        let medium = match terms {
            Some(terms) => ScatteringMedium::new(
                falloff_resolution,
                phase_resolution,
                terms.into_iter().map(ScatteringTerm::from),
            ),
            None => ScatteringMedium::earth(falloff_resolution, phase_resolution),
        };
        Self::from_owned(medium).into()
    }

    #[getter]
    pub fn label(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.label.as_deref().map(str::to_owned))
    }

    #[setter]
    pub fn set_label(&mut self, label: Option<String>) -> PyResult<()> {
        self.as_mut()?.label = label.map(Cow::Owned);
        Ok(())
    }

    #[getter]
    pub fn falloff_resolution(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.falloff_resolution)
    }

    #[setter]
    pub fn set_falloff_resolution(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.falloff_resolution = value;
        Ok(())
    }

    #[getter]
    pub fn phase_resolution(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.phase_resolution)
    }

    #[setter]
    pub fn set_phase_resolution(&mut self, value: u32) -> PyResult<()> {
        self.as_mut()?.phase_resolution = value;
        Ok(())
    }

    #[getter]
    pub fn terms(&self) -> PyResult<Vec<PyScatteringTerm>> {
        Ok(self
            .as_ref()?
            .terms
            .iter()
            .cloned()
            .map(Into::into)
            .collect())
    }

    #[setter]
    pub fn set_terms(&mut self, terms: Vec<PyScatteringTerm>) -> PyResult<()> {
        self.as_mut()?.terms = terms.into_iter().map(Into::into).collect();
        Ok(())
    }

    #[staticmethod]
    #[pyo3(signature = (falloff_resolution = 256, phase_resolution = 256))]
    pub fn earth(falloff_resolution: u32, phase_resolution: u32) -> PyResult<Py<Self>> {
        Python::attach(|py| {
            Py::new(
                py,
                Self::from_owned(ScatteringMedium::earth(
                    falloff_resolution,
                    phase_resolution,
                )),
            )
        })
    }

    #[staticmethod]
    #[pyo3(signature = (falloff_resolution = 256, phase_resolution = 256, *, dust_phase))]
    pub fn mars(
        falloff_resolution: u32,
        phase_resolution: u32,
        dust_phase: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        let handle = extract_handle_from_any(dust_phase)?;
        let dust_phase: Handle<Image> = handle.try_into()?;
        Python::attach(|py| {
            Py::new(
                py,
                Self::from_owned(ScatteringMedium::mars(
                    falloff_resolution,
                    phase_resolution,
                    dust_phase.clone(),
                )),
            )
        })
    }
}
