use std::fmt;

use bevy::{
    app::{
        App, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last,
        Main, MainScheduleOrder, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update,
    },
    ecs::{
        resource::Resource,
        schedule::{InternedScheduleLabel, ScheduleLabel},
        world::World,
    },
};

/// Exact identity of one interpreter-defined state type.
///
/// PyO3 adapters use `PyTypeObject* as usize`; RustPython adapters use the
/// corresponding interpreter object id. The value is opaque to the shared
/// schedule layer and is only meaningful for one interpreter lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateMachineId(usize);

impl StateMachineId {
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Whether a state schedule runs when entering or exiting a state value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateScheduleKind {
    Enter,
    Exit,
}

/// Interpreter-neutral label for `OnEnter` and `OnExit` schedules.
///
/// The machine id prevents equal enum-member hashes from aliasing schedules
/// belonging to different state types.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateScheduleLabel {
    machine_id: StateMachineId,
    kind: StateScheduleKind,
    state_hash: u64,
}

impl StateScheduleLabel {
    #[must_use]
    pub const fn on_enter(machine_id: StateMachineId, state_hash: u64) -> Self {
        Self {
            machine_id,
            kind: StateScheduleKind::Enter,
            state_hash,
        }
    }

    #[must_use]
    pub const fn on_exit(machine_id: StateMachineId, state_hash: u64) -> Self {
        Self {
            machine_id,
            kind: StateScheduleKind::Exit,
            state_hash,
        }
    }

    #[must_use]
    pub const fn machine_id(&self) -> StateMachineId {
        self.machine_id
    }

    #[must_use]
    pub const fn kind(&self) -> StateScheduleKind {
        self.kind
    }

    #[must_use]
    pub const fn state_hash(&self) -> u64 {
        self.state_hash
    }
}

/// Interpreter-neutral label for `OnTransition` schedules.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransitionScheduleLabel {
    machine_id: StateMachineId,
    exit_hash: u64,
    enter_hash: u64,
}

impl TransitionScheduleLabel {
    #[must_use]
    pub const fn new(machine_id: StateMachineId, exit_hash: u64, enter_hash: u64) -> Self {
        Self {
            machine_id,
            exit_hash,
            enter_hash,
        }
    }

    #[must_use]
    pub const fn machine_id(&self) -> StateMachineId {
        self.machine_id
    }

    #[must_use]
    pub const fn exit_hash(&self) -> u64 {
        self.exit_hash
    }

    #[must_use]
    pub const fn enter_hash(&self) -> u64 {
        self.enter_hash
    }
}

/// Custom PyBevy schedule that runs between `PreUpdate` and `Update`.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimTick;

/// Interpreter-neutral identity for every built-in schedule exposed by PyBevy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleKind {
    Startup,
    PreStartup,
    PostStartup,
    First,
    PreUpdate,
    Update,
    PostUpdate,
    Last,
    FixedFirst,
    FixedPreUpdate,
    FixedUpdate,
    FixedPostUpdate,
    FixedLast,
    Main,
    SimTick,
}

impl ScheduleKind {
    pub const ALL: [(&'static str, Self); 15] = [
        ("Startup", Self::Startup),
        ("PreStartup", Self::PreStartup),
        ("PostStartup", Self::PostStartup),
        ("First", Self::First),
        ("PreUpdate", Self::PreUpdate),
        ("Update", Self::Update),
        ("PostUpdate", Self::PostUpdate),
        ("Last", Self::Last),
        ("FixedFirst", Self::FixedFirst),
        ("FixedPreUpdate", Self::FixedPreUpdate),
        ("FixedUpdate", Self::FixedUpdate),
        ("FixedPostUpdate", Self::FixedPostUpdate),
        ("FixedLast", Self::FixedLast),
        ("Main", Self::Main),
        ("SimTick", Self::SimTick),
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Startup => "Startup",
            Self::PreStartup => "PreStartup",
            Self::PostStartup => "PostStartup",
            Self::First => "First",
            Self::PreUpdate => "PreUpdate",
            Self::Update => "Update",
            Self::PostUpdate => "PostUpdate",
            Self::Last => "Last",
            Self::FixedFirst => "FixedFirst",
            Self::FixedPreUpdate => "FixedPreUpdate",
            Self::FixedUpdate => "FixedUpdate",
            Self::FixedPostUpdate => "FixedPostUpdate",
            Self::FixedLast => "FixedLast",
            Self::Main => "Main",
            Self::SimTick => "SimTick",
        }
    }

