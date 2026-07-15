//! Interpreter-neutral storage and identity for Python-defined buffered messages.
//!
//! Native Bevy message types continue to use `Messages<T>`. This module owns the
//! backend-independent behavior needed by dynamically typed Python channels:
//! two-update retention, channel-local sequences, independent reader cursors,
//! exact/qualified-name registration, and synthetic scheduler access metadata.

use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

use bevy::ecs::{component::ComponentId, resource::Resource};

/// Stable identity of one Python-defined message channel within an App.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageChannelId(u32);

impl MessageChannelId {
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Exact interpreter identity of one Python message class.
///
/// Adapters map their stable class-object identity to this integer key and must
/// retain a strong class handle while it exists, so the interpreter cannot
/// recycle the identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MessageTypeKey(usize);

impl MessageTypeKey {
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Dynamic equivalent of Bevy's typed `MessageId<M>` for a Python message.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PythonMessageId {
    pub channel: MessageChannelId,
    pub sequence: u64,
}

/// One retained record in a Python message channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageRecord<V> {
    pub sequence: u64,
    pub value: V,
}

/// Persistent consumption state owned by one reader parameter.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MessageCursor {
    pub next_sequence: u64,
    pub missed: u64,
}

/// Shared cursor handle retained by a scheduled system or observer callback.
pub type SharedMessageCursor = Arc<Mutex<MessageCursor>>;

/// Create a new Bevy-like cursor that includes every record still retained.
#[must_use]
pub fn new_message_cursor() -> SharedMessageCursor {
    Arc::new(Mutex::new(MessageCursor::default()))
}

#[derive(Debug)]
struct ChannelLog<V> {
    next_sequence: u64,
    dropped_before: u64,
    previous: VecDeque<MessageRecord<V>>,
    current: VecDeque<MessageRecord<V>>,
}

impl<V> Default for ChannelLog<V> {
    fn default() -> Self {
        Self {
            next_sequence: 0,
            dropped_before: 0,
            previous: VecDeque::new(),
            current: VecDeque::new(),
        }
    }
}

#[derive(Debug)]
struct MessageLog<V> {
    channels: HashMap<MessageChannelId, ChannelLog<V>>,
}

impl<V> Default for MessageLog<V> {
    fn default() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }
}

/// Deterministic store, cursor, and registry failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageStoreError {
    ChannelNotRegistered(MessageChannelId),
    ChannelCapacityExhausted,
    SequenceOverflow(MessageChannelId),
    CursorMissedOverflow(MessageChannelId),
    CursorOutOfOrder {
        channel: MessageChannelId,
        expected: u64,
        found: u64,
    },
    SnapshotRecordUnavailable {
        channel: MessageChannelId,
        sequence: u64,
    },
}

impl fmt::Display for MessageStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChannelNotRegistered(channel) => {
                write!(
                    formatter,
                    "message channel {} is not registered",
                    channel.get()
                )
            }
            Self::ChannelCapacityExhausted => {
                formatter.write_str("message channel id capacity is exhausted")
            }
            Self::SequenceOverflow(channel) => write!(
                formatter,
                "message sequence capacity is exhausted for channel {}",
                channel.get()
            ),
            Self::CursorMissedOverflow(channel) => write!(
                formatter,
                "missed-message count overflowed for channel {}",
                channel.get()
            ),
            Self::CursorOutOfOrder {
                channel,
                expected,
                found,
            } => write!(
                formatter,
                "message snapshot for channel {} tried to consume sequence {found} before {expected}",
                channel.get()
            ),
            Self::SnapshotRecordUnavailable { channel, sequence } => write!(
                formatter,
                "message snapshot sequence {sequence} is not retained in channel {}",
                channel.get()
            ),
        }
    }
}

impl Error for MessageStoreError {}

/// Records retired by maintenance or explicit clearing.
///
/// Callers keep this value alive until after every World resource borrow and
/// interpreter-independent lock is released. Dropping it can release the final
/// backend strong reference and therefore must happen in a valid interpreter
/// context.
#[derive(Debug)]
pub struct RetiredMessages<V> {
    records: Vec<(MessageChannelId, MessageRecord<V>)>,
}

impl<V> Default for RetiredMessages<V> {
    fn default() -> Self {
        Self {
            records: Vec::new(),
        }
    }
}

impl<V> RetiredMessages<V> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn into_records(self) -> Vec<(MessageChannelId, MessageRecord<V>)> {
        self.records
    }
}

