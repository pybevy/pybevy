use bevy::light::atmosphere::Falloff;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(Falloff, manual)]
#[pyclass(
    name = "Falloff",
    module = "pybevy.light",
    frozen,
    subclass,
    from_py_object
)]
#[derive(Clone)]
pub struct PyFalloff(pub(crate) Falloff);

impl From<PyFalloff> for Falloff {
    fn from(value: PyFalloff) -> Self {
        value.0
    }
}

#[pymethods]
impl PyFalloff {
    #[new]
    pub fn new() -> PyResult<Self> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Falloff is an enum base; construct a nested variant",
        ))
    }

    pub fn __repr__(&self) -> String {
        match &self.0 {
            Falloff::Linear => "Falloff.Linear()".to_string(),
            Falloff::Exponential { scale } => {
                format!("Falloff.Exponential(scale={scale})")
            }
            Falloff::Tent { center, width } => {
                format!("Falloff.Tent(center={center}, width={width})")
            }
            Falloff::Curve(_) => "Falloff.Curve(...)".to_string(),
        }
    }
}

impl PyFalloff {
    pub(crate) fn linear() -> Self {
        Self(Falloff::Linear)
    }

    pub(crate) fn from_falloff(value: Falloff, py: Python<'_>) -> PyResult<Py<Self>> {
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
            Falloff::Linear => materialize_variant!(PyFalloffLinear),
            Falloff::Exponential { .. } => materialize_variant!(PyFalloffExponential),
            Falloff::Tent { .. } => materialize_variant!(PyFalloffTent),
            // Bevy stores an opaque Rust curve in this variant. It remains observable
            // through the base class, but has no constructible Python value model.
            Falloff::Curve(_) => Py::new(py, Self(value)),
        }
    }
}

#[pyclass(
    name = "Linear",
    module = "pybevy.light",
    extends = PyFalloff,
    frozen
)]
pub struct PyFalloffLinear;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyFalloffLinear {
    #[classattr]
    const __qualname__: &'static str = "Falloff.Linear";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyFalloff::linear()).add_subclass(Self)
    }
}

#[pyclass(
    name = "Exponential",
    module = "pybevy.light",
    extends = PyFalloff,
    frozen
)]
pub struct PyFalloffExponential;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyFalloffExponential {
    #[classattr]
    const __qualname__: &'static str = "Falloff.Exponential";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("scale",)
    }

    #[new]
    pub fn new(scale: f32) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyFalloff(Falloff::Exponential { scale })).add_subclass(Self)
    }

    #[getter]
    pub fn scale(slf: PyRef<'_, Self>) -> f32 {
        let base = slf.into_super();
        match base.0 {
            Falloff::Exponential { scale } => scale,
            _ => unreachable!("Falloff.Exponential changed discriminant"),
        }
    }
}

#[pyclass(
    name = "Tent",
    module = "pybevy.light",
    extends = PyFalloff,
    frozen
)]
pub struct PyFalloffTent;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyFalloffTent {
    #[classattr]
    const __qualname__: &'static str = "Falloff.Tent";

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str) {
        ("center", "width")
    }

    #[new]
    pub fn new(center: f32, width: f32) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyFalloff(Falloff::Tent { center, width })).add_subclass(Self)
    }

    #[getter]
    pub fn center(slf: PyRef<'_, Self>) -> f32 {
        let base = slf.into_super();
        match base.0 {
            Falloff::Tent { center, .. } => center,
            _ => unreachable!("Falloff.Tent changed discriminant"),
        }
    }

    #[getter]
    pub fn width(slf: PyRef<'_, Self>) -> f32 {
        let base = slf.into_super();
        match base.0 {
            Falloff::Tent { width, .. } => width,
            _ => unreachable!("Falloff.Tent changed discriminant"),
        }
    }
}

pub fn register_falloff_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("Falloff")?;
    base.setattr("Linear", py.get_type::<PyFalloffLinear>())?;
    base.setattr("Exponential", py.get_type::<PyFalloffExponential>())?;
    base.setattr("Tent", py.get_type::<PyFalloffTent>())?;
    Ok(())
}
