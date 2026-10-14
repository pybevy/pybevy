use std::{any::Any, mem::transmute, sync::Arc};

use bevy::ecs::{
    component::ComponentId,
    entity::Entity,
    lifecycle::RemovedComponentEntity,
    message::{MessageCursor, Messages},
    world::unsafe_world_cell::UnsafeWorldCell,
};
use pybevy_core::{
    PyEntity, ReaderCursorOp, apply_cursor_op,
    public_error::{
        REMOVED_COMPONENTS_EXPECTS_COMPONENT, REMOVED_COMPONENTS_NOT_RESOURCE,
        injected_system_parameter,
    },
};
use pyo3::{
    IntoPyObjectExt, PyTraverseError, PyVisit,
    exceptions::{PyRuntimeError, PyStopIteration, PyTypeError},
    prelude::*,
    types::PyType,
};
use smallvec::SmallVec;

use super::{
    component_type::{PyComponentType, clone_retained_classes, retain_custom_classes},
    helpers::validity_guard::ValidityFlag,
    messages::CursorStorage,
    resource::PyResource,
};

/// Parsed `RemovedComponents[T]` annotation.
#[pyclass(
    name = "_RemovedComponentsParam",
    module = "pybevy.ecs",
    frozen,
    from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyRemovedComponentsParam {
    pub(crate) component_type: PyComponentType,
    /// Strong references backing Python-owned component pointers.
    pub(crate) retained_types: SmallVec<[Arc<Py<PyType>>; 4]>,
}

impl Clone for PyRemovedComponentsParam {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            component_type: self.component_type,
            retained_types: clone_retained_classes(py, &self.retained_types),
        })
    }
}

#[pymethods]
impl PyRemovedComponentsParam {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained_types {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }
}

/// System parameter yielding the entities whose `T` component was removed or
/// despawned, through a cursor owned by this parameter.
#[pyclass(name = "RemovedComponents", module = "pybevy.ecs", frozen)]
pub struct PyRemovedComponents {
    component_id: ComponentId,
    cursor_storage: CursorStorage,
    world: UnsafeWorldCell<'static>,
    validity: ValidityFlag,
}

// SAFETY: the lifetime-erased world cell is reached only after `validity`
// admits a read on the owning system thread, and only through Bevy's
// metadata accessor `removed_components()`. The cursor sits behind the
// `cursor_storage` mutex.
unsafe impl Send for PyRemovedComponents {}
// SAFETY: as above; callers on another thread or after the system returns
// fail the validity check before the cell is touched.
unsafe impl Sync for PyRemovedComponents {}

impl PyRemovedComponents {
    /// # Safety
    /// `world` must stay live while `validity` is active, and `component_id`
    /// must have been resolved against that same world.
    pub(crate) unsafe fn new(
        component_id: ComponentId,
        cursor_storage: CursorStorage,
        world: UnsafeWorldCell<'_>,
        validity: ValidityFlag,
    ) -> Self {
        // SAFETY: the validity flag fences every use of the erased cell.
        let world = unsafe { transmute::<UnsafeWorldCell<'_>, UnsafeWorldCell<'static>>(world) };
        Self {
            component_id,
            cursor_storage,
            world,
            validity,
        }
    }

    fn with_state<R>(
        &self,
        operation: impl FnOnce(
            Option<&Messages<RemovedComponentEntity>>,
            &mut Option<Box<dyn Any + Send + Sync>>,
        ) -> R,
    ) -> PyResult<R> {
        self.validity
            .check_read()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let messages = self.world.removed_components().get(self.component_id);
        let mut state = self
            .cursor_storage
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(operation(messages, &mut state))
    }

    fn next_removed(&self) -> PyResult<Option<Entity>> {
        self.with_state(|messages, state| {
            let messages = messages?;
            let mut cursor = state
                .as_ref()
                .and_then(|state| state.downcast_ref::<MessageCursor<RemovedComponentEntity>>())
                .cloned()
                .unwrap_or_else(|| messages.get_cursor());
            let next = cursor.read(messages).next().cloned().map(Entity::from);
            *state = Some(Box::new(cursor));
            next
        })
    }

    fn share(&self) -> Self {
        Self {
            component_id: self.component_id,
            cursor_storage: Arc::clone(&self.cursor_storage),
            world: self.world,
            validity: self.validity.clone(),
        }
    }
}

#[pymethods]
impl PyRemovedComponents {
    #[new]
    fn new_injected_only() -> PyResult<Self> {
        Err(PyTypeError::new_err(injected_system_parameter(
            "RemovedComponents",
            "removed: RemovedComponents[MyComponent]",
        )))
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = key.py();
        let component_class = key
            .cast::<PyType>()
            .map_err(|_| PyTypeError::new_err(REMOVED_COMPONENTS_EXPECTS_COMPONENT))?;
        if component_class.is_subclass_of::<PyResource>()? {
            return Err(PyTypeError::new_err(REMOVED_COMPONENTS_NOT_RESOURCE));
        }
        let component_type = PyComponentType::try_from_py_type(component_class, py)?;
        PyRemovedComponentsParam {
            component_type,
            retained_types: retain_custom_classes(py, [component_type]),
        }
        .into_py_any(py)
    }

    /// Mark every currently retained removal read for this parameter only.
    pub fn clear(&self) -> PyResult<()> {
        self.with_state(|messages, state| {
            apply_cursor_op(messages, state, ReaderCursorOp::Clear);
        })
    }

    pub fn is_empty(&self) -> PyResult<bool> {
        Ok(self.len()? == 0)
    }

    pub fn len(&self) -> PyResult<usize> {
        self.with_state(|messages, state| {
            apply_cursor_op(messages, state, ReaderCursorOp::UnreadLen)
        })
    }

    pub fn read(&self) -> PyResult<PyRemovedComponentsIter> {
        self.validity
            .check_read()
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(PyRemovedComponentsIter {
            source: self.share(),
        })
    }

    pub fn __iter__(&self) -> PyResult<PyRemovedComponentsIter> {
        self.read()
    }
}

/// Iterator over unread removals; each yielded entity advances the cursor.
#[pyclass(name = "RemovedComponentsIter", module = "pybevy.ecs")]
pub(crate) struct PyRemovedComponentsIter {
    source: PyRemovedComponents,
}

#[pymethods]
impl PyRemovedComponentsIter {
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(&self) -> PyResult<PyEntity> {
        match self.source.next_removed()? {
            Some(entity) => Ok(PyEntity::from(entity)),
            None => Err(PyStopIteration::new_err(())),
        }
    }
}
