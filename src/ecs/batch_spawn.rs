use std::collections::HashMap;

use bevy::ecs::{component::ComponentId, entity::Entity, ptr::OwningPtr, world::World};
use pybevy_core::{
    LogicalTypeId, PreparedBatchComponent, PreparedUniformComponent,
    public_error::RESOURCE_COMPONENT_SPAWN, registry::global_registry,
};
use pybevy_ecs::shared::batch_spawn::{
    BatchCardinality, BatchSpawnCore, BatchSpawnPlan, BatchTypeKey, PreparedBatchInserter,
};
use pyo3::{
    exceptions::{PyStopIteration, PyTypeError, PyValueError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyTuple, PyType},
};

use super::{
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt,
        serialize_to_wrapper,
    },
    component_type::PyComponentType,
    component_wrapper::*,
    helpers::type_utils::get_python_type_name,
};

/// Enum to represent either a batch component or a uniform component
enum ComponentData {
    Batch {
        name: String,
        component_type: PyComponentType,
        prepared: Box<dyn PreparedBatchComponent>,
    },
    Uniform {
        name: String,
        component_type: PyComponentType,
        prepared: Box<dyn PreparedUniformComponent>,
        logical_type: Option<Option<LogicalTypeId>>,
    },
}

enum PreparedPayload {
    Columnar(Box<dyn PreparedBatchComponent>),
    Uniform(Box<dyn PreparedUniformComponent>),
}

struct MainPreparedInserter {
    component_id: ComponentId,
    component_type: PyComponentType,
    name: String,
    payload: PreparedPayload,
    logical_type: Option<Option<LogicalTypeId>>,
}

impl PreparedBatchInserter for MainPreparedInserter {
    fn component_id(&self) -> ComponentId {
        self.component_id
    }

