//! Interpreter-neutral semantic tracing for snapshot parity tests.

use std::{
    collections::{BTreeMap, HashMap},
    env, fmt,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use bevy::{
    ecs::entity::Entity,
    prelude::{Name, Resource, World},
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const PARITY_TRACE_ENV: &str = "PYBEVY_PARITY_TRACE";

/// Backend-neutral payload form. Float values deliberately collapse to one marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonValue {
    None,
    Bool(bool),
    Int(String),
    Float,
    String(String),
    List(Vec<Self>),
    Map(BTreeMap<String, Self>),
}

impl CanonValue {
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"pybevy.parity.canon\0");
        self.hash_into(&mut hasher);
        let bytes = hasher.finalize();
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        match self {
            Self::None => write_frame(hasher, b"none", &[]),
            Self::Bool(value) => write_frame(hasher, b"bool", &[*value as u8]),
            Self::Int(value) => write_frame(hasher, b"int", value.as_bytes()),
            Self::Float => write_frame(hasher, b"float", &[]),
            Self::String(value) => write_frame(hasher, b"string", value.as_bytes()),
            Self::List(values) => {
                write_frame(hasher, b"list.len", &(values.len() as u64).to_le_bytes());
                for value in values {
                    value.hash_into(hasher);
                }
            }
            Self::Map(values) => {
                write_frame(hasher, b"map.len", &(values.len() as u64).to_le_bytes());
                for (key, value) in values {
                    write_frame(hasher, b"map.key", key.as_bytes());
                    value.hash_into(hasher);
                }
            }
        }
    }
}

fn write_frame(hasher: &mut Sha256, tag: &[u8], payload: &[u8]) {
    hasher.update((tag.len() as u64).to_le_bytes());
    hasher.update(tag);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload);
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParityOpKind {
    Spawn,
    Insert,
    Despawn,
    ResourceInsert,
    ResourceRemove,
    MessageWrite,
    ObserverTrigger,
}

#[derive(Clone, Debug)]
pub struct PendingParityOp {
    pub kind: ParityOpKind,
    pub type_name: Option<String>,
    pub payload_digest: String,
    pub target: Option<Entity>,
}

#[derive(Debug)]
struct ParityRunState {
    system: String,
    run_index: u64,
    next_spawn: u64,
    spawn_tokens: HashMap<Entity, String>,
    operations: Vec<PendingParityOp>,
}

/// Run-local adapter handle. Raw entity ids never leave this pending buffer.
#[derive(Clone, Debug)]
pub struct ParityRunHandle(Arc<Mutex<ParityRunState>>);

impl ParityRunHandle {
    fn new(system: String, run_index: u64) -> Self {
        Self(Arc::new(Mutex::new(ParityRunState {
            system,
            run_index,
            next_spawn: 0,
            spawn_tokens: HashMap::new(),
            operations: Vec::new(),
        })))
    }

    pub fn record_spawn(&self, entity: Entity, payload: &CanonValue) {
        let mut state = lock_or_recover(&self.0);
        let spawn_index = state.next_spawn;
        state.next_spawn += 1;
        let token = format!("spawn:{}/{}/{}", state.system, state.run_index, spawn_index);
        state.spawn_tokens.insert(entity, token);
        state.operations.push(PendingParityOp {
            kind: ParityOpKind::Spawn,
            type_name: None,
            payload_digest: payload.digest(),
            target: Some(entity),
        });
    }

    pub fn record_op(&self, operation: PendingParityOp) {
        lock_or_recover(&self.0).operations.push(operation);
    }

    fn spawn_tokens(&self) -> HashMap<Entity, String> {
        lock_or_recover(&self.0).spawn_tokens.clone()
    }

    fn resolve(
        &self,
        world: &World,
        entity_keys: &HashMap<Entity, String>,
    ) -> Result<ResolvedRun, ParityTraceError> {
        let mut state = lock_or_recover(&self.0);
        let operations = std::mem::take(&mut state.operations)
            .into_iter()
            .map(|operation| {
                let target = operation
                    .target
                    .map(|entity| resolve_target(world, entity_keys, entity))
                    .transpose()?;
                Ok(ResolvedOperation {
                    kind: operation.kind,
                    type_name: operation.type_name,
                    payload_digest: operation.payload_digest,
                    target,
                })
            })
            .collect::<Result<Vec<_>, ParityTraceError>>()?;
        Ok(ResolvedRun {
            system: state.system.clone(),
            run_index: state.run_index,
            operations,
        })
    }
}

