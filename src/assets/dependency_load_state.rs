use bevy::asset::DependencyLoadState;
use pybevy_core::public_error::enum_base_construction;
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyDict};

#[derive(Debug, Clone, PartialEq, Eq)]
enum DependencyLoadStateValue {
    NotLoaded,
    Loading,
    Loaded,
    // Preserve the opaque native error as owned display text.
    Failed(String),
}

#[pyenum(DependencyLoadState, manual)]
#[pyclass(
    name = "DependencyLoadState",
    module = "pybevy.assets",
    eq,
    frozen,
    subclass,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyDependencyLoadState {
    value: DependencyLoadStateValue,
}

impl From<DependencyLoadState> for PyDependencyLoadState {
    fn from(state: DependencyLoadState) -> Self {
        let value = match state {
            DependencyLoadState::NotLoaded => DependencyLoadStateValue::NotLoaded,
            DependencyLoadState::Loading => DependencyLoadStateValue::Loading,
            DependencyLoadState::Loaded => DependencyLoadStateValue::Loaded,
            DependencyLoadState::Failed(error) => {
                DependencyLoadStateValue::Failed(error.to_string())
            }
        };
        Self { value }
    }
}

#[pymethods]
impl PyDependencyLoadState {
    #[new]
    pub fn new() -> PyResult<Self> {
        Err(PyTypeError::new_err(enum_base_construction(
            "DependencyLoadState",
        )))
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.clone().materialize(py)
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: &Bound<'_, PyDict>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    pub fn is_not_loaded(&self) -> bool {
        matches!(self.value, DependencyLoadStateValue::NotLoaded)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.value, DependencyLoadStateValue::Loading)
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self.value, DependencyLoadStateValue::Loaded)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.value, DependencyLoadStateValue::Failed(_))
    }

    fn __repr__(&self) -> String {
        match &self.value {
            DependencyLoadStateValue::NotLoaded => "DependencyLoadState.NotLoaded()".to_string(),
            DependencyLoadStateValue::Loading => "DependencyLoadState.Loading()".to_string(),
            DependencyLoadStateValue::Loaded => "DependencyLoadState.Loaded()".to_string(),
            DependencyLoadStateValue::Failed(value) => {
                format!("DependencyLoadState.Failed(value={value:?})")
            }
        }
    }

    fn __str__(&self) -> &'static str {
        match self.value {
            DependencyLoadStateValue::NotLoaded => "NotLoaded",
            DependencyLoadStateValue::Loading => "Loading",
            DependencyLoadStateValue::Loaded => "Loaded",
            DependencyLoadStateValue::Failed(_) => "Failed",
        }
    }
}

impl PyDependencyLoadState {
    pub fn from_state(state: DependencyLoadState, py: Python<'_>) -> PyResult<Py<Self>> {
        Self::from(state).materialize(py)
    }

    fn materialize(self, py: Python<'_>) -> PyResult<Py<Self>> {
        let state = self;

        macro_rules! materialize_variant {
            ($variant:expr) => {{
                let value = Py::new(py, PyClassInitializer::from(state).add_subclass($variant))?;
                Ok(value.into_bound(py).into_super().unbind())
            }};
        }

        match &state.value {
            DependencyLoadStateValue::NotLoaded => {
                materialize_variant!(PyDependencyLoadStateNotLoaded)
            }
            DependencyLoadStateValue::Loading => {
                materialize_variant!(PyDependencyLoadStateLoading)
            }
            DependencyLoadStateValue::Loaded => {
                materialize_variant!(PyDependencyLoadStateLoaded)
            }
            DependencyLoadStateValue::Failed(_) => {
                materialize_variant!(PyDependencyLoadStateFailed)
            }
        }
    }
}

#[pyclass(
    name = "NotLoaded",
    module = "pybevy.assets",
    extends = PyDependencyLoadState,
    frozen
)]
pub struct PyDependencyLoadStateNotLoaded;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyDependencyLoadStateNotLoaded {
    #[classattr]
    const __qualname__: &'static str = "DependencyLoadState.NotLoaded";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyDependencyLoadState {
            value: DependencyLoadStateValue::NotLoaded,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Loading",
    module = "pybevy.assets",
    extends = PyDependencyLoadState,
    frozen
)]
pub struct PyDependencyLoadStateLoading;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyDependencyLoadStateLoading {
    #[classattr]
    const __qualname__: &'static str = "DependencyLoadState.Loading";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyDependencyLoadState {
            value: DependencyLoadStateValue::Loading,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Loaded",
    module = "pybevy.assets",
    extends = PyDependencyLoadState,
    frozen
)]
pub struct PyDependencyLoadStateLoaded;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyDependencyLoadStateLoaded {
    #[classattr]
    const __qualname__: &'static str = "DependencyLoadState.Loaded";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyDependencyLoadState {
            value: DependencyLoadStateValue::Loaded,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Failed",
    module = "pybevy.assets",
    extends = PyDependencyLoadState,
    frozen
)]
pub struct PyDependencyLoadStateFailed;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyDependencyLoadStateFailed {
    #[classattr]
    const __qualname__: &'static str = "DependencyLoadState.Failed";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: String) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyDependencyLoadState {
            value: DependencyLoadStateValue::Failed(value),
        })
        .add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> String {
        let base = slf.into_super();
        match &base.value {
            DependencyLoadStateValue::Failed(value) => value.clone(),
            _ => unreachable!("DependencyLoadState.Failed changed discriminant"),
        }
    }
}

pub fn register_dependency_load_state_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("DependencyLoadState")?;
    base.setattr("NotLoaded", py.get_type::<PyDependencyLoadStateNotLoaded>())?;
    base.setattr("Loading", py.get_type::<PyDependencyLoadStateLoading>())?;
    base.setattr("Loaded", py.get_type::<PyDependencyLoadStateLoaded>())?;
    base.setattr("Failed", py.get_type::<PyDependencyLoadStateFailed>())?;
    Ok(())
}
