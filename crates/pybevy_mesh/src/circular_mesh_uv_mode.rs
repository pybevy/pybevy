use bevy::mesh::CircularMeshUvMode;
use pybevy_core::repr::float_repr;
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyenum(CircularMeshUvMode, manual)]
#[pyclass(
    name = "CircularMeshUvMode",
    module = "pybevy.mesh",
    frozen,
    subclass,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyCircularMeshUvMode(pub(crate) CircularMeshUvMode);

impl From<PyCircularMeshUvMode> for CircularMeshUvMode {
    fn from(value: PyCircularMeshUvMode) -> Self {
        value.0
    }
}

#[pymethods]
impl PyCircularMeshUvMode {
    pub fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        match self.0 {
            CircularMeshUvMode::Mask { angle } => {
                let instance = Py::new(
                    py,
                    PyClassInitializer::from(PyCircularMeshUvMode(CircularMeshUvMode::Mask {
                        angle,
                    }))
                    .add_subclass(PyCircularMeshUvModeMask),
                )?;
                Ok(instance.into_bound(py).into_super().unbind())
            }
            _ => Err(PyTypeError::new_err(
                "unsupported CircularMeshUvMode variant cannot be copied",
            )),
        }
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            CircularMeshUvMode::Mask { angle } => {
                format!("CircularMeshUvMode.Mask(angle={})", float_repr(angle))
            }
            _ => "CircularMeshUvMode(<unsupported>)".to_string(),
        }
    }
}

#[pyclass(name = "Mask", module = "pybevy.mesh", extends = PyCircularMeshUvMode, frozen)]
pub struct PyCircularMeshUvModeMask;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyCircularMeshUvModeMask {
    #[classattr]
    const __qualname__: &'static str = "CircularMeshUvMode.Mask";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("angle",)
    }

    #[new]
    #[pyo3(signature = (*, angle))]
    pub fn new(angle: f32) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyCircularMeshUvMode(CircularMeshUvMode::Mask { angle }))
            .add_subclass(Self)
    }

    #[getter]
    pub fn angle(slf: PyRef<'_, Self>) -> f32 {
        let base = slf.into_super();
        match base.0 {
            CircularMeshUvMode::Mask { angle } => angle,
            _ => unreachable!("CircularMeshUvMode.Mask changed discriminant"),
        }
    }
}

pub fn register_circular_mesh_uv_mode_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("CircularMeshUvMode")?;
    base.setattr("Mask", py.get_type::<PyCircularMeshUvModeMask>())?;
    Ok(())
}