fn resolve_target(
    world: &World,
    entity_keys: &HashMap<Entity, String>,
    entity: Entity,
) -> Result<String, ParityTraceError> {
    if let Some(token) = entity_keys.get(&entity) {
        return Ok(token.clone());
    }
    if let Some(name) = world.get::<Name>(entity) {
        return Ok(format!("name:{}", name.as_str()));
    }
    Err(ParityTraceError::UnresolvedTarget)
}

pub(crate) struct ResolvedRun {
    system: String,
    run_index: u64,
    operations: Vec<ResolvedOperation>,
}

/// One observer invocation and its private command queue.
pub struct ObserverTraceRun {
    observer: String,
    invocation_index: u64,
    run: ParityRunHandle,
}

impl ObserverTraceRun {
    pub fn run_handle(&self) -> &ParityRunHandle {
        &self.run
    }
}

#[derive(Serialize)]
struct ResolvedOperation {
    kind: ParityOpKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_name: Option<String>,
    payload_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
}

#[derive(Serialize)]
#[serde(tag = "record", rename_all = "snake_case")]
enum TraceRecord<'a> {
    SystemRun {
        system: &'a str,
        run_index: u64,
    },
    ObserverEntry {
        observer: &'a str,
        invocation_index: u64,
        trigger_type: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<&'a str>,
    },
    ObserverFlush {
        observer: &'a str,
        invocation_index: u64,
    },
    FlushBoundary {
        system: &'a str,
        run_index: u64,
    },
    Operation {
        system: &'a str,
        run_index: u64,
        #[serde(flatten)]
        operation: &'a ResolvedOperation,
    },
}

#[derive(Serialize)]
struct SequencedRecord<'a> {
    sequence: u64,
    #[serde(flatten)]
    record: TraceRecord<'a>,
}

struct SinkState {
    sequence: u64,
    writer: Box<dyn Write + Send>,
    entity_keys: HashMap<Entity, String>,
    observer_invocations: HashMap<String, u64>,
}

/// Process-output recorder shared by all traced systems in one App.
pub struct ParityOpSink {
    state: Mutex<SinkState>,
}

impl fmt::Debug for ParityOpSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParityOpSink")
            .finish_non_exhaustive()
    }
}

impl ParityOpSink {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ParityTraceError> {
        let file = File::create(path).map_err(ParityTraceError::Io)?;
        Ok(Self::new(Box::new(BufWriter::new(file))))
    }