/// Context-free snapshot used by a backend iterator.
///
/// Snapshot creation does not consume records. The adapter calls
/// [`MessageStore::consume_snapshot_record`] for each item it is about to yield.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageSnapshot<V> {
    pub channel: MessageChannelId,
    pub records: Vec<MessageRecord<V>>,
}

/// Result of attempting to consume one record from a previously created snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageConsumeOutcome {
    Consumed,
    AlreadyConsumed,
}

/// App-local store for dynamically typed Python messages.
///
/// Backend adapters use context-free strong handles for `V` (an `Arc` around
/// the interpreter's owned object handle). Cloning `V` while the log is locked
/// must not enter the interpreter, and adapters must drain returned
/// [`RetiredMessages`] before the final store handle is destroyed.
#[derive(Resource)]
pub struct MessageStore<V: Send + Sync + 'static> {
    inner: Arc<Mutex<MessageLog<V>>>,
}

impl<V: Send + Sync + 'static> Clone for MessageStore<V> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<V: Send + Sync + 'static> Default for MessageStore<V> {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MessageLog::default())),
        }
    }
}

impl<V: Send + Sync + 'static> MessageStore<V> {
    fn lock_log(&self) -> MutexGuard<'_, MessageLog<V>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_cursor(cursor: &SharedMessageCursor) -> MutexGuard<'_, MessageCursor> {
        cursor
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn channel(
        log: &MessageLog<V>,
        channel: MessageChannelId,
    ) -> Result<&ChannelLog<V>, MessageStoreError> {
        log.channels
            .get(&channel)
            .ok_or(MessageStoreError::ChannelNotRegistered(channel))
    }

    fn channel_mut(
        log: &mut MessageLog<V>,
        channel: MessageChannelId,
    ) -> Result<&mut ChannelLog<V>, MessageStoreError> {
        log.channels
            .get_mut(&channel)
            .ok_or(MessageStoreError::ChannelNotRegistered(channel))
    }

    fn normalize_cursor(
        channel_id: MessageChannelId,
        cursor: &mut MessageCursor,
        channel: &ChannelLog<V>,
    ) -> Result<(), MessageStoreError> {
        if cursor.next_sequence >= channel.dropped_before {
            return Ok(());
        }
        let newly_missed = channel.dropped_before - cursor.next_sequence;
        let missed = cursor
            .missed
            .checked_add(newly_missed)
            .ok_or(MessageStoreError::CursorMissedOverflow(channel_id))?;
        cursor.next_sequence = channel.dropped_before;
        cursor.missed = missed;
        Ok(())
    }

    /// Add an empty channel. Returns `false` when it already exists.
    pub fn register_channel(&self, channel: MessageChannelId) -> bool {
        let mut log = self.lock_log();
        if log.channels.contains_key(&channel) {
            return false;
        }
        log.channels.insert(channel, ChannelLog::default());
        true
    }

    #[must_use]
    pub fn contains_channel(&self, channel: MessageChannelId) -> bool {
        self.lock_log().channels.contains_key(&channel)
    }

    /// Append one value and return its channel-qualified public identity.
    pub fn append(
        &self,
        channel_id: MessageChannelId,
        value: V,
    ) -> Result<PythonMessageId, MessageStoreError> {
        let mut log = self.lock_log();
        let channel = Self::channel_mut(&mut log, channel_id)?;
        let next = channel
            .next_sequence
            .checked_add(1)
            .ok_or(MessageStoreError::SequenceOverflow(channel_id))?;
        let sequence = channel.next_sequence;
        channel.current.push_back(MessageRecord { sequence, value });
        channel.next_sequence = next;
        Ok(PythonMessageId {
            channel: channel_id,
            sequence,
        })
    }

    /// Append a complete prevalidated batch atomically.
    pub fn append_batch(
        &self,
        channel_id: MessageChannelId,
        values: Vec<V>,
    ) -> Result<Vec<PythonMessageId>, MessageStoreError> {
        let mut log = self.lock_log();
        let channel = Self::channel_mut(&mut log, channel_id)?;
        let count = u64::try_from(values.len())
            .map_err(|_| MessageStoreError::SequenceOverflow(channel_id))?;
        let next = channel
            .next_sequence
            .checked_add(count)
            .ok_or(MessageStoreError::SequenceOverflow(channel_id))?;
        let start = channel.next_sequence;
        let mut ids = Vec::with_capacity(values.len());
        for (offset, value) in values.into_iter().enumerate() {
            let offset = u64::try_from(offset)
                .expect("batch length was already proven representable as u64");
            let sequence = start + offset;
            channel.current.push_back(MessageRecord { sequence, value });
            ids.push(PythonMessageId {
                channel: channel_id,
                sequence,
            });
        }
        channel.next_sequence = next;
        Ok(ids)
    }

    /// Rotate every channel once, matching one admitted `Messages<T>::update()`.
    #[must_use]
    pub fn advance(&self) -> RetiredMessages<V> {
        let mut retired = Vec::new();
        // Declare retired storage before the guard so unwind drops the guard
        // first and only then releases any moved backend values.
        let mut log = self.lock_log();
        let retirement_count = log.channels.values().fold(0usize, |count, channel| {
            count.saturating_add(channel.previous.len())
        });
        // Reserve before moving values out of the log. Subsequent exact-size
        // extends cannot allocate and unwind with backend values in temporaries.
        retired.reserve(retirement_count);
        for (channel_id, channel) in &mut log.channels {
            retired.extend(
                std::mem::take(&mut channel.previous)
                    .into_iter()
                    .map(|record| (*channel_id, record)),
            );
            channel.previous = std::mem::take(&mut channel.current);
            channel.dropped_before = channel
                .previous
                .front()
                .map_or(channel.next_sequence, |record| record.sequence);
        }
        RetiredMessages { records: retired }
    }

    /// Clear one channel without resetting its monotonic sequence.
    pub fn clear_channel(
        &self,
        channel_id: MessageChannelId,
    ) -> Result<RetiredMessages<V>, MessageStoreError> {
        // Declare retired storage before the guard; see `advance`.
        let mut records = Vec::new();
        let mut log = self.lock_log();
        let channel = Self::channel_mut(&mut log, channel_id)?;
        records.reserve(channel.previous.len() + channel.current.len());
        records.extend(
            std::mem::take(&mut channel.previous)
                .into_iter()
                .map(|record| (channel_id, record)),
        );
        records.extend(
            std::mem::take(&mut channel.current)
                .into_iter()
                .map(|record| (channel_id, record)),
        );
        channel.dropped_before = channel.next_sequence;
        Ok(RetiredMessages { records })
    }

    /// Clear all channels without resetting their monotonic sequences.
    #[must_use]
    pub fn clear_all(&self) -> RetiredMessages<V> {
        // Declare retired storage before the guard; see `advance`.
        let mut records = Vec::new();
        let mut log = self.lock_log();
        let retirement_count = log.channels.values().fold(0usize, |count, channel| {
            count
                .saturating_add(channel.previous.len())
                .saturating_add(channel.current.len())
        });
        records.reserve(retirement_count);
        for (channel_id, channel) in &mut log.channels {
            records.extend(
                std::mem::take(&mut channel.previous)
                    .into_iter()
                    .map(|record| (*channel_id, record)),
            );
            records.extend(
                std::mem::take(&mut channel.current)
                    .into_iter()
                    .map(|record| (*channel_id, record)),
            );
            channel.dropped_before = channel.next_sequence;
        }
        RetiredMessages { records }
    }

    /// Return the retained unread count without consuming available records.
    pub fn unread_len(
        &self,
        channel_id: MessageChannelId,
        cursor: &SharedMessageCursor,
    ) -> Result<u64, MessageStoreError> {
        let mut cursor = Self::lock_cursor(cursor);
        let log = self.lock_log();
        let channel = Self::channel(&log, channel_id)?;
        Self::normalize_cursor(channel_id, &mut cursor, channel)?;
        Ok(channel.next_sequence.saturating_sub(cursor.next_sequence))
    }

    pub fn is_empty(
        &self,
        channel: MessageChannelId,
        cursor: &SharedMessageCursor,
    ) -> Result<bool, MessageStoreError> {
        self.unread_len(channel, cursor).map(|len| len == 0)
    }

    /// Advance only this cursor to the channel tail.
    pub fn clear_reader(
        &self,
        channel_id: MessageChannelId,
        cursor: &SharedMessageCursor,
    ) -> Result<(), MessageStoreError> {
        let mut cursor = Self::lock_cursor(cursor);
        let log = self.lock_log();
        let channel = Self::channel(&log, channel_id)?;
        Self::normalize_cursor(channel_id, &mut cursor, channel)?;
        cursor.next_sequence = channel.next_sequence;
        Ok(())
    }

    /// Snapshot currently unread values without advancing the cursor.
    pub fn snapshot_unread(
        &self,
        channel_id: MessageChannelId,
        cursor: &SharedMessageCursor,
    ) -> Result<MessageSnapshot<V>, MessageStoreError>
    where
        V: Clone,
    {
        let mut cursor = Self::lock_cursor(cursor);
        let log = self.lock_log();
        let channel = Self::channel(&log, channel_id)?;
        Self::normalize_cursor(channel_id, &mut cursor, channel)?;
        let records = channel
            .previous
            .iter()
            .chain(&channel.current)
            .filter(|record| record.sequence >= cursor.next_sequence)
            .cloned()
            .collect();
        Ok(MessageSnapshot {
            channel: channel_id,
            records,
        })
    }

    /// Consume one snapshot item immediately before a backend yields its value.
    pub fn consume_snapshot_record(
        &self,
        channel_id: MessageChannelId,
        cursor: &SharedMessageCursor,
        sequence: u64,
    ) -> Result<MessageConsumeOutcome, MessageStoreError> {
        let mut cursor = Self::lock_cursor(cursor);
        let log = self.lock_log();
        let channel = Self::channel(&log, channel_id)?;
        Self::normalize_cursor(channel_id, &mut cursor, channel)?;
        if sequence < cursor.next_sequence {
            return Ok(MessageConsumeOutcome::AlreadyConsumed);
        }
        if sequence > cursor.next_sequence {
            return Err(MessageStoreError::CursorOutOfOrder {
                channel: channel_id,
                expected: cursor.next_sequence,
                found: sequence,
            });
        }
        if sequence >= channel.next_sequence {
            return Err(MessageStoreError::SnapshotRecordUnavailable {
                channel: channel_id,
                sequence,
            });
        }
        cursor.next_sequence = cursor
            .next_sequence
            .checked_add(1)
            .ok_or(MessageStoreError::SequenceOverflow(channel_id))?;
        Ok(MessageConsumeOutcome::Consumed)
    }

    #[must_use]
    pub fn cursor_state(cursor: &SharedMessageCursor) -> MessageCursor {
        Self::lock_cursor(cursor).clone()
    }
}