    fn component_type(&self) -> BatchTypeKey {
        BatchTypeKey::new(component_type_key(self.component_type))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn cardinality(&self) -> BatchCardinality {
        match &self.payload {
            PreparedPayload::Columnar(prepared) => BatchCardinality::Columnar(prepared.count()),
            PreparedPayload::Uniform(_) => BatchCardinality::Uniform,
        }
    }

    fn insert(&mut self, world: &mut World, entities: &[Entity]) {
        match &mut self.payload {
            PreparedPayload::Columnar(prepared) => {
                prepared.insert(self.component_id, entities, world);
            }
            PreparedPayload::Uniform(prepared) => {
                prepared.insert(self.component_id, entities, world);
            }
        }
        if let (Some(logical_type), Some(native_type)) =
            (self.logical_type, self.component_type.type_id())
        {
            for &entity in entities {
                crate::ecs::commands::update_entity_logical_type(
                    world,
                    entity,
                    native_type,
                    logical_type,
                );
            }
        }
    }
}

fn component_type_key(component_type: PyComponentType) -> usize {
    match component_type {
        PyComponentType::Dynamic(type_ptr)
        | PyComponentType::Resource(type_ptr)
        | PyComponentType::Custom(type_ptr) => type_ptr as usize,
    }
}

/// Command to spawn multiple entities with batch and uniform components
pub struct SpawnBatchCommand {
    components: Vec<ComponentData>,
    explicit_count: Option<usize>,
    spawn_count: usize,
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
                let component_type_ptr = bridge.component_type_ptr(py, &component)?;
                let component_type_ptr = component_type_ptr as *const PyTypeObject;
                let component_type =
                    if global_registry::get_bridge_by_py_type(component_type_ptr).is_some() {
                        PyComponentType::Dynamic(component_type_ptr)
                    } else {
                        PyComponentType::Custom(component_type_ptr)
                    };
                let prepared = bridge.prepare(py, &component)?;
                component_data.push(ComponentData::Batch {
                    name: bridge.name().to_owned(),
                    component_type,
                    prepared,
                });
            } else {
                // It's a uniform component - determine its type
                let component_type = PyComponentType::try_from((&component.get_type(), py))?;
                let (name, prepared) = prepare_uniform(py, &component, component_type)?;
                component_data.push(ComponentData::Uniform {
                    name,
                    component_type,
                    prepared,
                    logical_type: crate::ecs::commands::component_logical_type(&component)?,
                });
            }
        }

        let spawn_count = validate_prepared_metadata(&component_data, count)?;

        Ok(SpawnBatchCommand {
            components: component_data,
            explicit_count: count,
            spawn_count,
        })
    }

    pub fn spawn_count(&self) -> usize {
        self.spawn_count
    }

    /// Apply the spawn batch command to the world
    pub fn apply(self, world: &mut World) -> PyResult<Vec<Entity>> {
        self.apply_inner(world, None)
    }

    /// Apply to IDs already reserved by the Commands adapter.
    pub fn apply_to(self, world: &mut World, entities: Vec<Entity>) -> PyResult<Vec<Entity>> {
        self.apply_inner(world, Some(entities))
    }

    fn apply_inner(
        self,
        world: &mut World,
        entities: Option<Vec<Entity>>,
    ) -> PyResult<Vec<Entity>> {
        let lifecycle_types = self
            .components
            .iter()
            .map(|component| match component {
                ComponentData::Batch { component_type, .. }
                | ComponentData::Uniform { component_type, .. } => *component_type,
            })
            .collect::<Vec<_>>();

        // Resolving/registering component identities is allowed to mutate the
        // World's type registry, but happens before any target entity exists.
        let insertions = Python::attach(|py| {
            self.components
                .into_iter()
                .map(|component| match component {
                    ComponentData::Batch {
                        name,
                        component_type,
                        prepared,
                    } => Box::new(MainPreparedInserter {
                        component_id: component_type.register_simple(world, py),
                        component_type,
                        name,
                        payload: PreparedPayload::Columnar(prepared),
                        logical_type: None,
                    }) as Box<dyn PreparedBatchInserter>,
                    ComponentData::Uniform {
                        name,
                        component_type,
                        prepared,
                        logical_type,
                    } => Box::new(MainPreparedInserter {
                        component_id: component_type.register_simple(world, py),
                        component_type,
                        name,
                        payload: PreparedPayload::Uniform(prepared),
                        logical_type,
                    }) as Box<dyn PreparedBatchInserter>,
                })
                .collect()
        });

        let validated =
            BatchSpawnCore::validate(BatchSpawnPlan::new(self.explicit_count, insertions))
                .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let committed = match entities {
            Some(entities) => BatchSpawnCore::apply_to(world, validated, entities),
            None => BatchSpawnCore::apply(world, validated),
        };

        for &entity in &committed.entities {
            crate::ecs::lifecycle_mutation::finish_new_bundle(world, entity, &lifecycle_types);
        }

        Ok(committed.entities)
    }
}

pub(crate) fn prepare_iter_batch(
    py: Python<'_>,
    batch: &Bound<'_, PyAny>,
) -> PyResult<Vec<SpawnBatchCommand>> {
    let iter = batch.call_method0("__iter__")?;
    let mut prepared = Vec::new();
    loop {
        match iter.call_method0("__next__") {
            Ok(bundle) => {
                let components = if let Ok(tuple) = bundle.cast::<PyTuple>() {
                    tuple.clone()
                } else {
                    PyTuple::new(py, [&bundle])?
                };
                prepared.push(SpawnBatchCommand::new(py, &components, Some(1))?);
            }
            Err(error) if error.is_instance_of::<PyStopIteration>(py) => break,
            Err(error) => return Err(error),
        }
    }
    Ok(prepared)
}

fn validate_prepared_metadata(
    components: &[ComponentData],
    explicit_count: Option<usize>,
) -> PyResult<usize> {
    let spawn_count = explicit_count
        .or_else(|| {
            components.iter().find_map(|component| match component {
                ComponentData::Batch { prepared, .. } => Some(prepared.count()),
                ComponentData::Uniform { .. } => None,
            })
        })
        .ok_or_else(|| {
            PyValueError::new_err(
                "spawn_batch requires either a 'count' parameter or at least one batch component",
            )
        })?;

    let mut seen = HashMap::with_capacity(components.len());
    for component in components {
        let (name, component_type) = match component {
            ComponentData::Batch {
                name,
                component_type,
                prepared,
            } => {
                let actual = prepared.count();
                if actual != spawn_count {
                    return Err(PyValueError::new_err(format!(
                        "{name} has {actual} elements but expected {spawn_count}"
                    )));
                }
                (name, *component_type)
            }
            ComponentData::Uniform {
                name,
                component_type,
                ..
            } => (name, *component_type),
        };
        let type_key = component_type_key(component_type);
        if let Some(first) = seen.insert(type_key, name) {
            return Err(PyValueError::new_err(format!(
                "spawn_batch component {name} duplicates {first}"
            )));
        }
    }

    Ok(spawn_count)
}

