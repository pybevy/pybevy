use std::{collections::HashMap, mem::take, sync::Arc};

use bevy::ecs::{component::ComponentId, entity::Entity, ptr::OwningPtr, world::World};
use pybevy_core::{
    LogicalTypeId, PreparedBatchComponent, PreparedUniformComponent,
    public_error::{self, RESOURCE_COMPONENT_SPAWN},
    registry::global_registry,
};
use pybevy_ecs::shared::batch_spawn::{
    BatchCardinality, BatchSpawnCore, BatchSpawnPlan, BatchTypeKey, PreparedBatchInserter,
    ValidatedBatchSpawn,
};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyMemoryError, PyStopIteration, PyTypeError, PyValueError},
    ffi::{PyObject as RawPyObject, PyTypeObject},
    prelude::*,
    types::{PyTuple, PyType},
};

use super::{
    commands::{
        component_logical_type, component_relationship_parent, reject_borrowed_component_proxy,
        update_entity_logical_type, validate_inserted_link,
    },
    component_layout::{ComponentStorageType, ComponentStorageTypeExt, serialize_to_wrapper},
    component_type::{
        PreparedCustomComponentRegistration, PyComponentType, check_custom_component_identity,
        register_prepared_custom_component,
    },
    component_wrapper::*,
    custom_batch::PyCustomComponentBatch,
    helpers::type_utils::get_python_type_name,
    lifecycle_mutation::finish_new_bundle,
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
                update_entity_logical_type(world, entity, native_type, logical_type);
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
    retained_types: Vec<Py<PyType>>,
    registrations: Vec<Option<PreparedCustomComponentRegistration>>,
    relationship_parents: Vec<Entity>,
}

struct ValidatedSpawnBatch {
    plan: ValidatedBatchSpawn,
    lifecycle_types: Vec<PyComponentType>,
}

struct CommittedSpawnBatch {
    entities: Vec<Entity>,
    lifecycle_types: Vec<PyComponentType>,
}

impl CommittedSpawnBatch {
    fn finish(&self, world: &mut World) {
        for &entity in &self.entities {
            finish_new_bundle(world, entity, &self.lifecycle_types);
        }
    }
}

/// Bevy panics once entity indices run out, and an entity id buffer for an
/// unbounded count aborts on allocation failure.
fn check_spawn_count(count: usize) -> PyResult<()> {
    if count > u32::MAX as usize {
        return Err(PyValueError::new_err(
            public_error::spawn_batch_count_too_large(count),
        ));
    }
    Vec::<Entity>::new()
        .try_reserve_exact(count)
        .map_err(|_| PyMemoryError::new_err(public_error::spawn_batch_allocation_failed(count)))
}

impl SpawnBatchCommand {
    /// Create a new spawn batch command from Python components
    pub fn new(
        py: Python,
        components: &Bound<'_, PyTuple>,
        count: Option<usize>,
    ) -> PyResult<Self> {
        if let Some(count) = count {
            check_spawn_count(count)?;
        }
        let mut component_data = Vec::new();
        let mut retained_types = Vec::new();
        let mut registrations = Vec::new();
        let mut relationship_parents = Vec::new();

        for component in components.iter() {
            reject_borrowed_component_proxy(&component)?;
            // Check if it's a registered batch component
            let type_ptr = component.get_type().as_type_ptr();
            if let Some(bridge) = global_registry::get_batch_bridge_by_py_type(type_ptr) {
                let component_type_ptr = bridge.component_type_ptr(py, &component)?;
                let component_type_ptr = component_type_ptr as *const PyTypeObject;
                // SAFETY: the live batch bridge supplies its retained component class.
                let class =
                    unsafe { Bound::from_borrowed_ptr(py, component_type_ptr.cast_mut().cast()) };
                retained_types.push(class.cast_into::<PyType>()?.unbind());
                let component_type =
                    if global_registry::get_bridge_by_py_type(component_type_ptr).is_some() {
                        PyComponentType::Dynamic(component_type_ptr)
                    } else {
                        PyComponentType::Custom(component_type_ptr)
                    };
                let prepared = bridge.prepare(py, &component)?;
                registrations.push(if matches!(component_type, PyComponentType::Custom(_)) {
                    Some(
                        component
                            .extract::<PyRef<PyCustomComponentBatch>>()?
                            .prepared_registration(py)?,
                    )
                } else {
                    None
                });
                component_data.push(ComponentData::Batch {
                    name: bridge.name().to_owned(),
                    component_type,
                    prepared,
                });
            } else {
                retained_types.push(component.get_type().unbind());
                // It's a uniform component - determine its type
                let component_type = PyComponentType::try_from((&component.get_type(), py))?;
                if let Some((parent, _)) =
                    component_relationship_parent(&component, component_type)?
                {
                    relationship_parents.push(parent);
                }
                let (name, prepared, registration) =
                    prepare_uniform(py, &component, component_type)?;
                registrations.push(registration);
                component_data.push(ComponentData::Uniform {
                    name,
                    component_type,
                    prepared,
                    logical_type: component_logical_type(&component)?,
                });
            }
        }

        let spawn_count = validate_prepared_metadata(&component_data, count)?;

        Ok(SpawnBatchCommand {
            components: component_data,
            explicit_count: count,
            spawn_count,
            retained_types,
            registrations,
            relationship_parents,
        })
    }

