use bevy::asset::RecursiveDependencyLoadState;
use pybevy_core::public_error::enum_base_construction;
use pybevy_macros::pyenum;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyDict};

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecursiveDependencyLoadStateValue {
    NotLoaded,
    Loading,
    Loaded,
    // Preserve the opaque native error as owned display text.
    Failed(String),
}

#[pyenum(RecursiveDependencyLoadState, manual)]
#[pyclass(
    name = "RecursiveDependencyLoadState",
    module = "pybevy.assets",
    eq,
    frozen,
    subclass,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyRecursiveDependencyLoadState {
    value: RecursiveDependencyLoadStateValue,
}

impl From<RecursiveDependencyLoadState> for PyRecursiveDependencyLoadState {
    fn from(state: RecursiveDependencyLoadState) -> Self {
        let value = match state {
            RecursiveDependencyLoadState::NotLoaded => RecursiveDependencyLoadStateValue::NotLoaded,
            RecursiveDependencyLoadState::Loading => RecursiveDependencyLoadStateValue::Loading,
            RecursiveDependencyLoadState::Loaded => RecursiveDependencyLoadStateValue::Loaded,
            RecursiveDependencyLoadState::Failed(error) => {
                RecursiveDependencyLoadStateValue::Failed(error.to_string())
            }
        };
        Self { value }
    }
}

#[pymethods]
impl PyRecursiveDependencyLoadState {
    #[new]
    pub fn new() -> PyResult<Self> {
        Err(PyTypeError::new_err(enum_base_construction(
            "RecursiveDependencyLoadState",
        )))
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Py<Self>> {
        self.clone().materialize(py)
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: &Bound<'_, PyDict>) -> PyResult<Py<Self>> {
        self.__copy__(py)
    }

    pub fn is_not_loaded(&self) -> bool {
        matches!(self.value, RecursiveDependencyLoadStateValue::NotLoaded)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.value, RecursiveDependencyLoadStateValue::Loading)
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self.value, RecursiveDependencyLoadStateValue::Loaded)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.value, RecursiveDependencyLoadStateValue::Failed(_))
    }

    fn __repr__(&self) -> String {
        match &self.value {
            RecursiveDependencyLoadStateValue::NotLoaded => {
                "RecursiveDependencyLoadState.NotLoaded()".to_string()
            }
            RecursiveDependencyLoadStateValue::Loading => {
                "RecursiveDependencyLoadState.Loading()".to_string()
            }
            RecursiveDependencyLoadStateValue::Loaded => {
                "RecursiveDependencyLoadState.Loaded()".to_string()
            }
            RecursiveDependencyLoadStateValue::Failed(value) => {
                format!("RecursiveDependencyLoadState.Failed(value={value:?})")
            }
        }
    }

    fn __str__(&self) -> &'static str {
        match self.value {
            RecursiveDependencyLoadStateValue::NotLoaded => "NotLoaded",
            RecursiveDependencyLoadStateValue::Loading => "Loading",
            RecursiveDependencyLoadStateValue::Loaded => "Loaded",
            RecursiveDependencyLoadStateValue::Failed(_) => "Failed",
        }
    }
}

impl PyRecursiveDependencyLoadState {
    pub fn from_state(state: RecursiveDependencyLoadState, py: Python<'_>) -> PyResult<Py<Self>> {
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
            RecursiveDependencyLoadStateValue::NotLoaded => {
                materialize_variant!(PyRecursiveDependencyLoadStateNotLoaded)
            }
            RecursiveDependencyLoadStateValue::Loading => {
                materialize_variant!(PyRecursiveDependencyLoadStateLoading)
            }
            RecursiveDependencyLoadStateValue::Loaded => {
                materialize_variant!(PyRecursiveDependencyLoadStateLoaded)
            }
            RecursiveDependencyLoadStateValue::Failed(_) => {
                materialize_variant!(PyRecursiveDependencyLoadStateFailed)
            }
        }
    }
}

#[pyclass(
    name = "NotLoaded",
    module = "pybevy.assets",
    extends = PyRecursiveDependencyLoadState,
    frozen
)]
pub struct PyRecursiveDependencyLoadStateNotLoaded;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRecursiveDependencyLoadStateNotLoaded {
    #[classattr]
    const __qualname__: &'static str = "RecursiveDependencyLoadState.NotLoaded";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyRecursiveDependencyLoadState {
            value: RecursiveDependencyLoadStateValue::NotLoaded,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Loading",
    module = "pybevy.assets",
    extends = PyRecursiveDependencyLoadState,
    frozen
)]
pub struct PyRecursiveDependencyLoadStateLoading;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRecursiveDependencyLoadStateLoading {
    #[classattr]
    const __qualname__: &'static str = "RecursiveDependencyLoadState.Loading";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyRecursiveDependencyLoadState {
            value: RecursiveDependencyLoadStateValue::Loading,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Loaded",
    module = "pybevy.assets",
    extends = PyRecursiveDependencyLoadState,
    frozen
)]
pub struct PyRecursiveDependencyLoadStateLoaded;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRecursiveDependencyLoadStateLoaded {
    #[classattr]
    const __qualname__: &'static str = "RecursiveDependencyLoadState.Loaded";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyRecursiveDependencyLoadState {
            value: RecursiveDependencyLoadStateValue::Loaded,
        })
        .add_subclass(Self)
    }
}

#[pyclass(
    name = "Failed",
    module = "pybevy.assets",
    extends = PyRecursiveDependencyLoadState,
    frozen
)]
pub struct PyRecursiveDependencyLoadStateFailed;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRecursiveDependencyLoadStateFailed {
    #[classattr]
    const __qualname__: &'static str = "RecursiveDependencyLoadState.Failed";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: String) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyRecursiveDependencyLoadState {
            value: RecursiveDependencyLoadStateValue::Failed(value),
        })
        .add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> String {
        let base = slf.into_super();
        match &base.value {
            RecursiveDependencyLoadStateValue::Failed(value) => value.clone(),
            _ => unreachable!("RecursiveDependencyLoadState.Failed changed discriminant"),
        }
    }
}

pub fn register_recursive_dependency_load_state_variants(
    module: &Bound<'_, PyModule>,
) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("RecursiveDependencyLoadState")?;
    base.setattr(
        "NotLoaded",
        py.get_type::<PyRecursiveDependencyLoadStateNotLoaded>(),
    )?;
    base.setattr(
        "Loading",
        py.get_type::<PyRecursiveDependencyLoadStateLoading>(),
    )?;
    base.setattr(
        "Loaded",
        py.get_type::<PyRecursiveDependencyLoadStateLoaded>(),
    )?;
    base.setattr(
        "Failed",
        py.get_type::<PyRecursiveDependencyLoadStateFailed>(),
    )?;
    Ok(())
}
