use std::sync::Arc;

use bevy::ecs::{entity::Entity, ptr::OwningPtr, world::World};
use pybevy_core::{BatchComponent, registry::global_registry};
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyTuple, PyType},
};

use super::{
    component_layout::{ComponentLayout, ComponentStorageType, serialize_to_wrapper},
    component_type::{PyComponentType, register_custom_component},
    component_wrapper::*,
    helpers::type_utils::get_python_type_name,
};

/// Enum to represent either a batch component or a uniform component
enum ComponentData {
    Batch {
        bridge: Arc<dyn BatchComponent>,
        py_obj: Py<PyAny>,
    },
    Uniform(Py<PyAny>, PyComponentType),
}

/// Command to spawn multiple entities with batch and uniform components
pub struct SpawnBatchCommand {
    components: Vec<ComponentData>,
    explicit_count: Option<usize>,
}

impl SpawnBatchCommand {
    /// Create a new spawn batch command from Python components
    pub fn new(
        py: Python,
        components: &Bound<'_, PyTuple>,
        count: Option<usize>,
    ) -> PyResult<Self> {
        let mut component_data = Vec::new();

        for component in components.iter() {
            // Check if it's a registered batch component
            let type_ptr = component.get_type().as_type_ptr();
            if let Some(bridge) = global_registry::get_batch_bridge_by_py_type(type_ptr) {
                component_data.push(ComponentData::Batch {
                    bridge,
                    py_obj: component.clone().unbind(),
                });
            } else {
                // It's a uniform component - determine its type
                let component_type = PyComponentType::try_from((&component.get_type(), py))?;
                component_data.push(ComponentData::Uniform(
                    component.clone().unbind(),
                    component_type,
                ));
            }
        }

        Ok(SpawnBatchCommand {
            components: component_data,
            explicit_count: count,
        })
    }

    /// Apply the spawn batch command to the world
    pub fn apply(self, world: &mut World) -> PyResult<Vec<Entity>> {
        Python::attach(|py| {
            // Determine spawn count
            let spawn_count = if let Some(count) = self.explicit_count {
                count
            } else {
                let mut found_count = None;
                for comp in &self.components {
                    if let ComponentData::Batch { bridge, py_obj } = comp {
                        found_count = Some(bridge.count(py, py_obj.bind(py))?);
                        break;
                    }
                }
                match found_count {
                    Some(count) => count,
                    None => {
                        return Err(PyValueError::new_err(
                            "spawn_batch requires either a 'count' parameter or at least one batch component",
                        ));
                    }
                }
            };

            // Validate that all batch components have matching counts
            for comp in &self.components {
                if let ComponentData::Batch { bridge, py_obj } = comp {
                    let actual = bridge.count(py, py_obj.bind(py))?;
                    if actual != spawn_count {
                        return Err(PyValueError::new_err(format!(
                            "{} has {} elements but expected {}",
                            bridge.name(),
                            actual,
                            spawn_count,
                        )));
                    }
                }
            }

            // Phase 1: Spawn all entities.
            let mut entities = Vec::with_capacity(spawn_count);
            for _ in 0..spawn_count {
                entities.push(world.spawn_empty().id());
            }

            // Phase 2: Bulk-insert batch components.
            for comp in &self.components {
                if let ComponentData::Batch { bridge, py_obj } = comp {
                    bridge.insert_bulk(py, py_obj.bind(py), &entities, world)?;
                }
            }

            // Phase 3: Bulk-insert uniform components.
            // Each component is resolved once, then inserted into all entities.
            for comp in &self.components {
                if let ComponentData::Uniform(py_obj, comp_type) = comp {
                    insert_uniform_bulk(py, py_obj.bind(py), comp_type, &entities, world)?;
                }
            }

            Ok(entities)
        })
    }
}

/// Wrapper to make PyTypeObject pointer Send-safe
#[derive(Copy, Clone)]
struct SendTypePtr(usize);

impl SendTypePtr {
    fn new(ptr: *const PyTypeObject) -> Self {
        SendTypePtr(ptr as usize)
    }

    fn as_ptr(&self) -> *const PyTypeObject {
        self.0 as *const PyTypeObject
    }
}

unsafe impl Send for SendTypePtr {}
unsafe impl Sync for SendTypePtr {}

/// Bulk-insert a uniform component into all entities.
fn insert_uniform_bulk(
    py: Python,
    component: &Bound<'_, PyAny>,
    comp_type: &PyComponentType,
    entities: &[Entity],
    world: &mut World,
) -> PyResult<()> {
    match comp_type {
        PyComponentType::Dynamic(type_ptr) => {
            if let Some(bridge) = global_registry::get_bridge_by_py_type(*type_ptr) {
                bridge.insert_bulk_uniform(component, entities, world)?;
            } else {
                return Err(PyRuntimeError::new_err(format!(
                    "No component bridge registered for Dynamic type {:?}",
                    type_ptr
                )));
            }
        }
        PyComponentType::Custom(raw_type_ptr) => {
            let type_ptr = SendTypePtr::new(*raw_type_ptr);
            let name = get_python_type_name(py, type_ptr.as_ptr());
            let component_id = register_custom_component(world, type_ptr.as_ptr(), name);

            // Determine storage type and pre-serialize for wrapper storage
            let py_type = unsafe {
                Bound::from_borrowed_ptr(py, type_ptr.as_ptr() as *mut pyo3::ffi::PyObject)
            };

            let wrapper_bytes = if let Ok(cls) = py_type.cast::<PyType>() {
                let storage_type = ComponentStorageType::from_python_class(cls)
                    .unwrap_or(ComponentStorageType::PyObject);

                match storage_type {
                    ComponentStorageType::Wrapper(_) => {
                        if let Ok(layout) = ComponentLayout::from_annotations(cls) {
                            serialize_to_wrapper(component, &layout).ok()
                        } else {
                            None
                        }
                    }
                    ComponentStorageType::PyObject => None,
                }
            } else {
                None
            };

            if let Some(bytes) = wrapper_bytes {
                // Wrapper storage: insert properly serialized ComponentWrapper
                let wrapper_size =
                    WrapperSize::for_size(bytes.len()).expect("Wrapper size should be valid");

                macro_rules! insert_wrapper {
                    ($size:expr, $wrapper_type:ty) => {
                        if wrapper_size == $size {
                            for &entity_id in entities {
                                let mut wrapper = <$wrapper_type>::default();
                                wrapper.data[..bytes.len()].copy_from_slice(&bytes);
                                OwningPtr::make(wrapper, |ptr| unsafe {
                                    world.entity_mut(entity_id).insert_by_id(component_id, ptr);
                                });
                            }
                        }
                    };
                }

                insert_wrapper!(WrapperSize::W8, ComponentWrapper8);
                insert_wrapper!(WrapperSize::W16, ComponentWrapper16);
                insert_wrapper!(WrapperSize::W32, ComponentWrapper32);
                insert_wrapper!(WrapperSize::W64, ComponentWrapper64);
                insert_wrapper!(WrapperSize::W128, ComponentWrapper128);
                insert_wrapper!(WrapperSize::W256, ComponentWrapper256);
                insert_wrapper!(WrapperSize::W512, ComponentWrapper512);
                insert_wrapper!(WrapperSize::W1024, ComponentWrapper1024);
            } else {
                // PyObject storage: insert Py<PyAny> directly
                for &entity_id in entities {
                    let py_obj = component.clone().unbind();
                    OwningPtr::make(py_obj, |ptr| unsafe {
                        world.entity_mut(entity_id).insert_by_id(component_id, ptr);
                    });
                }
            }
        }
    }
    Ok(())
}
