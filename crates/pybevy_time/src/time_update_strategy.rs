use std::{any::TypeId, sync::Arc, time::Duration};

use bevy::{
    ecs::{component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell},
    prelude::World,
    time::TimeUpdateStrategy,
};
use pybevy_core::{
    PyResource, ResourceBridge, ResourceBridgeRegistration, ResourceStorage, ValidityFlagWithMode,
    registry::global_registry,
};
use pybevy_macros::pyresource;
use pyo3::{
    PyTypeInfo,
    exceptions::{PyNotImplementedError, PyRuntimeError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyType, PyTypeMethods},
};

#[pyresource(TimeUpdateStrategy, no_clone)]
#[pyclass(
    name = "TimeUpdateStrategy",
    module = "pybevy.time",
    extends = PyResource,
    frozen,
    subclass
)]
pub struct PyTimeUpdateStrategy {
    pub(crate) storage: ResourceStorage<TimeUpdateStrategy>,
}

impl PyTimeUpdateStrategy {
    fn initializer(strategy: TimeUpdateStrategy) -> PyClassInitializer<Self> {
        (
            Self {
                storage: ResourceStorage::owned(strategy),
            },
            PyResource,
        )
            .into()
    }

    fn clone_supported(strategy: &TimeUpdateStrategy) -> PyResult<TimeUpdateStrategy> {
        match strategy {
            TimeUpdateStrategy::Automatic => Ok(TimeUpdateStrategy::Automatic),
            TimeUpdateStrategy::ManualDuration(duration) => {
                Ok(TimeUpdateStrategy::ManualDuration(*duration))
            }
            TimeUpdateStrategy::FixedTimesteps(steps) => {
                Ok(TimeUpdateStrategy::FixedTimesteps(*steps))
            }
            TimeUpdateStrategy::ManualInstant(_) => Err(PyNotImplementedError::new_err(
                "TimeUpdateStrategy.ManualInstant is not exposed because Bevy's Instant has no Python representation",
            )),
        }
    }
}

#[pymethods]
impl PyTimeUpdateStrategy {
    #[new]
    pub fn new() -> PyResult<PyClassInitializer<Self>> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "TimeUpdateStrategy is an enum base; construct a nested variant",
        ))
    }

    fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()? {
            TimeUpdateStrategy::Automatic => Ok("TimeUpdateStrategy.Automatic()".to_string()),
            TimeUpdateStrategy::ManualDuration(duration) => Ok(format!(
                "TimeUpdateStrategy.ManualDuration(timedelta(seconds={}))",
                duration.as_secs_f64()
            )),
            TimeUpdateStrategy::FixedTimesteps(steps) => {
                Ok(format!("TimeUpdateStrategy.FixedTimesteps({steps})"))
            }
            TimeUpdateStrategy::ManualInstant(_) => {
                Ok("TimeUpdateStrategy.<unsupported ManualInstant>".to_string())
            }
        }
    }
}

#[pyclass(
    name = "Automatic",
    module = "pybevy.time",
    extends = PyTimeUpdateStrategy,
    frozen
)]
pub struct PyTimeUpdateStrategyAutomatic;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyTimeUpdateStrategyAutomatic {
    #[classattr]
    const __qualname__: &'static str = "TimeUpdateStrategy.Automatic";

    #[classattr]
    const __match_args__: () = ();

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        PyTimeUpdateStrategy::initializer(TimeUpdateStrategy::Automatic).add_subclass(Self)
    }
}

#[pyclass(
    name = "ManualDuration",
    module = "pybevy.time",
    extends = PyTimeUpdateStrategy,
    frozen
)]
pub struct PyTimeUpdateStrategyManualDuration;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyTimeUpdateStrategyManualDuration {
    #[classattr]
    const __qualname__: &'static str = "TimeUpdateStrategy.ManualDuration";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("duration",)
    }

    #[new]
    pub fn new(duration: Duration) -> PyClassInitializer<Self> {
        PyTimeUpdateStrategy::initializer(TimeUpdateStrategy::ManualDuration(duration))
            .add_subclass(Self)
    }

    #[getter]
    pub fn duration(slf: PyRef<'_, Self>) -> PyResult<Duration> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            TimeUpdateStrategy::ManualDuration(duration) => Ok(*duration),
            _ => unreachable!("TimeUpdateStrategy.ManualDuration changed discriminant"),
        }
    }
}