/// Scheduler and reload metadata for one logical Python message channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageChannelMetadata {
    pub qualified_name: String,
    pub access_id: ComponentId,
    pub newest_generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MessageAlias {
    channel: MessageChannelId,
    generation: u32,
}

/// Result of registering an exact Python class identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRegisterOutcome {
    Reused(MessageChannelId),
    Aliased(MessageChannelId),
    Registered(MessageChannelId),
}

impl MessageRegisterOutcome {
    #[must_use]
    pub const fn channel(self) -> MessageChannelId {
        match self {
            Self::Reused(channel) | Self::Aliased(channel) | Self::Registered(channel) => channel,
        }
    }
}

/// App-local exact/qualified identity registry with synthetic scheduler metadata.
#[derive(Debug, Default, Resource)]
pub struct MessageRegistryCore {
    by_type: HashMap<MessageTypeKey, MessageAlias>,
    by_qualified_name: HashMap<String, MessageChannelId>,
    channels: HashMap<MessageChannelId, MessageChannelMetadata>,
    next_channel: u32,
}

impl MessageRegistryCore {
    #[must_use]
    pub fn channel_for_type(&self, type_key: MessageTypeKey) -> Option<MessageChannelId> {
        self.by_type.get(&type_key).map(|alias| alias.channel)
    }

