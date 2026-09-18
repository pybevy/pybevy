use bevy::asset::LoadState;
use pybevy_core::public_error::enum_base_construction;
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyDict};

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoadStateValue {
    NotLoaded,
    Loading,
    Loaded,
    // Preserve the opaque native error as owned display text.
    Failed(String),
}

#[pyenum(LoadState, manual)]
#[pyclass(
    name = "LoadState",
    module = "pybevy.assets",
    eq,
    frozen,
    subclass,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyLoadState {
    value: LoadStateValue,
}

impl From<LoadState> for PyLoadState {
    fn from(state: LoadState) -> Self {
        let value = match state {
            LoadState::NotLoaded => LoadStateValue::NotLoaded,
            LoadState::Loading => LoadStateValue::Loading,
            LoadState::Loaded => LoadStateValue::Loaded,
            LoadState::Failed(error) => LoadStateValue::Failed(error.to_string()),
        };
        Self { value }
    }
}

#[pymethods]
impl PyLoadState {
    #[new]
    pub fn new() -> PyResult<Self> {
        Err(PyTypeError::new_err(enum_base_construction("LoadState")))
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.clone().materialize(py)
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: &Bound<'_, PyDict>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    pub fn is_not_loaded(&self) -> bool {
        matches!(self.value, LoadStateValue::NotLoaded)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.value, LoadStateValue::Loading)
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self.value, LoadStateValue::Loaded)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.value, LoadStateValue::Failed(_))
    }

    fn __repr__(&self) -> String {
        match &self.value {
            LoadStateValue::NotLoaded => "LoadState.NotLoaded()".to_string(),
            LoadStateValue::Loading => "LoadState.Loading()".to_string(),
            LoadStateValue::Loaded => "LoadState.Loaded()".to_string(),
            LoadStateValue::Failed(value) => {
                format!("LoadState.Failed(value={value:?})")
            }
        }
    }

    fn __str__(&self) -> &'static str {
        match self.value {
            LoadStateValue::NotLoaded => "NotLoaded",
            LoadStateValue::Loading => "Loading",
            LoadStateValue::Loaded => "Loaded",
            LoadStateValue::Failed(_) => "Failed",
        }
    }
}

impl PyLoadState {
    pub fn from_state(state: LoadState, py: Python<'_>) -> PyResult<Py<Self>> {
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
            LoadStateValue::NotLoaded => materialize_variant!(PyLoadStateNotLoaded),
            LoadStateValue::Loading => materialize_variant!(PyLoadStateLoading),
            LoadStateValue::Loaded => materialize_variant!(PyLoadStateLoaded),
            LoadStateValue::Failed(_) => materialize_variant!(PyLoadStateFailed),
        }
    }
}

#[pyclass(
    name = "NotLoaded",
    module = "pybevy.assets",
    extends = PyLoadState,
    frozen
)]
pub struct PyLoadStateNotLoaded;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyLoadStateNotLoaded {
    #[classattr]
    const __qualname__: &'static str = "LoadState.NotLoaded";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyLoadState {
            value: LoadStateValue::NotLoaded,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Loading",
    module = "pybevy.assets",
    extends = PyLoadState,
    frozen
)]
pub struct PyLoadStateLoading;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyLoadStateLoading {
    #[classattr]
    const __qualname__: &'static str = "LoadState.Loading";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyLoadState {
            value: LoadStateValue::Loading,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Loaded",
    module = "pybevy.assets",
    extends = PyLoadState,
    frozen
)]
pub struct PyLoadStateLoaded;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyLoadStateLoaded {
    #[classattr]
    const __qualname__: &'static str = "LoadState.Loaded";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyLoadState {
            value: LoadStateValue::Loaded,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Failed",
    module = "pybevy.assets",
    extends = PyLoadState,
    frozen
)]
pub struct PyLoadStateFailed;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyLoadStateFailed {
    #[classattr]
    const __qualname__: &'static str = "LoadState.Failed";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: String) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyLoadState {
            value: LoadStateValue::Failed(value),
        })
        .add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> String {
        let base = slf.into_super();
        match &base.value {
            LoadStateValue::Failed(value) => value.clone(),
            _ => unreachable!("LoadState.Failed changed discriminant"),
        }
    }
}

pub fn register_load_state_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("LoadState")?;
    base.setattr("NotLoaded", py.get_type::<PyLoadStateNotLoaded>())?;
    base.setattr("Loading", py.get_type::<PyLoadStateLoading>())?;
    base.setattr("Loaded", py.get_type::<PyLoadStateLoaded>())?;
    base.setattr("Failed", py.get_type::<PyLoadStateFailed>())?;
    Ok(())
}