#[pyclass(
    name = "FixedTimesteps",
    module = "pybevy.time",
    extends = PyTimeUpdateStrategy,
    frozen
)]
pub struct PyTimeUpdateStrategyFixedTimesteps;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyTimeUpdateStrategyFixedTimesteps {
    #[classattr]
    const __qualname__: &'static str = "TimeUpdateStrategy.FixedTimesteps";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("steps",)
    }

    #[new]
    pub fn new(steps: u32) -> PyClassInitializer<Self> {
        PyTimeUpdateStrategy::initializer(TimeUpdateStrategy::FixedTimesteps(steps))
            .add_subclass(Self)
    }

    #[getter]
    pub fn steps(slf: PyRef<'_, Self>) -> PyResult<u32> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            TimeUpdateStrategy::FixedTimesteps(steps) => Ok(*steps),
            _ => unreachable!("TimeUpdateStrategy.FixedTimesteps changed discriminant"),
        }
    }
}

fn materialize_time_update_strategy(
    py: Python<'_>,
    storage: ResourceStorage<TimeUpdateStrategy>,
) -> PyResult<Py<PyAny>> {
    enum Variant {
        Automatic,
        ManualDuration,
        FixedTimesteps,
        UnsupportedManualInstant,
    }

    let variant = match storage.as_ref()? {
        TimeUpdateStrategy::Automatic => Variant::Automatic,
        TimeUpdateStrategy::ManualDuration(_) => Variant::ManualDuration,
        TimeUpdateStrategy::FixedTimesteps(_) => Variant::FixedTimesteps,
        TimeUpdateStrategy::ManualInstant(_) => Variant::UnsupportedManualInstant,
    };
    let base = PyClassInitializer::from(PyResource).add_subclass(PyTimeUpdateStrategy { storage });

    match variant {
        Variant::Automatic => {
            Ok(Py::new(py, base.add_subclass(PyTimeUpdateStrategyAutomatic))?.into_any())
        }
        Variant::ManualDuration => {
            Ok(Py::new(py, base.add_subclass(PyTimeUpdateStrategyManualDuration))?.into_any())
        }
        Variant::FixedTimesteps => {
            Ok(Py::new(py, base.add_subclass(PyTimeUpdateStrategyFixedTimesteps))?.into_any())
        }
        Variant::UnsupportedManualInstant => Ok(Py::new(py, base)?.into_any()),
    }
}

pub fn register_time_update_strategy_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("TimeUpdateStrategy")?;
    base.setattr("Automatic", py.get_type::<PyTimeUpdateStrategyAutomatic>())?;
    base.setattr(
        "ManualDuration",
        py.get_type::<PyTimeUpdateStrategyManualDuration>(),
    )?;
    base.setattr(
        "FixedTimesteps",
        py.get_type::<PyTimeUpdateStrategyFixedTimesteps>(),
    )?;

    let canonical = PyTimeUpdateStrategy::type_object_raw(py);
    for alias in [
        PyTimeUpdateStrategyAutomatic::type_object_raw(py),
        PyTimeUpdateStrategyManualDuration::type_object_raw(py),
        PyTimeUpdateStrategyFixedTimesteps::type_object_raw(py),
    ] {
        if !global_registry::register_resource_bridge_alias(alias, canonical) {
            return Err(PyRuntimeError::new_err(
                "TimeUpdateStrategy bridge was not registered before its variants",
            ));
        }
    }
    Ok(())
}

pub struct TimeUpdateStrategyBridge;

impl TimeUpdateStrategyBridge {
    fn wrap(
        strategy: &TimeUpdateStrategy,
        validity: ValidityFlagWithMode,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let ptr = strategy as *const TimeUpdateStrategy as *mut TimeUpdateStrategy;
        // SAFETY: the pointer comes from a live World resource borrow and the
        // caller-provided validity flag fences every dereference by the wrapper.
        let storage = unsafe { ResourceStorage::borrowed(ptr, validity) };
        materialize_time_update_strategy(py, storage)
    }
}

