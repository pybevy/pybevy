use std::{any::TypeId, sync::Arc, time::Duration};

use bevy::{
    ecs::{component::ComponentId, world::unsafe_world_cell::UnsafeWorldCell},
    prelude::World,
    time::TimeUpdateStrategy,
};
use pybevy_core::{
    PyResource, ResourceBridge, ResourceBridgeRegistration, ResourceStorage, ValidityFlagWithMode,
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
#[pyclass(name = "TimeUpdateStrategy", extends = PyResource, frozen)]
pub struct PyTimeUpdateStrategy {
    pub(crate) storage: ResourceStorage<TimeUpdateStrategy>,
}

impl PyTimeUpdateStrategy {
    fn owned(strategy: TimeUpdateStrategy) -> PyClassInitializer<Self> {
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
    pub fn new() -> PyClassInitializer<Self> {
        Self::owned(TimeUpdateStrategy::Automatic)
    }

    #[staticmethod]
    #[pyo3(name = "Automatic")]
    pub fn automatic(py: Python<'_>) -> PyResult<Py<Self>> {
        Py::new(py, Self::owned(TimeUpdateStrategy::Automatic))
    }

    #[staticmethod]
    #[pyo3(name = "ManualDuration")]
    pub fn manual(py: Python<'_>, duration: Duration) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::owned(TimeUpdateStrategy::ManualDuration(duration)),
        )
    }

    #[staticmethod]
    #[pyo3(name = "FixedTimesteps")]
    pub fn fixed_timesteps(py: Python<'_>, steps: u32) -> PyResult<Py<Self>> {
        Py::new(py, Self::owned(TimeUpdateStrategy::FixedTimesteps(steps)))
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
        Ok(Py::new(py, PyTimeUpdateStrategy::from_borrowed(storage))?.into_any())
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
        Ok(Py::new(py, PyTimeUpdateStrategy::from_borrowed(storage))?.into_any())
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