    pub const fn is_startup(self) -> bool {
        matches!(self, Self::Startup | Self::PreStartup | Self::PostStartup)
    }

    pub fn intern_label(self) -> InternedScheduleLabel {
        match self {
            Self::Startup => Startup.intern(),
            Self::PreStartup => PreStartup.intern(),
            Self::PostStartup => PostStartup.intern(),
            Self::First => First.intern(),
            Self::PreUpdate => PreUpdate.intern(),
            Self::Update => Update.intern(),
            Self::PostUpdate => PostUpdate.intern(),
            Self::Last => Last.intern(),
            Self::FixedFirst => FixedFirst.intern(),
            Self::FixedPreUpdate => FixedPreUpdate.intern(),
            Self::FixedUpdate => FixedUpdate.intern(),
            Self::FixedPostUpdate => FixedPostUpdate.intern(),
            Self::FixedLast => FixedLast.intern(),
            Self::Main => Main.intern(),
            Self::SimTick => SimTick.intern(),
        }
    }

    pub fn run_on_world(self, world: &mut World) {
        world.run_schedule(self.intern_label());
    }

    pub fn init_on_app(self, app: &mut App) {
        app.init_schedule(self.intern_label());
    }
}

impl fmt::Display for ScheduleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Resource)]
struct StandardSchedulesConfigured;

/// Install schedules and ordering shared by every PyBevy app.
///
/// The marker makes this safe for native runners and WASM bootstrap paths to
/// call even when an app may already have been configured by Python.
pub fn configure_standard_schedules(app: &mut App) {
    if app
        .world()
        .contains_resource::<StandardSchedulesConfigured>()
    {
        return;
    }

    app.init_schedule(SimTick);
    app.world_mut()
        .resource_mut::<MainScheduleOrder>()
        .insert_after(PreUpdate, SimTick);
    app.insert_resource(StandardSchedulesConfigured);
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bevy::ecs::schedule::Schedules;

    use super::*;

    #[test]
    fn all_schedule_names_and_labels_are_unique() {
        let names: HashSet<_> = ScheduleKind::ALL.iter().map(|(name, _)| *name).collect();
        let labels: HashSet<_> = ScheduleKind::ALL
            .iter()
            .map(|(_, kind)| kind.intern_label())
            .collect();

        assert_eq!(names.len(), ScheduleKind::ALL.len());
        assert_eq!(labels.len(), ScheduleKind::ALL.len());
        for (name, kind) in ScheduleKind::ALL {
            assert_eq!(kind.name(), name);
            assert_eq!(kind.to_string(), name);
        }
    }

    #[test]
    fn startup_classification_matches_the_three_startup_schedules() {
        let startup: Vec<_> = ScheduleKind::ALL
            .iter()
            .filter_map(|(_, kind)| kind.is_startup().then_some(*kind))
            .collect();

        assert_eq!(
            startup,
            vec![
                ScheduleKind::Startup,
                ScheduleKind::PreStartup,
                ScheduleKind::PostStartup,
            ]
        );
    }

    #[test]
    fn standard_schedule_configuration_is_idempotent() {
        let mut app = App::new();

        configure_standard_schedules(&mut app);
        configure_standard_schedules(&mut app);

        let schedules = app.world().resource::<Schedules>();
        assert!(schedules.contains(SimTick));
        assert!(
            app.world()
                .contains_resource::<StandardSchedulesConfigured>()
        );
    }

    #[test]
    fn state_schedule_labels_include_machine_identity() {
        let first = StateMachineId::new(1);
        let second = StateMachineId::new(2);

        assert_eq!(
            StateScheduleLabel::on_enter(first, 42),
            StateScheduleLabel::on_enter(first, 42)
        );
        assert_ne!(
            StateScheduleLabel::on_enter(first, 42),
            StateScheduleLabel::on_enter(second, 42)
        );
        assert_ne!(
            StateScheduleLabel::on_enter(first, 42),
            StateScheduleLabel::on_exit(first, 42)
        );
    }

    #[test]
    fn transition_schedule_labels_include_machine_identity() {
        let first = StateMachineId::new(1);
        let second = StateMachineId::new(2);

        assert_eq!(
            TransitionScheduleLabel::new(first, 10, 20),
            TransitionScheduleLabel::new(first, 10, 20)
        );
        assert_ne!(
            TransitionScheduleLabel::new(first, 10, 20),
            TransitionScheduleLabel::new(second, 10, 20)
        );
        assert_ne!(
            TransitionScheduleLabel::new(first, 10, 20),
            TransitionScheduleLabel::new(first, 10, 30)
        );
    }
}