    pub(crate) fn traverse(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained_types {
            visit.call(class)?;
        }
        for registration in self.registrations.iter().flatten() {
            registration.traverse(visit.clone())?;
        }
        for component in &self.components {
            if let ComponentData::Uniform { prepared, .. } = component {
                prepared.traverse(visit.clone())?;
            }
        }
        Ok(())
    }

    pub fn spawn_count(&self) -> usize {
        self.spawn_count
    }

    /// Validate every bundle before allocating or inserting any targets.
    pub(crate) fn apply_many(world: &mut World, mut batches: Vec<Self>) -> PyResult<Vec<Entity>> {
        let plans = batches
            .iter_mut()
            .map(|batch| batch.validate(world, None))
            .collect::<PyResult<Vec<_>>>()?;
        let committed = batches
            .iter()
            .zip(plans)
            .map(|(batch, plan)| batch.commit(world, plan, None))
            .collect::<Vec<_>>();
        for batch in &committed {
            batch.finish(world);
        }
        Ok(committed
            .into_iter()
            .flat_map(|batch| batch.entities)
            .collect())
    }

    /// Reject the whole batch and reclaim empty reservations on preflight failure.
    pub(crate) fn apply_many_to(
        world: &mut World,
        mut batches: Vec<(Self, Vec<Entity>)>,
    ) -> PyResult<()> {
        let mut plans = Vec::with_capacity(batches.len());
        for (batch, entities) in &mut batches {
            match batch.validate(world, Some(entities)) {
                Ok(plan) => plans.push(plan),
                Err(error) => {
                    for (_, entities) in &batches {
                        for &entity in entities {
                            if let Ok(target) = world.get_entity(entity)
                                && target.archetype().components().is_empty()
                            {
                                world.despawn(entity);
                            }
                        }
                    }
                    return Err(error);
                }
            }
        }
        let committed = batches
            .iter_mut()
            .zip(plans)
            .map(|((batch, entities), plan)| batch.commit(world, plan, Some(take(entities))))
            .collect::<Vec<_>>();
        for batch in &committed {
            batch.finish(world);
        }
        Ok(())
    }