    #[must_use]
    pub fn channel_for_qualified_name(&self, name: &str) -> Option<MessageChannelId> {
        self.by_qualified_name.get(name).copied()
    }

    #[must_use]
    pub fn metadata(&self, channel: MessageChannelId) -> Option<&MessageChannelMetadata> {
        self.channels.get(&channel)
    }

    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    #[must_use]
    pub fn alias_count(&self) -> usize {
        self.by_type.len()
    }

    /// Register or refresh one class identity.
    ///
    /// `register_access` is called only for a genuinely new logical channel, so
    /// adapters do not leak unused synthetic `ComponentId`s on reuse or reload.
    pub fn register(
        &mut self,
        type_key: MessageTypeKey,
        qualified_name: &str,
        generation: u32,
        register_access: impl FnOnce(MessageChannelId) -> ComponentId,
    ) -> Result<MessageRegisterOutcome, MessageStoreError> {
        if let Some(alias) = self.by_type.get_mut(&type_key) {
            alias.generation = alias.generation.max(generation);
            if let Some(metadata) = self.channels.get_mut(&alias.channel) {
                metadata.newest_generation = metadata.newest_generation.max(generation);
            }
            return Ok(MessageRegisterOutcome::Reused(alias.channel));
        }

        if let Some(channel) = self.channel_for_qualified_name(qualified_name) {
            self.by_type.insert(
                type_key,
                MessageAlias {
                    channel,
                    generation,
                },
            );
            if let Some(metadata) = self.channels.get_mut(&channel) {
                metadata.newest_generation = metadata.newest_generation.max(generation);
            }
            return Ok(MessageRegisterOutcome::Aliased(channel));
        }

        let next_channel = self
            .next_channel
            .checked_add(1)
            .ok_or(MessageStoreError::ChannelCapacityExhausted)?;
        let channel = MessageChannelId::new(self.next_channel);
        let access_id = register_access(channel);
        self.next_channel = next_channel;
        self.by_type.insert(
            type_key,
            MessageAlias {
                channel,
                generation,
            },
        );
        self.by_qualified_name
            .insert(qualified_name.to_owned(), channel);
        self.channels.insert(
            channel,
            MessageChannelMetadata {
                qualified_name: qualified_name.to_owned(),
                access_id,
                newest_generation: generation,
            },
        );
        Ok(MessageRegisterOutcome::Registered(channel))
    }

