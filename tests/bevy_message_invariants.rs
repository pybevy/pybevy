//! Bevy 0.19 buffered-message invariants relevant to PyBevy's custom-message store.
//!
//! These tests pin upstream behavior before either binding backend is migrated.
//! They exercise native Bevy only; no Python objects or PyBevy message adapters
//! participate in the contract.

use std::panic::{AssertUnwindSafe, catch_unwind};

use bevy::{
    ecs::{
        message::{
            MessageCursor, MessageMutator, MessageReader, MessageRegistry, MessageUpdateSystems,
            MessageWriter, Messages, ShouldUpdateMessages, message_update_condition,
            message_update_system, signal_message_update_system,
        },
        system::{RunSystemOnce, SystemState},
    },
    prelude::*,
};

#[derive(Message, Clone, Debug, PartialEq, Eq)]
struct ContractMessage(u32);

#[derive(Message, Clone, Debug, PartialEq, Eq)]
struct OtherMessage(u32);

#[derive(Clone, Debug, PartialEq, Eq)]
enum TraceStep {
    Wrote { id: usize, value: u32 },
    Read(u32),
}

#[derive(Resource, Default)]
struct MessageTrace(Vec<TraceStep>);

#[derive(Resource, Default)]
struct MaintenanceRuns(usize);

fn ordered_writer(mut writer: MessageWriter<ContractMessage>, mut trace: ResMut<MessageTrace>) {
    let id = writer.write(ContractMessage(7));
    trace.0.push(TraceStep::Wrote {
        id: id.id,
        value: 7,
    });
}

fn ordered_reader(mut reader: MessageReader<ContractMessage>, mut trace: ResMut<MessageTrace>) {
    trace
        .0
        .extend(reader.read().map(|message| TraceStep::Read(message.0)));
}

fn maintenance_probe(mut runs: ResMut<MaintenanceRuns>) {
    runs.0 += 1;
}

#[test]
fn ordered_writer_is_visible_to_reader_in_the_same_schedule_pass() {
    let mut app = App::new();
    app.add_message::<ContractMessage>()
        .init_resource::<MessageTrace>()
        .add_systems(Update, (ordered_writer, ordered_reader).chain());

    app.update();

    assert_eq!(
        app.world().resource::<MessageTrace>().0,
        [TraceStep::Wrote { id: 0, value: 7 }, TraceStep::Read(7),]
    );
}

#[test]
fn messages_expire_after_two_update_calls() {
    let mut messages = Messages::<ContractMessage>::default();
    messages.write(ContractMessage(1));

    messages.update();
    let mut after_one_update = messages.get_cursor();
    assert_eq!(
        after_one_update
            .read(&messages)
            .map(|message| message.0)
            .collect::<Vec<_>>(),
        [1]
    );

    messages.update();
    let after_two_updates = messages.get_cursor();
    assert_eq!(after_two_updates.len(&messages), 0);
    assert_eq!(after_two_updates.missed_messages(&messages), 1);
    assert!(messages.is_empty());
}

#[test]
fn default_cursor_includes_retained_messages_and_current_cursor_skips_them() {
    let mut messages = Messages::<ContractMessage>::default();
    messages.write(ContractMessage(10));

    let mut default_cursor = messages.get_cursor();
    let mut current_cursor = messages.get_cursor_current();
    assert_eq!(default_cursor.len(&messages), 1);
    assert!(current_cursor.is_empty(&messages));

    messages.write(ContractMessage(20));
    assert_eq!(
        default_cursor
            .read(&messages)
            .map(|message| message.0)
            .collect::<Vec<_>>(),
        [10, 20]
    );
    assert_eq!(
        current_cursor
            .read(&messages)
            .map(|message| message.0)
            .collect::<Vec<_>>(),
        [20]
    );
}