    fn validate(
        &mut self,
        world: &mut World,
        entities: Option<&[Entity]>,
    ) -> PyResult<ValidatedSpawnBatch> {
        Python::attach(|py| {
            for registration in self.registrations.iter().flatten() {
                registration.validate_current(py)?;
            }
            Ok::<(), PyErr>(())
        })?;
        for registration in self.registrations.iter().flatten() {
            check_custom_component_identity(world, registration)?;
        }
        if self.spawn_count != 0 && !self.relationship_parents.is_empty() {
            for &parent in &self.relationship_parents {
                validate_inserted_link(world, Entity::PLACEHOLDER, parent)
                    .map_err(|error| PyTypeError::new_err(error.to_string()))?;
            }
            if let Some(entities) = entities {
                for &child in entities {
                    // Earlier queued commands can populate a reserved target.
                    if world
                        .get_entity(child)
                        .is_ok_and(|target| !target.archetype().components().is_empty())
                    {
                        for &parent in &self.relationship_parents {
                            validate_inserted_link(world, child, parent)
                                .map_err(|error| PyTypeError::new_err(error.to_string()))?;
                        }
                    }
                }
            }
        }
        let lifecycle_types = self
            .components
            .iter()
            .map(|component| match component {
                ComponentData::Batch { component_type, .. }
                | ComponentData::Uniform { component_type, .. } => *component_type,
            })
            .collect::<Vec<_>>();

        // Registry changes happen before any prepared insertion commits.
        let insertions = Python::attach(|py| {
            take(&mut self.components)
                .into_iter()
                .zip(&self.registrations)
                .map(|(component, registration)| {
                    let kind = match &component {
                        ComponentData::Batch { component_type, .. }
                        | ComponentData::Uniform { component_type, .. } => component_type,
                    };
                    let component_id = match registration {
                        Some(registration) => {
                            register_prepared_custom_component(world, registration)?
                        }
                        None => kind.register_simple(world, py),
                    };
                    Ok(match component {
                        ComponentData::Batch {
                            name,
                            component_type,
                            prepared,
                        } => Box::new(MainPreparedInserter {
                            component_id,
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
                            component_id,
                            component_type,
                            name,
                            payload: PreparedPayload::Uniform(prepared),
                            logical_type,
                        }) as Box<dyn PreparedBatchInserter>,
                    })
                })
                .collect::<PyResult<Vec<_>>>()
        })?;

        let plan = BatchSpawnCore::validate(BatchSpawnPlan::new(self.explicit_count, insertions))
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(ValidatedSpawnBatch {
            plan,
            lifecycle_types,
        })
    }

    fn commit(
        &self,
        world: &mut World,
        validated: ValidatedSpawnBatch,
        entities: Option<Vec<Entity>>,
    ) -> CommittedSpawnBatch {
        let committed = match entities {
            Some(entities) => BatchSpawnCore::apply_to(world, validated.plan, entities),
            None => BatchSpawnCore::apply(world, validated.plan),
        };

        CommittedSpawnBatch {
            entities: committed.entities,
            lifecycle_types: validated.lifecycle_types,
        }
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
    fn traverse(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        if let Self::PyObject(value) = self {
            visit.call(value)?;
        }
        Ok(())
    }

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
) -> PyResult<(
    String,
    Box<dyn PreparedUniformComponent>,
    Option<PreparedCustomComponentRegistration>,
)> {
    match component_type {
        PyComponentType::Dynamic(type_ptr) => {
            let bridge = global_registry::get_bridge_by_py_type(type_ptr).ok_or_else(|| {
                PyValueError::new_err(format!(
                    "No component bridge registered for Dynamic type {type_ptr:?}"
                ))
            })?;
            let prepared = bridge.prepare_uniform(component)?;
            Ok((bridge.name().to_owned(), prepared, None))
        }
        PyComponentType::Resource(_) => Err(PyTypeError::new_err(RESOURCE_COMPONENT_SPAWN)),
        PyComponentType::Custom(raw_type_ptr) => {
            let name = get_python_type_name(py, raw_type_ptr);

            // SAFETY: the live component retains its exact class during preparation.
            let py_type = unsafe { Bound::from_borrowed_ptr(py, raw_type_ptr as *mut RawPyObject) };
            let class = py_type.cast::<PyType>()?;
            // A uniform component is prepared once and reused for every entity in the batch.
            let (storage_type, wrapper_layout) = ComponentStorageType::storage_with_layout(class)?;
            let registration = PreparedCustomComponentRegistration::from_python_class_with_layout(
                class,
                storage_type,
                wrapper_layout.clone().map(Arc::new),
            )?;
            let prepared: Box<dyn PreparedUniformComponent> = match storage_type {
                ComponentStorageType::Wrapper(_) => {
                    let layout = wrapper_layout
                        .expect("wrapper storage carries the layout that selected it");
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
            Ok((name, prepared, Some(registration)))
        }
    }
}