    pub fn from_env() -> Result<Option<Self>, ParityTraceError> {
        let Some(path) = env::var_os(PARITY_TRACE_ENV) else {
            return Ok(None);
        };
        Ok(Some(Self::from_path(path)?))
    }

    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            state: Mutex::new(SinkState {
                sequence: 0,
                writer,
                entity_keys: HashMap::new(),
                observer_invocations: HashMap::new(),
            }),
        }
    }

    pub fn start_run(
        &self,
        system: &str,
        run_index: u64,
    ) -> Result<ParityRunHandle, ParityTraceError> {
        self.write(TraceRecord::SystemRun { system, run_index })?;
        Ok(ParityRunHandle::new(system.to_string(), run_index))
    }

    pub(crate) fn resolve_before_flush(
        &self,
        runs: &[ParityRunHandle],
        world: &World,
    ) -> Result<Vec<ResolvedRun>, ParityTraceError> {
        let entity_keys = self.register_spawn_tokens(runs.iter());
        runs.iter()
            .map(|run| run.resolve(world, &entity_keys))
            .collect()
    }

    pub(crate) fn record_flushed(&self, runs: &[ResolvedRun]) -> Result<(), ParityTraceError> {
        for run in runs {
            self.write(TraceRecord::FlushBoundary {
                system: &run.system,
                run_index: run.run_index,
            })?;
            for operation in &run.operations {
                self.write(TraceRecord::Operation {
                    system: &run.system,
                    run_index: run.run_index,
                    operation,
                })?;
            }
        }
        Ok(())
    }

    pub fn start_observer(
        &self,
        observer: &str,
        trigger_type: &str,
        target: Option<Entity>,
        world: &World,
    ) -> Result<ObserverTraceRun, ParityTraceError> {
        let target = target
            .map(|entity| self.resolve_and_cache_target(world, entity))
            .transpose()?;
        let mut state = lock_or_recover(&self.state);
        let invocation_index = state
            .observer_invocations
            .entry(observer.to_string())
            .or_default();
        let current_index = *invocation_index;
        *invocation_index += 1;
        Self::write_locked(
            &mut state,
            TraceRecord::ObserverEntry {
                observer,
                invocation_index: current_index,
                trigger_type,
                target: target.as_deref(),
            },
        )?;
        Ok(ObserverTraceRun {
            observer: observer.to_string(),
            invocation_index: current_index,
            run: ParityRunHandle::new(format!("observer:{observer}"), current_index),
        })
    }

    pub(crate) fn resolve_observer_before_flush(
        &self,
        observer: &ObserverTraceRun,
        world: &World,
    ) -> Result<ResolvedRun, ParityTraceError> {
        let entity_keys = self.register_spawn_tokens(std::iter::once(&observer.run));
        observer.run.resolve(world, &entity_keys)
    }

    pub(crate) fn record_observer_flushed(
        &self,
        observer: &ObserverTraceRun,
        resolved: &ResolvedRun,
    ) -> Result<(), ParityTraceError> {
        self.write(TraceRecord::ObserverFlush {
            observer: &observer.observer,
            invocation_index: observer.invocation_index,
        })?;
        for operation in &resolved.operations {
            self.write(TraceRecord::Operation {
                system: &resolved.system,
                run_index: resolved.run_index,
                operation,
            })?;
        }
        Ok(())
    }

    fn register_spawn_tokens<'a>(
        &self,
        runs: impl Iterator<Item = &'a ParityRunHandle>,
    ) -> HashMap<Entity, String> {
        let tokens = runs.flat_map(|run| run.spawn_tokens()).collect::<Vec<_>>();
        let mut state = lock_or_recover(&self.state);
        state.entity_keys.extend(tokens);
        state.entity_keys.clone()
    }

    fn resolve_and_cache_target(
        &self,
        world: &World,
        entity: Entity,
    ) -> Result<String, ParityTraceError> {
        let mut state = lock_or_recover(&self.state);
        if let Some(key) = state.entity_keys.get(&entity) {
            return Ok(key.clone());
        }
        let key = world
            .get::<Name>(entity)
            .map(|name| format!("name:{}", name.as_str()))
            .ok_or(ParityTraceError::UnresolvedTarget)?;
        state.entity_keys.insert(entity, key.clone());
        Ok(key)
    }

    fn write(&self, record: TraceRecord<'_>) -> Result<(), ParityTraceError> {
        let mut state = lock_or_recover(&self.state);
        Self::write_locked(&mut state, record)
    }

    fn write_locked(
        state: &mut SinkState,
        record: TraceRecord<'_>,
    ) -> Result<(), ParityTraceError> {
        let sequence = state.sequence;
        let encoded = serde_json::to_vec(&SequencedRecord { sequence, record })
            .map_err(ParityTraceError::Serialize)?;
        state
            .writer
            .write_all(&encoded)
            .map_err(ParityTraceError::Io)?;
        state
            .writer
            .write_all(b"\n")
            .map_err(ParityTraceError::Io)?;
        state.writer.flush().map_err(ParityTraceError::Io)?;
        state.sequence += 1;
        Ok(())
    }
}

#[derive(Resource, Clone, Debug)]
pub struct ParityTraceResource(pub Arc<ParityOpSink>);

#[derive(Debug)]
pub enum ParityTraceError {
    Io(std::io::Error),
    Serialize(serde_json::Error),
    UnresolvedTarget,
}

