use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::color_grading::{PyColorGradingGlobal, PyColorGradingSection};

#[pycomponent(ColorGrading, bridge)]
#[pyclass(name = "ColorGrading", extends = PyComponent)]
#[derive(Clone)]
pub struct PyColorGrading {
    pub(crate) storage: ComponentStorage<ColorGrading>,
}

#[pymethods]
impl PyColorGrading {
    #[new]
    #[pyo3(signature = (
        global_ = PyColorGradingGlobal::default(),
        shadows = PyColorGradingSection::default(),
        midtones = PyColorGradingSection::default(),
        highlights = PyColorGradingSection::default()
    ))]
    pub fn new(
        global_: PyColorGradingGlobal,
        shadows: PyColorGradingSection,
        midtones: PyColorGradingSection,
        highlights: PyColorGradingSection,
    ) -> PyResult<(Self, PyComponent)> {
        let color_grading = ColorGrading {
            global: global_.try_into()?,
            shadows: shadows.try_into()?,
            midtones: midtones.try_into()?,
            highlights: highlights.try_into()?,
        };

        Ok(Self::from_owned(color_grading))
    }

    #[getter(global_)]
    pub fn global(&self) -> PyResult<PyColorGradingGlobal> {
        Ok(self.storage.borrow_field_as(|c| &c.global)?)
    }

    #[setter(global_)]
    pub fn set_global(&mut self, global: PyColorGradingGlobal) -> PyResult<()> {
        self.as_mut()?.global = global.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn shadows(&self) -> PyResult<PyColorGradingSection> {
        Ok(self.storage.borrow_field_as(|c| &c.shadows)?)
    }

    #[setter]
    pub fn set_shadows(&mut self, shadows: PyColorGradingSection) -> PyResult<()> {
        self.as_mut()?.shadows = shadows.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn midtones(&self) -> PyResult<PyColorGradingSection> {
        Ok(self.storage.borrow_field_as(|c| &c.midtones)?)
    }

    #[setter]
    pub fn set_midtones(&mut self, midtones: PyColorGradingSection) -> PyResult<()> {
        self.as_mut()?.midtones = midtones.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn highlights(&self) -> PyResult<PyColorGradingSection> {
        Ok(self.storage.borrow_field_as(|c| &c.highlights)?)
    }

    #[setter]
    pub fn set_highlights(&mut self, highlights: PyColorGradingSection) -> PyResult<()> {
        self.as_mut()?.highlights = highlights.try_into()?;
        Ok(())
    }

    #[staticmethod]
    pub fn with_identical_sections(
        py: Python,
        global: PyColorGradingGlobal,
        section: PyColorGradingSection,
    ) -> PyResult<Py<Self>> {
        let bevy_global: ColorGradingGlobal = global.try_into()?;
        let bevy_section: ColorGradingSection = section.try_into()?;
        let color_grading = ColorGrading::with_identical_sections(bevy_global, bevy_section);
        Py::new(py, Self::from_owned(color_grading))
    }
}
