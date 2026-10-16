use pybevy_core::public_error;
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyByteArray, PyBytes, PyString, PyTuple},
};

use crate::ecs::{
    conditional_system::PyConditionalSystem,
    decorated_callable::{CallableKind, unwrap_decorated},
    system_config::{PySystemConfig, PySystemSetConfig, system_set_value},
};

#[pyclass(name = "ChainedSystems", module = "pybevy.app", from_py_object)]
pub struct PyChainedSystems {
    pub systems: Py<PyTuple>,
}

impl Clone for PyChainedSystems {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            systems: self.systems.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyChainedSystems {
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.systems)
    }

    #[new]
    #[pyo3(signature = (*systems))]
    pub fn py_new(systems: Bound<'_, PyTuple>) -> PyResult<Self> {
        Ok(Self {
            systems: normalize_chain_members(&systems)?.unbind(),
        })
    }
}

/// Replace `@system` members by their functions; configs and nested chains stay.
fn normalize_chain_members<'py>(systems: &Bound<'py, PyTuple>) -> PyResult<Bound<'py, PyTuple>> {
    let mut members = Vec::with_capacity(systems.len());
    for member in systems.iter() {
        if is_system_group(&member) {
            return Err(PyTypeError::new_err(public_error::CHAIN_GROUP_MEMBER));
        }
        members.push(normalize_system_unit(&member, "chain()")?);
    }
    PyTuple::new(systems.py(), members)
}

/// One `add_systems` unit with a `@system` function unwrapped to the function.
///
/// Configs, conditional systems, and chains already hold unwrapped functions.
pub(crate) fn normalize_system_unit<'py>(
    value: &Bound<'py, PyAny>,
    entry: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if value.is_instance_of::<PySystemConfig>()
        || value.is_instance_of::<PyConditionalSystem>()
        || value.is_instance_of::<PyChainedSystems>()
    {
        return Ok(value.clone());
    }
    unwrap_decorated(value, CallableKind::System, entry)
}

/// A sequence of system sets configured with Bevy's chained ordering semantics.
#[pyclass(name = "ChainedSystemSets", module = "pybevy.app", from_py_object)]
pub struct PyChainedSystemSets {
    pub sets: Py<PyTuple>,
}

impl Clone for PyChainedSystemSets {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            sets: self.sets.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyChainedSystemSets {
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.sets)
    }

    #[new]
    #[pyo3(signature = (*sets))]
    pub fn new(sets: Bound<'_, PyTuple>) -> Self {
        Self {
            sets: sets.unbind(),
        }
    }
}

#[pyfunction]
#[pyo3(signature = (*systems))]
pub fn chain(py: Python<'_>, systems: Bound<'_, PyTuple>) -> PyResult<Py<PyAny>> {
    if systems.is_empty() {
        return Err(PyValueError::new_err(
            "chain() requires at least one system",
        ));
    }

    let mut set_count = 0;
    for value in systems.iter() {
        if is_system_group(&value) {
            return Err(PyTypeError::new_err(public_error::CHAIN_GROUP_MEMBER));
        }
        if value.extract::<PySystemSetConfig>().is_ok() || system_set_value(&value)?.is_some() {
            set_count += 1;
        }
    }

    if set_count == systems.len() {
        return Ok(Py::new(py, PyChainedSystemSets::new(systems))?.into_any());
    }
    if set_count != 0 {
        return Err(PyTypeError::new_err(
            "chain() cannot mix system sets with systems",
        ));
    }

    Ok(Py::new(py, PyChainedSystems::py_new(systems)?)?.into_any())
}

/// Whether `value` is an unordered group of systems, such as a tuple.
fn is_system_group(value: &Bound<'_, PyAny>) -> bool {
    !(value.is_callable()
        || value.is_instance_of::<PyChainedSystems>()
        || value.is_instance_of::<PyChainedSystemSets>()
        || value.is_instance_of::<PySystemConfig>()
        || value.is_instance_of::<PyConditionalSystem>()
        || value.is_instance_of::<PyString>()
        || value.is_instance_of::<PyBytes>()
        || value.is_instance_of::<PyByteArray>())
        && value.try_iter().is_ok()
}

/// Expand `add_systems` arguments into registration units.
///
/// A unit is one system or one chain. Groups such as tuples nest freely and
/// impose no order between their members, matching Bevy's tuple configs.
pub(crate) fn flatten_system_units<'py>(
    value: &Bound<'py, PyAny>,
    units: &mut Vec<Bound<'py, PyAny>>,
) -> PyResult<()> {
    if is_system_group(value) {
        for member in value.try_iter()? {
            flatten_system_units(&member?, units)?;
        }
    } else {
        units.push(value.clone());
    }
    Ok(())
}

/// The systems of a chain in execution order, splicing nested chains.
///
/// Returns `None` when `value` is not a chain.
pub(crate) fn chain_members<'py>(
    value: &Bound<'py, PyAny>,
) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    let Ok(chained) = value.cast::<PyChainedSystems>() else {
        return Ok(None);
    };
    let mut members = Vec::new();
    collect_chain_members(chained, &mut members)?;
    if members.is_empty() {
        return Err(PyRuntimeError::new_err(public_error::EMPTY_CHAINED_SYSTEMS));
    }
    Ok(Some(members))
}

fn collect_chain_members<'py>(
    chained: &Bound<'py, PyChainedSystems>,
    members: &mut Vec<Bound<'py, PyAny>>,
) -> PyResult<()> {
    let systems = chained.borrow().systems.clone_ref(chained.py());
    for member in systems.bind(chained.py()).iter() {
        if let Ok(nested) = member.cast::<PyChainedSystems>() {
            collect_chain_members(nested, members)?;
        } else if is_system_group(&member) {
            return Err(PyTypeError::new_err(public_error::CHAIN_GROUP_MEMBER));
        } else {
            members.push(member);
        }
    }
    Ok(())
}