impl fmt::Display for ParityTraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "parity trace I/O failed: {error}"),
            Self::Serialize(error) => write!(formatter, "parity trace serialization failed: {error}"),
            Self::UnresolvedTarget => formatter.write_str(
                "parity trace target is unresolved; traced scenes may only address spawned or named entities",
            ),
        }
    }
}

impl std::error::Error for ParityTraceError {}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bevy::prelude::World;

    use super::*;

    #[derive(Clone)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            lock_or_recover(&self.0).extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn canonical_maps_sort_keys_and_ignore_float_values() {
        let left = CanonValue::Map(BTreeMap::from([
            ("b".to_string(), CanonValue::Float),
            ("a".to_string(), CanonValue::Int("2".to_string())),
        ]));
        let right = CanonValue::Map(BTreeMap::from([
            ("a".to_string(), CanonValue::Int("2".to_string())),
            ("b".to_string(), CanonValue::Float),
        ]));
        assert_eq!(left.digest(), right.digest());
        assert_eq!(CanonValue::Float.digest(), CanonValue::Float.digest());
    }

    #[test]
    fn sequence_allocation_and_append_share_one_lock() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = ParityOpSink::new(Box::new(SharedWriter(output.clone())));
        sink.start_run("first", 0).unwrap();
        sink.start_run("second", 0).unwrap();
        let text = String::from_utf8(lock_or_recover(&output).clone()).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert!(lines[0].contains("\"sequence\":0"));
        assert!(lines[1].contains("\"sequence\":1"));
    }

    #[test]
    fn resolves_spawn_tokens_before_apply_and_never_serializes_entity_ids() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = ParityOpSink::new(Box::new(SharedWriter(output.clone())));
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let run = sink.start_run("setup", 0).unwrap();
        run.record_spawn(entity, &CanonValue::None);
        let resolved = sink.resolve_before_flush(&[run], &world).unwrap();
        sink.record_flushed(&resolved).unwrap();
        let text = String::from_utf8(lock_or_recover(&output).clone()).unwrap();
        assert!(text.contains("spawn:setup/0/0"));
        assert!(!text.contains(&entity.to_bits().to_string()));
    }

    #[test]
    fn spawn_tokens_resolve_targets_in_later_runs() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = ParityOpSink::new(Box::new(SharedWriter(output.clone())));
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let setup = sink.start_run("setup", 0).unwrap();
        setup.record_spawn(entity, &CanonValue::None);
        let resolved = sink.resolve_before_flush(&[setup], &world).unwrap();
        sink.record_flushed(&resolved).unwrap();

        let update = sink.start_run("update", 0).unwrap();
        update.record_op(PendingParityOp {
            kind: ParityOpKind::Despawn,
            type_name: None,
            payload_digest: CanonValue::None.digest(),
            target: Some(entity),
        });
        let resolved = sink.resolve_before_flush(&[update], &world).unwrap();
        sink.record_flushed(&resolved).unwrap();

        let text = String::from_utf8(lock_or_recover(&output).clone()).unwrap();
        assert_eq!(text.matches("spawn:setup/0/0").count(), 2);
    }

    #[test]
    fn observer_entry_and_private_flush_wrap_its_operations() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = ParityOpSink::new(Box::new(SharedWriter(output.clone())));
        let mut world = World::new();
        let target = world.spawn(Name::new("target")).id();
        let observer = sink
            .start_observer("observe", "Impact", Some(target), &world)
            .unwrap();
        observer.run_handle().record_op(PendingParityOp {
            kind: ParityOpKind::Insert,
            type_name: Some("Audit".to_string()),
            payload_digest: CanonValue::None.digest(),
            target: Some(target),
        });
        let resolved = sink
            .resolve_observer_before_flush(&observer, &world)
            .unwrap();
        sink.record_observer_flushed(&observer, &resolved).unwrap();

        let text = String::from_utf8(lock_or_recover(&output).clone()).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("\"record\":\"observer_entry\""));
        assert!(lines[0].contains("\"trigger_type\":\"Impact\""));
        assert!(lines[0].contains("\"target\":\"name:target\""));
        assert!(lines[1].contains("\"record\":\"observer_flush\""));
        assert!(lines[2].contains("\"system\":\"observer:observe\""));
    }
}