    /// Remove stale exact identities while retaining every newest-generation alias.
    ///
    /// Returned keys let the backend remove and later drop its matching strong
    /// class handles after releasing the registry's World resource borrow.
    pub fn prune_aliases(&mut self, minimum_generation: u32) -> Vec<MessageTypeKey> {
        let mut newest_by_channel = HashMap::<MessageChannelId, u32>::new();
        for alias in self.by_type.values() {
            newest_by_channel
                .entry(alias.channel)
                .and_modify(|generation| *generation = (*generation).max(alias.generation))
                .or_insert(alias.generation);
        }

        let mut removed = Vec::new();
        self.by_type.retain(|type_key, alias| {
            let newest = newest_by_channel
                .get(&alias.channel)
                .copied()
                .unwrap_or(alias.generation);
            let keep = alias.generation >= minimum_generation || alias.generation == newest;
            if !keep {
                removed.push(*type_key);
            }
            keep
        });
        removed
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
    };

    use bevy::ecs::world::World;

    use super::*;

    fn channel(raw: u32) -> MessageChannelId {
        MessageChannelId::new(raw)
    }

    fn registered_store() -> (MessageStore<u64>, MessageChannelId) {
        let store = MessageStore::default();
        let channel = channel(0);
        assert!(store.register_channel(channel));
        (store, channel)
    }

    fn snapshot_values(snapshot: &MessageSnapshot<u64>) -> Vec<u64> {
        snapshot.records.iter().map(|record| record.value).collect()
    }

