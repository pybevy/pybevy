use bevy::{asset::Handle, image::Image, light::atmosphere::PhaseFunction};
use pybevy_core::{PyHandle, extract_handle_from_any};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(PhaseFunction, manual)]
#[pyclass(
    name = "PhaseFunction",
    module = "pybevy.light",
    frozen,
    subclass,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPhaseFunction(pub(crate) PhaseFunction);

impl From<PyPhaseFunction> for PhaseFunction {
    fn from(value: PyPhaseFunction) -> Self {
        value.0
    }
}

#[pymethods]
impl PyPhaseFunction {
    #[new]
    pub fn new() -> PyResult<Self> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "PhaseFunction is an enum base; construct a nested variant",
        ))
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            PhaseFunction::Isotropic => "PhaseFunction.Isotropic()".to_string(),
            PhaseFunction::Rayleigh => "PhaseFunction.Rayleigh()".to_string(),
            PhaseFunction::Mie { asymmetry } => {
                format!("PhaseFunction.Mie(asymmetry={asymmetry})")
            }
            PhaseFunction::Curve(_) => "PhaseFunction.Curve(...)".to_string(),
            PhaseFunction::ChromaticCurve(_) => "PhaseFunction.ChromaticCurve(...)".to_string(),
            PhaseFunction::ChromaticTexture(_) => {
                "PhaseFunction.ChromaticTexture(image=...)".to_string()
            }
        }
    }
}

impl PyPhaseFunction {
    pub(crate) fn mie(asymmetry: f32) -> Self {
        Self(PhaseFunction::Mie { asymmetry })
    }

    pub(crate) fn from_phase(value: PhaseFunction, py: Python<'_>) -> PyResult<Py<Self>> {
        macro_rules! materialize_variant {
            ($variant:expr) => {{
                let value = Py::new(
                    py,
                    PyClassInitializer::from(Self(value)).add_subclass($variant),
                )?;
                Ok(value.into_bound(py).into_super().unbind())
            }};
        }

        match &value {
            PhaseFunction::Isotropic => materialize_variant!(PyPhaseFunctionIsotropic),
            PhaseFunction::Rayleigh => materialize_variant!(PyPhaseFunctionRayleigh),
            PhaseFunction::Mie { .. } => materialize_variant!(PyPhaseFunctionMie),
            PhaseFunction::ChromaticTexture(_) => {
                materialize_variant!(PyPhaseFunctionChromaticTexture)
            }
            // Bevy stores opaque Rust curves in these variants. They remain observable
            // through the base class, but have no constructible Python value model.
            PhaseFunction::Curve(_) | PhaseFunction::ChromaticCurve(_) => Py::new(py, Self(value)),
        }
    }
}

#[pyclass(
    name = "Isotropic",
    module = "pybevy.light",
    extends = PyPhaseFunction,
    frozen
)]
pub struct PyPhaseFunctionIsotropic;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyPhaseFunctionIsotropic {
    #[classattr]
    const __qualname__: &'static str = "PhaseFunction.Isotropic";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyPhaseFunction(PhaseFunction::Isotropic)).add_subclass(Self)
    }
}

#[pyclass(
    name = "Rayleigh",
    module = "pybevy.light",
    extends = PyPhaseFunction,
    frozen
)]
pub struct PyPhaseFunctionRayleigh;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyPhaseFunctionRayleigh {
    #[classattr]
    const __qualname__: &'static str = "PhaseFunction.Rayleigh";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyPhaseFunction(PhaseFunction::Rayleigh)).add_subclass(Self)
    }
}

#[pyclass(
    name = "Mie",
    module = "pybevy.light",
    extends = PyPhaseFunction,
    frozen
)]
pub struct PyPhaseFunctionMie;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyPhaseFunctionMie {
    #[classattr]
    const __qualname__: &'static str = "PhaseFunction.Mie";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("asymmetry",)
    }

    #[new]
    pub fn new(asymmetry: f32) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyPhaseFunction::mie(asymmetry)).add_subclass(Self)
    }

    #[getter]
    pub fn asymmetry(slf: PyRef<'_, Self>) -> f32 {
        let base = slf.into_super();
        match base.0 {
            PhaseFunction::Mie { asymmetry } => asymmetry,
            _ => unreachable!("PhaseFunction.Mie changed discriminant"),
        }
    }
}

#[pyclass(
    name = "ChromaticTexture",
    module = "pybevy.light",
    extends = PyPhaseFunction,
    frozen
)]
pub struct PyPhaseFunctionChromaticTexture;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyPhaseFunctionChromaticTexture {
    #[classattr]
    const __qualname__: &'static str = "PhaseFunction.ChromaticTexture";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("image",)
    }

    #[new]
    pub fn new(image: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let image: Handle<Image> = extract_handle_from_any(image)?.try_into()?;
        Ok(
            PyClassInitializer::from(PyPhaseFunction(PhaseFunction::ChromaticTexture(image)))
                .add_subclass(Self),
        )
    }

    #[getter]
    pub fn image(slf: PyRef<'_, Self>) -> PyHandle {
        let base = slf.into_super();
        match &base.0 {
            PhaseFunction::ChromaticTexture(image) => PyHandle::from(image),
            _ => unreachable!("PhaseFunction.ChromaticTexture changed discriminant"),
        }
    }
}

pub fn register_phase_function_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("PhaseFunction")?;
    base.setattr("Isotropic", py.get_type::<PyPhaseFunctionIsotropic>())?;
    base.setattr("Rayleigh", py.get_type::<PyPhaseFunctionRayleigh>())?;
    base.setattr("Mie", py.get_type::<PyPhaseFunctionMie>())?;
    base.setattr(
        "ChromaticTexture",
        py.get_type::<PyPhaseFunctionChromaticTexture>(),
    )?;
    Ok(())
}
