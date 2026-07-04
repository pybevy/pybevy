use std::sync::Arc;

use bevy::{asset::Handle, image::Image, light::atmosphere::PhaseFunction};
use pybevy_core::extract_handle_from_any;
use pyo3::prelude::*;

#[pyclass(name = "PhaseFunction", frozen, from_py_object)]
#[derive(Clone)]
pub struct PyPhaseFunction(pub(crate) PhaseFunction);

impl From<PhaseFunction> for PyPhaseFunction {
    fn from(val: PhaseFunction) -> Self {
        PyPhaseFunction(val)
    }
}

impl From<PyPhaseFunction> for PhaseFunction {
    fn from(val: PyPhaseFunction) -> Self {
        val.0
    }
}

#[pymethods]
impl PyPhaseFunction {
    #[staticmethod]
    #[pyo3(name = "Isotropic")]
    pub fn isotropic() -> Self {
        PyPhaseFunction(PhaseFunction::Isotropic)
    }

    #[staticmethod]
    #[pyo3(name = "Rayleigh")]
    pub fn rayleigh() -> Self {
        PyPhaseFunction(PhaseFunction::Rayleigh)
    }

    #[staticmethod]
    #[pyo3(name = "Mie")]
    pub fn mie(asymmetry: f32) -> Self {
        PyPhaseFunction(PhaseFunction::Mie { asymmetry })
    }

    #[staticmethod]
    #[pyo3(name = "ChromaticTexture")]
    pub fn chromatic_texture(image: &Bound<'_, PyAny>) -> PyResult<Self> {
        let image: Handle<Image> = extract_handle_from_any(image)?.try_into()?;
        Ok(PyPhaseFunction(PhaseFunction::ChromaticTexture(image)))
    }

    // Curve/ChromaticCurve hold Rust closures; not constructible from Python
    fn __eq__(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (PhaseFunction::Isotropic, PhaseFunction::Isotropic) => true,
            (PhaseFunction::Rayleigh, PhaseFunction::Rayleigh) => true,
            (PhaseFunction::Mie { asymmetry: a }, PhaseFunction::Mie { asymmetry: b }) => a == b,
            (PhaseFunction::Curve(a), PhaseFunction::Curve(b)) => Arc::ptr_eq(a, b),
            (PhaseFunction::ChromaticCurve(a), PhaseFunction::ChromaticCurve(b)) => {
                Arc::ptr_eq(a, b)
            }
            (PhaseFunction::ChromaticTexture(a), PhaseFunction::ChromaticTexture(b)) => a == b,
            _ => false,
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            PhaseFunction::Isotropic => "PhaseFunction.Isotropic()".to_string(),
            PhaseFunction::Rayleigh => "PhaseFunction.Rayleigh()".to_string(),
            PhaseFunction::Mie { asymmetry } => format!("PhaseFunction.Mie({})", asymmetry),
            PhaseFunction::Curve(_) => "PhaseFunction.Curve(...)".to_string(),
            PhaseFunction::ChromaticCurve(_) => "PhaseFunction.ChromaticCurve(...)".to_string(),
            PhaseFunction::ChromaticTexture(_) => "PhaseFunction.ChromaticTexture(...)".to_string(),
        }
    }
}