    struct DropProbe(Arc<AtomicUsize>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn store_and_registry_are_bevy_resources() {
        let mut world = World::new();
        world.insert_resource(MessageStore::<u64>::default());
        world.insert_resource(MessageRegistryCore::default());
        assert!(world.contains_resource::<MessageStore<u64>>());
        assert!(world.contains_resource::<MessageRegistryCore>());
    }

    #[test]
    fn channel_registration_and_append_are_idempotent_and_monotonic() {
        let (store, first_channel) = registered_store();
        assert!(!store.register_channel(first_channel));
        let other = channel(1);
        assert!(store.register_channel(other));

        let first = store.append(first_channel, 10).unwrap();
        let second = store.append(first_channel, 20).unwrap();
        let other_first = store.append(other, 30).unwrap();

        assert_eq!(
            (first, second, other_first),
            (
                PythonMessageId {
                    channel: first_channel,
                    sequence: 0,
                },
                PythonMessageId {
                    channel: first_channel,
                    sequence: 1,
                },
                PythonMessageId {
                    channel: other,
                    sequence: 0,
                },
            )
        );
    }

    #[test]
    fn batch_append_is_contiguous_and_overflow_is_atomic() {
        let (store, channel) = registered_store();
        let ids = store.append_batch(channel, vec![10, 20, 30]).unwrap();
        assert_eq!(
            ids.iter().map(|id| id.sequence).collect::<Vec<_>>(),
            [0, 1, 2]
        );

        {
            let mut log = store.lock_log();
            log.channels.get_mut(&channel).unwrap().next_sequence = u64::MAX - 1;
        }
        let error = store.append_batch(channel, vec![40, 50]).unwrap_err();
        assert_eq!(error, MessageStoreError::SequenceOverflow(channel));

        let snapshot = store
            .snapshot_unread(channel, &new_message_cursor())
            .unwrap();
        assert_eq!(snapshot_values(&snapshot), [10, 20, 30]);
        let log = store.lock_log();
        assert_eq!(log.channels[&channel].next_sequence, u64::MAX - 1);
    }

    #[test]
    fn two_advances_retain_then_retire_current_records() {
        let (store, channel) = registered_store();
        store.append(channel, 10).unwrap();

        let first_retired = store.advance();
        assert!(first_retired.is_empty());
        let cursor = new_message_cursor();
        assert_eq!(store.unread_len(channel, &cursor).unwrap(), 1);

        let second_retired = store.advance();
        assert_eq!(second_retired.len(), 1);
        assert_eq!(
            second_retired.into_records()[0],
            (
                channel,
                MessageRecord {
                    sequence: 0,
                    value: 10,
                },
            )
        );
        assert_eq!(store.unread_len(channel, &cursor).unwrap(), 0);
        assert_eq!(MessageStore::<u64>::cursor_state(&cursor).missed, 1);
    }

    #[test]
    fn cursors_are_independent_and_clear_is_reader_local() {
        let (store, channel) = registered_store();
        store.append_batch(channel, vec![1, 2]).unwrap();
        let first = new_message_cursor();
        let second = new_message_cursor();

        assert_eq!(store.unread_len(channel, &first).unwrap(), 2);
        assert_eq!(store.unread_len(channel, &second).unwrap(), 2);
        store.clear_reader(channel, &first).unwrap();
        assert!(store.is_empty(channel, &first).unwrap());
        assert_eq!(store.unread_len(channel, &second).unwrap(), 2);
        assert_eq!(
            snapshot_values(&store.snapshot_unread(channel, &second).unwrap()),
            [1, 2]
        );
    }

    #[test]
    fn snapshot_consumes_per_item_and_preserves_an_abandoned_remainder() {
        let (store, channel) = registered_store();
        store.append_batch(channel, vec![1, 2, 3]).unwrap();
        let cursor = new_message_cursor();
        let snapshot = store.snapshot_unread(channel, &cursor).unwrap();

        assert_eq!(
            store
                .consume_snapshot_record(channel, &cursor, snapshot.records[0].sequence)
                .unwrap(),
            MessageConsumeOutcome::Consumed
        );
        drop(snapshot);

        assert_eq!(store.unread_len(channel, &cursor).unwrap(), 2);
        assert_eq!(
            snapshot_values(&store.snapshot_unread(channel, &cursor).unwrap()),
            [2, 3]
        );
    }

    #[test]
    fn concurrent_snapshots_cannot_double_consume() {
        let (store, channel) = registered_store();
        store.append_batch(channel, vec![1, 2]).unwrap();
        let cursor = new_message_cursor();
        let first = store.snapshot_unread(channel, &cursor).unwrap();
        let second = store.snapshot_unread(channel, &cursor).unwrap();

        assert_eq!(
            store
                .consume_snapshot_record(channel, &cursor, first.records[0].sequence)
                .unwrap(),
            MessageConsumeOutcome::Consumed
        );
        assert_eq!(
            store
                .consume_snapshot_record(channel, &cursor, second.records[0].sequence)
                .unwrap(),
            MessageConsumeOutcome::AlreadyConsumed
        );
        assert_eq!(
            store
                .consume_snapshot_record(channel, &cursor, second.records[1].sequence)
                .unwrap(),
            MessageConsumeOutcome::Consumed
        );
        assert!(store.is_empty(channel, &cursor).unwrap());
    }

    #[test]
    fn out_of_order_or_unavailable_snapshot_records_fail_closed() {
        let (store, channel) = registered_store();
        store.append_batch(channel, vec![1, 2]).unwrap();
        let cursor = new_message_cursor();

        assert_eq!(
            store
                .consume_snapshot_record(channel, &cursor, 1)
                .unwrap_err(),
            MessageStoreError::CursorOutOfOrder {
                channel,
                expected: 0,
                found: 1,
            }
        );
        assert_eq!(MessageStore::<u64>::cursor_state(&cursor).next_sequence, 0);

        store.clear_reader(channel, &cursor).unwrap();
        assert_eq!(
            store
                .consume_snapshot_record(channel, &cursor, 2)
                .unwrap_err(),
            MessageStoreError::SnapshotRecordUnavailable {
                channel,
                sequence: 2,
            }
        );
        assert_eq!(MessageStore::<u64>::cursor_state(&cursor).next_sequence, 2);
    }

    #[test]
    fn missed_counts_are_channel_local_amid_interleaved_writes() {
        let store = MessageStore::default();
        let first = channel(0);
        let second = channel(1);
        store.register_channel(first);
        store.register_channel(second);
        let first_cursor = new_message_cursor();
        let second_cursor = new_message_cursor();

        store.append_batch(first, vec![1, 2]).unwrap();
        store.append_batch(second, vec![10, 20, 30]).unwrap();
        let _ = store.advance();
        store.append(second, 40).unwrap();
        let _ = store.advance();

        assert_eq!(store.unread_len(first, &first_cursor).unwrap(), 0);
        assert_eq!(store.unread_len(second, &second_cursor).unwrap(), 1);
        assert_eq!(MessageStore::<u64>::cursor_state(&first_cursor).missed, 2);
        assert_eq!(MessageStore::<u64>::cursor_state(&second_cursor).missed, 3);
        assert_eq!(
            snapshot_values(&store.snapshot_unread(second, &second_cursor).unwrap()),
            [40]
        );
    }

    #[test]
    fn clearing_returns_values_and_preserves_channel_sequences() {
        let store = MessageStore::default();
        let first = channel(0);
        let second = channel(1);
        store.register_channel(first);
        store.register_channel(second);
        store.append_batch(first, vec![1, 2]).unwrap();
        store.append(second, 3).unwrap();

        let retired = store.clear_channel(first).unwrap();
        assert_eq!(retired.len(), 2);
        assert_eq!(store.append(first, 4).unwrap().sequence, 2);
        assert_eq!(store.unread_len(first, &new_message_cursor()).unwrap(), 1);

        let all_retired = store.clear_all();
        assert_eq!(all_retired.len(), 2);
        assert_eq!(store.append(first, 5).unwrap().sequence, 3);
        assert_eq!(store.append(second, 6).unwrap().sequence, 1);
    }

    #[test]
    fn retired_values_are_returned_for_out_of_lock_destruction() {
        let store = MessageStore::default();
        let channel = channel(0);
        let drops = Arc::new(AtomicUsize::new(0));
        store.register_channel(channel);
        store
            .append(channel, DropProbe(Arc::clone(&drops)))
            .unwrap();

        drop(store.advance());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let retired = store.advance();
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(retired);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn poisoned_store_and_cursor_locks_recover() {
        let (store, channel) = registered_store();
        let poisoned_store = store.clone();
        thread::spawn(move || {
            let _guard = poisoned_store.inner.lock().unwrap();
            panic!("intentional store poison");
        })
        .join()
        .unwrap_err();
        assert_eq!(store.append(channel, 1).unwrap().sequence, 0);

        let cursor = new_message_cursor();
        let poisoned_cursor = Arc::clone(&cursor);
        thread::spawn(move || {
            let _guard = poisoned_cursor.lock().unwrap();
            panic!("intentional cursor poison");
        })
        .join()
        .unwrap_err();
        assert_eq!(store.unread_len(channel, &cursor).unwrap(), 1);
    }

    #[test]
    fn concurrent_appends_allocate_gapless_channel_sequences() {
        const THREADS: usize = 8;
        const WRITES: usize = 100;

        let (store, channel) = registered_store();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let mut workers = Vec::new();
        for worker in 0..THREADS {
            let store = store.clone();
            let observed = Arc::clone(&observed);
            workers.push(thread::spawn(move || {
                for offset in 0..WRITES {
                    let value = u64::try_from(worker * WRITES + offset).unwrap();
                    let id = store.append(channel, value).unwrap();
                    observed.lock().unwrap().push(id.sequence);
                }
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }

        let mut sequences = observed.lock().unwrap().clone();
        sequences.sort_unstable();
        assert_eq!(
            sequences,
            (0..u64::try_from(THREADS * WRITES).unwrap()).collect::<Vec<_>>()
        );
        let snapshot = store
            .snapshot_unread(channel, &new_message_cursor())
            .unwrap();
        assert_eq!(snapshot.records.len(), THREADS * WRITES);
        assert!(
            snapshot
                .records
                .windows(2)
                .all(|pair| pair[0].sequence + 1 == pair[1].sequence)
        );
    }

    #[test]
    fn registry_reuses_exact_identity_without_allocating_an_access_id() {
        let mut registry = MessageRegistryCore::default();
        let allocations = AtomicUsize::new(0);
        let first = registry
            .register(MessageTypeKey::new(1), "game.Hit", 0, |_| {
                allocations.fetch_add(1, Ordering::SeqCst);
                ComponentId::new(10)
            })
            .unwrap();
        let second = registry
            .register(MessageTypeKey::new(1), "changed.Name", 2, |_| {
                allocations.fetch_add(1, Ordering::SeqCst);
                ComponentId::new(11)
            })
            .unwrap();

        assert_eq!(first, MessageRegisterOutcome::Registered(channel(0)));
        assert_eq!(second, MessageRegisterOutcome::Reused(channel(0)));
        assert_eq!(allocations.load(Ordering::SeqCst), 1);
        assert_eq!(registry.metadata(channel(0)).unwrap().newest_generation, 2);
    }

    #[test]
    fn qualified_redefinition_aliases_but_same_bare_name_stays_distinct() {
        let mut registry = MessageRegistryCore::default();
        let original = registry
            .register(MessageTypeKey::new(1), "alpha.Hit", 0, |_| {
                ComponentId::new(10)
            })
            .unwrap();
        let alias = registry
            .register(MessageTypeKey::new(2), "alpha.Hit", 1, |_| {
                ComponentId::new(11)
            })
            .unwrap();
        let distinct = registry
            .register(MessageTypeKey::new(3), "beta.Hit", 1, |_| {
                ComponentId::new(12)
            })
            .unwrap();

        assert_eq!(alias, MessageRegisterOutcome::Aliased(original.channel()));
        assert_eq!(distinct, MessageRegisterOutcome::Registered(channel(1)));
        assert_ne!(original.channel(), distinct.channel());
        assert_eq!(registry.channel_count(), 2);
        assert_eq!(registry.alias_count(), 3);
        assert_eq!(
            registry.metadata(channel(0)).unwrap().access_id,
            ComponentId::new(10)
        );
    }

    #[test]
    fn pruning_returns_stale_keys_and_keeps_the_newest_alias_per_channel() {
        let mut registry = MessageRegistryCore::default();
        registry
            .register(MessageTypeKey::new(1), "game.Hit", 0, |_| {
                ComponentId::new(10)
            })
            .unwrap();
        registry
            .register(MessageTypeKey::new(2), "game.Hit", 1, |_| {
                ComponentId::new(11)
            })
            .unwrap();
        registry
            .register(MessageTypeKey::new(3), "game.Hit", 3, |_| {
                ComponentId::new(12)
            })
            .unwrap();

        let removed = registry
            .prune_aliases(2)
            .into_iter()
            .collect::<HashSet<_>>();
        assert_eq!(
            removed,
            HashSet::from([MessageTypeKey::new(1), MessageTypeKey::new(2)])
        );
        assert_eq!(registry.alias_count(), 1);
        assert_eq!(
            registry.channel_for_type(MessageTypeKey::new(3)),
            Some(channel(0))
        );

        assert!(registry.prune_aliases(10).is_empty());
        assert_eq!(registry.alias_count(), 1);
        assert_eq!(
            registry.channel_for_qualified_name("game.Hit"),
            Some(channel(0))
        );
    }

    #[test]
    fn registry_capacity_failure_does_not_allocate_or_mutate() {
        let mut registry = MessageRegistryCore {
            next_channel: u32::MAX,
            ..Default::default()
        };
        let allocations = AtomicUsize::new(0);
        let error = registry
            .register(MessageTypeKey::new(1), "game.Hit", 0, |_| {
                allocations.fetch_add(1, Ordering::SeqCst);
                ComponentId::new(10)
            })
            .unwrap_err();

        assert_eq!(error, MessageStoreError::ChannelCapacityExhausted);
        assert_eq!(allocations.load(Ordering::SeqCst), 0);
        assert_eq!(registry.channel_count(), 0);
        assert_eq!(registry.alias_count(), 0);
    }
}
