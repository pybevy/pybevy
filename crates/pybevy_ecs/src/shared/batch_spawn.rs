//! Backend-neutral validation and commit ordering for enhanced batch spawning.
//!
//! Interpreter adapters are responsible for resolving component identities and
//! producing fully owned, infallible inserters. This module validates the whole
//! plan before allocating entities, then commits columnar insertions before
//! uniform insertions to preserve PyBevy's existing enhanced-batch ordering.
//!
//! Commit is deliberately not transactional. Native Bevy hooks run inline and
//! may mutate or despawn targets, recursively spawn entities, or panic.

use std::{collections::HashMap, error::Error, fmt};

use bevy::ecs::{component::ComponentId, entity::Entity, world::World};

/// Opaque interpreter type identity carried through the shared batch core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchTypeKey(usize);

impl BatchTypeKey {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

/// Whether one prepared insertion supplies one value or one value per entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchCardinality {
    Uniform,
    Columnar(usize),
}

/// A fully owned insertion prepared by an interpreter adapter.
///
/// `insert` has no recoverable error channel: dtype, shape, annotation, bridge,
/// serialization, and count failures must be eliminated before validation.
/// Native hook panics are allowed to unwind through commit.
pub trait PreparedBatchInserter: Send + 'static {
    fn component_id(&self) -> ComponentId;
    fn component_type(&self) -> BatchTypeKey;
    fn name(&self) -> &str;
    fn cardinality(&self) -> BatchCardinality;
    fn insert(&mut self, world: &mut World, entities: &[Entity]);
}

/// An unvalidated batch-spawn request assembled by a backend adapter.
pub struct BatchSpawnPlan {
    explicit_count: Option<usize>,
    insertions: Vec<Box<dyn PreparedBatchInserter>>,
}

impl BatchSpawnPlan {
    pub fn new(
        explicit_count: Option<usize>,
        insertions: Vec<Box<dyn PreparedBatchInserter>>,
    ) -> Self {
        Self {
            explicit_count,
            insertions,
        }
    }
}

/// A failure found before the first entity is allocated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchSpawnError {
    MissingCount,
    CountMismatch {
        component: String,
        expected: usize,
        actual: usize,
    },
    DuplicateComponent {
        component_id: ComponentId,
        first: String,
        duplicate: String,
    },
}

impl fmt::Display for BatchSpawnError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCount => formatter.write_str(
                "spawn_batch requires either a 'count' parameter or at least one batch component",
            ),
            Self::CountMismatch {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "{component} has {actual} elements but expected {expected}"
            ),
            Self::DuplicateComponent {
                component_id,
                first,
                duplicate,
            } => write!(
                formatter,
                "spawn_batch component {duplicate} duplicates {first} at ComponentId {component_id:?}"
            ),
        }
    }
}

impl Error for BatchSpawnError {}

/// A validated plan whose ordinary failures have all occurred before commit.
pub struct ValidatedBatchSpawn {
    spawn_count: usize,
    columnar: Vec<Box<dyn PreparedBatchInserter>>,
    uniform: Vec<Box<dyn PreparedBatchInserter>>,
}

impl ValidatedBatchSpawn {
    pub const fn spawn_count(&self) -> usize {
        self.spawn_count
    }

    pub fn insertion_count(&self) -> usize {
        self.columnar.len() + self.uniform.len()
    }
}

/// One successfully invoked prepared insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchInsertionFact {
    pub component_id: ComponentId,
    pub component_type: BatchTypeKey,
    pub entities: Vec<Entity>,
}

/// The intended entities and insertions from a completed commit.
///
/// Hooks may already have removed components or despawned any of these IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedBatch {
    pub entities: Vec<Entity>,
    pub insertions: Vec<BatchInsertionFact>,
}

pub struct BatchSpawnCore;