#[test]
fn readers_have_independent_cursors() {
    let mut messages = Messages::<ContractMessage>::default();
    messages.write_batch([ContractMessage(1), ContractMessage(2)]);
    let mut first = messages.get_cursor();
    let mut second = messages.get_cursor();

    assert_eq!(
        first
            .read(&messages)
            .map(|message| message.0)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(first.len(&messages), 0);
    assert_eq!(second.len(&messages), 2);
    assert_eq!(
        second
            .read(&messages)
            .map(|message| message.0)
            .collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn len_is_empty_and_clear_are_reader_local_and_non_destructive() {
    let mut messages = Messages::<ContractMessage>::default();
    messages.write_batch([ContractMessage(1), ContractMessage(2)]);
    let mut cleared_reader = messages.get_cursor();
    let untouched_reader = messages.get_cursor();

    assert_eq!(cleared_reader.len(&messages), 2);
    assert!(!cleared_reader.is_empty(&messages));
    cleared_reader.clear(&messages);

    assert!(cleared_reader.is_empty(&messages));
    assert_eq!(untouched_reader.len(&messages), 2);
    assert_eq!(messages.len(), 2);
}

#[test]
fn dropping_a_partial_iterator_leaves_the_remainder_unread() {
    let mut messages = Messages::<ContractMessage>::default();
    messages.write_batch([ContractMessage(1), ContractMessage(2), ContractMessage(3)]);
    let mut cursor = messages.get_cursor();

    {
        let mut iterator = cursor.read(&messages);
        assert_eq!(iterator.next(), Some(&ContractMessage(1)));
    }

    assert_eq!(cursor.len(&messages), 2);
    assert_eq!(
        cursor
            .read(&messages)
            .map(|message| message.0)
            .collect::<Vec<_>>(),
        [2, 3]
    );
}

#[test]
fn message_ids_are_monotonic_within_each_typed_channel() {
    let mut first_channel = Messages::<ContractMessage>::default();
    let mut second_channel = Messages::<OtherMessage>::default();

    let first_zero = first_channel.write(ContractMessage(1));
    let first_one = first_channel.write(ContractMessage(2));
    let second_zero = second_channel.write(OtherMessage(3));

    assert_eq!((first_zero.id, first_one.id), (0, 1));
    assert_eq!(second_zero.id, 0);
    assert_eq!(format!("{first_zero:?}"), "message<ContractMessage>#0");
    assert_eq!(format!("{second_zero:?}"), "message<OtherMessage>#0");
}

#[test]
fn missing_messages_resource_is_a_parameter_validation_error() {
    let mut world = World::new();
    let mut state = SystemState::<MessageReader<ContractMessage>>::new(&mut world);

    let error = state.get(&world).unwrap_err();
    assert!(
        error.to_string().contains("Message not initialized"),
        "unexpected validation error: {error}"
    );
}

#[test]
fn same_type_reader_writer_conflicts_and_message_mutator_is_the_alternative() {
    let mut world = World::new();
    MessageRegistry::register_message::<ContractMessage>(&mut world);

    let conflict = catch_unwind(AssertUnwindSafe(|| {
        SystemState::<(
            MessageReader<ContractMessage>,
            MessageWriter<ContractMessage>,
        )>::new(&mut world)
    }));
    assert!(conflict.is_err());

    let mut mutator_state = SystemState::<MessageMutator<ContractMessage>>::new(&mut world);
    let mut mutator = mutator_state.get_mut(&mut world).unwrap();
    mutator.write(ContractMessage(4));
    assert_eq!(
        mutator.read().map(|message| message.0).collect::<Vec<_>>(),
        [4]
    );
}

#[test]
fn message_mutator_reads_writes_and_mutates_with_bevy_cursor_timing() {
    let mut world = World::new();
    MessageRegistry::register_message::<ContractMessage>(&mut world);
    world.write_message(ContractMessage(1));

    let mut state = SystemState::<MessageMutator<ContractMessage>>::new(&mut world);
    {
        let mut mutator = state.get_mut(&mut world).unwrap();
        let before_read = mutator.write(ContractMessage(2));
        assert_eq!(before_read.id, 1);
        for message in mutator.read() {
            message.0 += 10;
        }
        let after_read = mutator.write(ContractMessage(3));
        assert_eq!(after_read.id, 2);
        assert_eq!(mutator.len(), 1);
    }

    let messages = world.resource::<Messages<ContractMessage>>();
    let mut observer = messages.get_cursor();
    assert_eq!(
        observer
            .read(messages)
            .map(|message| message.0)
            .collect::<Vec<_>>(),
        [11, 12, 3]
    );

    let mut mutator = state.get_mut(&mut world).unwrap();
    assert_eq!(
        mutator.read().map(|message| message.0).collect::<Vec<_>>(),
        [3]
    );
    assert!(mutator.is_empty());
}

#[test]
fn message_mutator_clear_is_cursor_local_and_non_destructive() {
    let mut world = World::new();
    MessageRegistry::register_message::<ContractMessage>(&mut world);
    world.write_message_batch([ContractMessage(1), ContractMessage(2)]);
    let untouched = world.resource::<Messages<ContractMessage>>().get_cursor();

    let mut state = SystemState::<MessageMutator<ContractMessage>>::new(&mut world);
    let mut mutator = state.get_mut(&mut world).unwrap();
    assert_eq!(mutator.len(), 2);
    mutator.clear();
    assert!(mutator.is_empty());
    drop(mutator);

    let messages = world.resource::<Messages<ContractMessage>>();
    assert_eq!(untouched.len(messages), 2);
    assert_eq!(messages.len(), 2);
}

#[test]
fn message_mutator_has_exclusive_same_channel_access_only() {
    let mut world = World::new();
    MessageRegistry::register_message::<ContractMessage>(&mut world);
    MessageRegistry::register_message::<OtherMessage>(&mut world);

    let reader_conflict = catch_unwind(AssertUnwindSafe(|| {
        SystemState::<(
            MessageMutator<ContractMessage>,
            MessageReader<ContractMessage>,
        )>::new(&mut world)
    }));
    assert!(reader_conflict.is_err());

    let writer_conflict = catch_unwind(AssertUnwindSafe(|| {
        SystemState::<(
            MessageMutator<ContractMessage>,
            MessageWriter<ContractMessage>,
        )>::new(&mut world)
    }));
    assert!(writer_conflict.is_err());

    let mutator_conflict = catch_unwind(AssertUnwindSafe(|| {
        SystemState::<(
            MessageMutator<ContractMessage>,
            MessageMutator<ContractMessage>,
        )>::new(&mut world)
    }));
    assert!(mutator_conflict.is_err());

    SystemState::<(MessageMutator<ContractMessage>, MessageReader<OtherMessage>)>::new(&mut world);
}

#[test]
fn fixed_update_signal_gates_rotation_and_bevy_updater_consumes_ready() {
    let mut app = App::new();
    app.add_message::<ContractMessage>()
        .init_resource::<MaintenanceRuns>()
        .add_systems(
            First,
            maintenance_probe
                .in_set(MessageUpdateSystems)
                .before(message_update_system)
                .run_if(message_update_condition),
        );

    app.world_mut()
        .resource_mut::<MessageRegistry>()
        .should_update = ShouldUpdateMessages::Waiting;
    app.world_mut().write_message(ContractMessage(9));
    let cursor = MessageCursor::<ContractMessage>::default();

    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<MaintenanceRuns>().0, 0);
    assert_eq!(
        cursor.len(app.world().resource::<Messages<ContractMessage>>()),
        1
    );

    app.world_mut()
        .run_system_once(signal_message_update_system)
        .unwrap();
    assert_eq!(
        app.world().resource::<MessageRegistry>().should_update,
        ShouldUpdateMessages::Ready
    );

    app.update();
    assert_eq!(app.world().resource::<MaintenanceRuns>().0, 1);
    assert_eq!(
        app.world().resource::<MessageRegistry>().should_update,
        ShouldUpdateMessages::Waiting
    );
    assert_eq!(
        cursor.len(app.world().resource::<Messages<ContractMessage>>()),
        1
    );

    app.world_mut()
        .run_system_once(signal_message_update_system)
        .unwrap();
    app.update();
    assert_eq!(app.world().resource::<MaintenanceRuns>().0, 2);
    assert_eq!(
        cursor.len(app.world().resource::<Messages<ContractMessage>>()),
        0
    );
    assert_eq!(
        cursor.missed_messages(app.world().resource::<Messages<ContractMessage>>()),
        1
    );
}