enum PreparedCustomUniform {
    Wrapper {
        bytes: Vec<u8>,
        wrapper_size: WrapperSize,
    },
    PyObject(Py<PyAny>),
}

impl PreparedUniformComponent for PreparedCustomUniform {
    fn insert(&mut self, component_id: ComponentId, entities: &[Entity], world: &mut World) {
        match self {
            Self::Wrapper {
                bytes,
                wrapper_size,
            } => {
                macro_rules! insert_wrapper {
                    ($size:expr, $wrapper_type:ty) => {
                        if *wrapper_size == $size {
                            for &entity_id in entities {
                                let mut wrapper = <$wrapper_type>::default();
                                wrapper.data[..bytes.len()].copy_from_slice(bytes);
                                OwningPtr::make(wrapper, |ptr| {
                                    // SAFETY: preparation selected the exact wrapper descriptor
                                    // registered for component_id and validated the byte length.
                                    unsafe {
                                        world.entity_mut(entity_id).insert_by_id(component_id, ptr);
                                    }
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
            }
            Self::PyObject(value) => Python::attach(|py| {
                for &entity_id in entities {
                    let value = value.clone_ref(py);
                    OwningPtr::make(value, |ptr| {
                        // SAFETY: component_id was registered for Py<PyAny> storage for this
                        // exact custom class, and each entity receives a new strong reference.
                        unsafe {
                            world.entity_mut(entity_id).insert_by_id(component_id, ptr);
                        }
                    });
                }
            }),
        }
    }
}

fn prepare_uniform(
    py: Python,
    component: &Bound<'_, PyAny>,
    component_type: PyComponentType,
) -> PyResult<(String, Box<dyn PreparedUniformComponent>)> {
    match component_type {
        PyComponentType::Dynamic(type_ptr) => {
            let bridge = global_registry::get_bridge_by_py_type(type_ptr).ok_or_else(|| {
                PyValueError::new_err(format!(
                    "No component bridge registered for Dynamic type {type_ptr:?}"
                ))
            })?;
            let prepared = bridge.prepare_uniform(component)?;
            Ok((bridge.name().to_owned(), prepared))
        }
        PyComponentType::Resource(_) => Err(PyTypeError::new_err(RESOURCE_COMPONENT_SPAWN)),
        PyComponentType::Custom(raw_type_ptr) => {
            let name = get_python_type_name(py, raw_type_ptr);

            // SAFETY: decorated component classes remain alive for the interpreter
            // lifetime, and raw_type_ptr came from the component object's exact type.
            let py_type =
                unsafe { Bound::from_borrowed_ptr(py, raw_type_ptr as *mut pyo3::ffi::PyObject) };
            let class = py_type.cast::<PyType>()?;
            let storage_type = ComponentStorageType::from_python_class(class)?;
            let prepared: Box<dyn PreparedUniformComponent> = match storage_type {
                ComponentStorageType::Wrapper(_) => {
                    let layout = ComponentLayout::from_annotations(class)?;
                    let bytes = serialize_to_wrapper(component, &layout)?;
                    let wrapper_size = WrapperSize::for_size(bytes.len()).ok_or_else(|| {
                        PyValueError::new_err(format!(
                            "Component '{name}' has invalid wrapper size {}",
                            bytes.len()
                        ))
                    })?;
                    Box::new(PreparedCustomUniform::Wrapper {
                        bytes,
                        wrapper_size,
                    })
                }
                ComponentStorageType::PyObject => {
                    Box::new(PreparedCustomUniform::PyObject(component.clone().unbind()))
                }
            };
            Ok((name, prepared))
        }
    }
}