impl BatchSpawnCore {
    /// Validate counts and duplicate component identities without touching a World.
    pub fn validate(plan: BatchSpawnPlan) -> Result<ValidatedBatchSpawn, BatchSpawnError> {
        let spawn_count = match plan.explicit_count {
            Some(count) => count,
            None => plan
                .insertions
                .iter()
                .find_map(|inserter| match inserter.cardinality() {
                    BatchCardinality::Uniform => None,
                    BatchCardinality::Columnar(count) => Some(count),
                })
                .ok_or(BatchSpawnError::MissingCount)?,
        };

        let mut seen = HashMap::with_capacity(plan.insertions.len());
        let mut columnar = Vec::new();
        let mut uniform = Vec::new();

        for inserter in plan.insertions {
            if let BatchCardinality::Columnar(actual) = inserter.cardinality()
                && actual != spawn_count
            {
                return Err(BatchSpawnError::CountMismatch {
                    component: inserter.name().to_owned(),
                    expected: spawn_count,
                    actual,
                });
            }

            let component_id = inserter.component_id();
            if let Some(first) = seen.insert(component_id, inserter.name().to_owned()) {
                return Err(BatchSpawnError::DuplicateComponent {
                    component_id,
                    first,
                    duplicate: inserter.name().to_owned(),
                });
            }

            match inserter.cardinality() {
                BatchCardinality::Columnar(_) => columnar.push(inserter),
                BatchCardinality::Uniform => uniform.push(inserter),
            }
        }

        Ok(ValidatedBatchSpawn {
            spawn_count,
            columnar,
            uniform,
        })
    }

    /// Allocate all targets, then commit columnar and uniform insertions.
    ///
    /// Native hook panics propagate. No cleanup or rollback is attempted.
    pub fn apply(world: &mut World, plan: ValidatedBatchSpawn) -> CommittedBatch {
        let entities = (0..plan.spawn_count)
            .map(|_| world.spawn_empty().id())
            .collect::<Vec<_>>();
        Self::apply_to(world, plan, entities)
    }