impl ResourceBridge for TimeUpdateStrategyBridge {
    fn bevy_type_id(&self) -> TypeId {
        TypeId::of::<TimeUpdateStrategy>()
    }

    fn py_type_ptr(&self) -> *const PyTypeObject {
        Python::attach(|py| PyTimeUpdateStrategy::type_object(py).as_type_ptr())
    }

    fn py_type<'py>(&self, py: Python<'py>) -> Bound<'py, PyType> {
        PyTimeUpdateStrategy::type_object(py)
    }

    fn name(&self) -> &'static str {
        "TimeUpdateStrategy"
    }

    fn get(
        &self,
        world: &World,
        validity: ValidityFlagWithMode,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let strategy = world
            .get_resource::<TimeUpdateStrategy>()
            .ok_or_else(|| PyRuntimeError::new_err("TimeUpdateStrategy resource not found"))?;
        Self::wrap(strategy, validity, py)
    }

    fn get_mut(
        &self,
        world: &mut World,
        validity: ValidityFlagWithMode,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let strategy = world
            .get_resource_mut::<TimeUpdateStrategy>()
            .ok_or_else(|| PyRuntimeError::new_err("TimeUpdateStrategy resource not found"))?;
        let ptr = strategy.into_inner() as *mut TimeUpdateStrategy;
        // SAFETY: the pointer comes from an exclusive World resource borrow and
        // the write-mode validity flag fences every dereference by the wrapper.
        let storage = unsafe { ResourceStorage::borrowed(ptr, validity) };
        materialize_time_update_strategy(py, storage)
    }

    unsafe fn get_from_cell(
        &self,
        cell: UnsafeWorldCell<'_>,
        validity: ValidityFlagWithMode,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        // SAFETY: the caller declared read access to TimeUpdateStrategy during
        // system initialization, so Bevy excludes a concurrent writer.
        let strategy = unsafe { cell.get_resource::<TimeUpdateStrategy>() }
            .ok_or_else(|| PyRuntimeError::new_err("TimeUpdateStrategy resource not found"))?;
        Self::wrap(strategy, validity, py)
    }

    unsafe fn get_mut_from_cell(
        &self,
        cell: UnsafeWorldCell<'_>,
        validity: ValidityFlagWithMode,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        // SAFETY: the caller declared write access to TimeUpdateStrategy during
        // system initialization, so Bevy excludes every concurrent access.
        let strategy = unsafe { cell.get_resource_mut::<TimeUpdateStrategy>() }
            .ok_or_else(|| PyRuntimeError::new_err("TimeUpdateStrategy resource not found"))?;
        let ptr = strategy.into_inner() as *mut TimeUpdateStrategy;
        // SAFETY: ptr comes from the unique resource borrow above and shares the
        // run-scoped validity flag supplied by the system executor.
        let storage = unsafe { ResourceStorage::borrowed(ptr, validity) };
        Ok(Py::new(py, PyTimeUpdateStrategy::from_borrowed(storage))?.into_any())
    }

    fn insert(&self, world: &mut World, resource: &Bound<'_, PyAny>) -> PyResult<()> {
        let strategy = resource.extract::<PyRef<'_, PyTimeUpdateStrategy>>()?;
        let strategy_ref = PyTimeUpdateStrategy::as_ref(&strategy)?;
        world.insert_resource(PyTimeUpdateStrategy::clone_supported(strategy_ref)?);
        Ok(())
    }

    fn remove(&self, world: &mut World) {
        world.remove_resource::<TimeUpdateStrategy>();
    }

    fn contains_in_world(&self, world: &World) -> bool {
        world.contains_resource::<TimeUpdateStrategy>()
    }

    fn resource_id(&self, world: &World) -> Option<ComponentId> {
        world.components().component_id::<TimeUpdateStrategy>()
    }

    fn register_resource_id(&self, world: &mut World) -> ComponentId {
        world.register_component::<TimeUpdateStrategy>()
    }

    fn reset_to_default(&self, world: &mut World) -> bool {
        world.insert_resource(TimeUpdateStrategy::Automatic);
        true
    }
}

pybevy_core::inventory::submit!(ResourceBridgeRegistration {
    create: || Arc::new(TimeUpdateStrategyBridge),
});