    /// Commit into entities reserved by an interpreter command adapter.
    pub fn apply_to(
        world: &mut World,
        plan: ValidatedBatchSpawn,
        entities: Vec<Entity>,
    ) -> CommittedBatch {
        assert_eq!(entities.len(), plan.spawn_count);
        let mut facts = Vec::with_capacity(plan.insertion_count());

        for mut inserter in plan.columnar.into_iter().chain(plan.uniform) {
            let component_id = inserter.component_id();
            let component_type = inserter.component_type();
            inserter.insert(world, &entities);
            facts.push(BatchInsertionFact {
                component_id,
                component_type,
                entities: entities.clone(),
            });
        }

        CommittedBatch {
            entities,
            insertions: facts,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bevy::ecs::component::Component;

    use super::*;

    #[derive(Component, Debug, PartialEq, Eq)]
    struct ColumnValue(u32);

    #[derive(Component, Debug, PartialEq, Eq)]
    struct UniformValue(u32);

    enum FakeValues {
        Column(Vec<u32>),
        Uniform(u32),
    }

    struct FakeInserter {
        component_id: ComponentId,
        component_type: BatchTypeKey,
        name: &'static str,
        values: FakeValues,
        order: Arc<Mutex<Vec<&'static str>>>,
    }

    impl FakeInserter {
        fn column(
            component_id: ComponentId,
            name: &'static str,
            values: Vec<u32>,
            order: Arc<Mutex<Vec<&'static str>>>,
        ) -> Self {
            Self {
                component_id,
                component_type: BatchTypeKey::new(component_id.index()),
                name,
                values: FakeValues::Column(values),
                order,
            }
        }

        fn uniform(
            component_id: ComponentId,
            name: &'static str,
            value: u32,
            order: Arc<Mutex<Vec<&'static str>>>,
        ) -> Self {
            Self {
                component_id,
                component_type: BatchTypeKey::new(component_id.index()),
                name,
                values: FakeValues::Uniform(value),
                order,
            }
        }
    }

    impl PreparedBatchInserter for FakeInserter {
        fn component_id(&self) -> ComponentId {
            self.component_id
        }

        fn component_type(&self) -> BatchTypeKey {
            self.component_type
        }

        fn name(&self) -> &str {
            self.name
        }

        fn cardinality(&self) -> BatchCardinality {
            match &self.values {
                FakeValues::Column(values) => BatchCardinality::Columnar(values.len()),
                FakeValues::Uniform(_) => BatchCardinality::Uniform,
            }
        }

        fn insert(&mut self, world: &mut World, entities: &[Entity]) {
            self.order
                .lock()
                .expect("fake insertion log lock poisoned")
                .push(self.name);
            match &self.values {
                FakeValues::Column(values) => {
                    for (&entity, &value) in entities.iter().zip(values) {
                        world.entity_mut(entity).insert(ColumnValue(value));
                    }
                }
                FakeValues::Uniform(value) => {
                    for &entity in entities {
                        world.entity_mut(entity).insert(UniformValue(*value));
                    }
                }
            }
        }
    }

    fn boxed(inserter: FakeInserter) -> Box<dyn PreparedBatchInserter> {
        Box::new(inserter)
    }

    #[test]
    fn infers_count_and_commits_columnar_before_uniform() {
        let mut world = World::new();
        let column_id = world.register_component::<ColumnValue>();
        let uniform_id = world.register_component::<UniformValue>();
        let order = Arc::new(Mutex::new(Vec::new()));
        let plan = BatchSpawnPlan::new(
            None,
            vec![
                boxed(FakeInserter::uniform(
                    uniform_id,
                    "uniform",
                    9,
                    Arc::clone(&order),
                )),
                boxed(FakeInserter::column(
                    column_id,
                    "column",
                    vec![1, 2],
                    Arc::clone(&order),
                )),
            ],
        );

        let validated = BatchSpawnCore::validate(plan).expect("plan should validate");
        assert_eq!(validated.spawn_count(), 2);
        let committed = BatchSpawnCore::apply(&mut world, validated);

        assert_eq!(
            *order.lock().expect("fake insertion log lock poisoned"),
            vec!["column", "uniform"]
        );
        assert_eq!(committed.entities.len(), 2);
        assert_eq!(committed.insertions.len(), 2);
        for (index, entity) in committed.entities.iter().copied().enumerate() {
            assert_eq!(
                world.get::<ColumnValue>(entity),
                Some(&ColumnValue(index as u32 + 1))
            );
            assert_eq!(world.get::<UniformValue>(entity), Some(&UniformValue(9)));
        }
    }

    #[test]
    fn rejects_missing_and_mismatched_counts() {
        let mut world = World::new();
        let uniform_id = world.register_component::<UniformValue>();
        let column_id = world.register_component::<ColumnValue>();
        let order = Arc::new(Mutex::new(Vec::new()));

        let missing = BatchSpawnPlan::new(
            None,
            vec![boxed(FakeInserter::uniform(
                uniform_id,
                "uniform",
                1,
                Arc::clone(&order),
            ))],
        );
        assert_eq!(
            BatchSpawnCore::validate(missing).err(),
            Some(BatchSpawnError::MissingCount)
        );

        let mismatch = BatchSpawnPlan::new(
            Some(3),
            vec![boxed(FakeInserter::column(
                column_id,
                "column",
                vec![1, 2],
                order,
            ))],
        );
        assert_eq!(
            BatchSpawnCore::validate(mismatch).err(),
            Some(BatchSpawnError::CountMismatch {
                component: "column".to_owned(),
                expected: 3,
                actual: 2,
            })
        );
    }

    #[test]
    fn rejects_duplicate_component_ids_before_commit() {
        let mut world = World::new();
        let component_id = world.register_component::<ColumnValue>();
        let order = Arc::new(Mutex::new(Vec::new()));
        let plan = BatchSpawnPlan::new(
            Some(2),
            vec![
                boxed(FakeInserter::column(
                    component_id,
                    "first",
                    vec![1, 2],
                    Arc::clone(&order),
                )),
                boxed(FakeInserter::column(
                    component_id,
                    "duplicate",
                    vec![3, 4],
                    order,
                )),
            ],
        );

        assert_eq!(
            BatchSpawnCore::validate(plan).err(),
            Some(BatchSpawnError::DuplicateComponent {
                component_id,
                first: "first".to_owned(),
                duplicate: "duplicate".to_owned(),
            })
        );
        assert_eq!(world.query::<&ColumnValue>().iter(&world).count(), 0);
    }

    #[test]
    fn explicit_zero_and_empty_explicit_plans_are_valid() {
        let mut world = World::new();
        let zero = BatchSpawnCore::validate(BatchSpawnPlan::new(Some(0), Vec::new()))
            .expect("explicit zero should validate");
        let committed = BatchSpawnCore::apply(&mut world, zero);
        assert!(committed.entities.is_empty());
        assert!(committed.insertions.is_empty());

        let empty = BatchSpawnCore::validate(BatchSpawnPlan::new(Some(3), Vec::new()))
            .expect("an explicit count permits an empty component plan");
        let committed = BatchSpawnCore::apply(&mut world, empty);
        assert_eq!(committed.entities.len(), 3);
        assert!(committed.insertions.is_empty());
    }
}
