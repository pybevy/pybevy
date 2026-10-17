use std::{any::Any, collections::HashMap, fmt};

use bevy::{
    app::{
        App, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last,
        Main, MainScheduleOrder, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update,
    },
    ecs::{
        resource::Resource,
        schedule::{InternedScheduleLabel, InternedSystemSet, ScheduleLabel, SystemSet},
        world::World,
    },
};

/// Bevy panics rather than returning when a schedule's graph cannot be built.
const SCHEDULE_BUILD_PANIC: &str = "Error when initializing schedule ";

/// Recognize a Bevy schedule-build panic and return its message.
///
/// Classifying the payload needs no interpreter, so both backends share it and
/// each adapter picks its own exception type for the message.
#[must_use]
pub fn schedule_build_message(payload: &(dyn Any + Send)) -> Option<String> {
    let text = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&'static str>().copied())?;
    text.starts_with(SCHEDULE_BUILD_PANIC)
        .then(|| text.to_string())
}

/// Interpreter-neutral boolean expression over backend-owned condition leaves.
///
/// Each leaf is lowered independently so backends preserve its exact scheduler
/// access and parameter plan instead of forcing all predicates through one
/// callable signature.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConditionExpr<T> {
    Leaf(T),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Not(Box<Self>),
}

impl<T> ConditionExpr<T> {
    #[must_use]
    pub fn map_ref<U>(&self, f: &mut impl FnMut(&T) -> U) -> ConditionExpr<U> {
        match self {
            Self::Leaf(leaf) => ConditionExpr::Leaf(f(leaf)),
            Self::And(left, right) => {
                ConditionExpr::And(Box::new(left.map_ref(f)), Box::new(right.map_ref(f)))
            }
            Self::Or(left, right) => {
                ConditionExpr::Or(Box::new(left.map_ref(f)), Box::new(right.map_ref(f)))
            }
            Self::Not(condition) => ConditionExpr::Not(Box::new(condition.map_ref(f))),
        }
    }

    /// Number of combiner nodes on the longest root-to-leaf path.
    ///
    /// Callers only measure expressions already bounded by the construction
    /// cap, so the recursion stays shallow.
    #[must_use]
    pub fn depth(&self) -> usize {
        match self {
            Self::Leaf(_) => 0,
            Self::And(left, right) | Self::Or(left, right) => 1 + left.depth().max(right.depth()),
            Self::Not(condition) => 1 + condition.depth(),
        }
    }

    pub fn for_each_leaf(&self, f: &mut impl FnMut(&T)) {
        match self {
            Self::Leaf(leaf) => f(leaf),
            Self::And(left, right) | Self::Or(left, right) => {
                left.for_each_leaf(f);
                right.for_each_leaf(f);
            }
            Self::Not(condition) => condition.for_each_leaf(f),
        }
    }
}

/// Namespace for a dynamic Python scheduler identity.
///
/// User-declared sets and callable identities deliberately occupy different
/// namespaces so a set named after a function cannot accidentally alias that
/// function's ordering target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicSetKind {
    Named,
    Callable,
}

/// Interpreter-neutral Bevy system-set label for Python scheduling.
///
/// Adapters derive `qualified_name` from stable Python metadata such as
/// `__module__` and `__qualname__`. The label owns no interpreter object or raw
/// type pointer, so recreating a class or function during hot reload produces
/// the same scheduler identity.
#[derive(SystemSet, Clone, PartialEq, Eq, Hash)]
pub struct DynamicSetLabel {
    kind: DynamicSetKind,
    qualified_name: String,
}

/// Bevy prints set labels with `Debug` in scheduler diagnostics, so print the name.
impl fmt::Debug for DynamicSetLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DynamicSetKind::Named => write!(formatter, "SystemSet(\"{}\")", self.qualified_name),
            DynamicSetKind::Callable => formatter.write_str(&self.qualified_name),
        }
    }
}

impl DynamicSetLabel {
    #[must_use]
    pub fn named(qualified_name: impl Into<String>) -> Self {
        Self {
            kind: DynamicSetKind::Named,
            qualified_name: qualified_name.into(),
        }
    }

    #[must_use]
    pub fn callable(qualified_name: impl Into<String>) -> Self {
        Self {
            kind: DynamicSetKind::Callable,
            qualified_name: qualified_name.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> DynamicSetKind {
        self.kind
    }

    #[must_use]
    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }
}

/// Interpreter-neutral target for Python-defined and native Bevy system sets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SystemSetTarget {
    Dynamic(DynamicSetLabel),
    Native {
        label: InternedSystemSet,
        qualified_name: String,
    },
}

impl SystemSetTarget {
    #[must_use]
    pub fn dynamic(label: DynamicSetLabel) -> Self {
        Self::Dynamic(label)
    }

    #[must_use]
    pub fn native(label: InternedSystemSet, qualified_name: impl Into<String>) -> Self {
        Self::Native {
            label,
            qualified_name: qualified_name.into(),
        }
    }

    #[must_use]
    pub fn qualified_name(&self) -> &str {
        match self {
            Self::Dynamic(label) => label.qualified_name(),
            Self::Native { qualified_name, .. } => qualified_name,
        }
    }

    #[must_use]
    pub fn intern(&self) -> InternedSystemSet {
        match self {
            Self::Dynamic(label) => label.clone().intern(),
            Self::Native { label, .. } => *label,
        }
    }
}

impl From<DynamicSetLabel> for SystemSetTarget {
    fn from(value: DynamicSetLabel) -> Self {
        Self::dynamic(value)
    }
}

/// Interpreter-neutral ordering metadata attached to a Python system or set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ScheduleOrdering {
    pub in_sets: Vec<SystemSetTarget>,
    pub before: Vec<SystemSetTarget>,
    pub after: Vec<SystemSetTarget>,
}

impl ScheduleOrdering {
    pub fn push_in_set(&mut self, set: SystemSetTarget) {
        push_unique(&mut self.in_sets, set);
    }

    pub fn push_before(&mut self, set: SystemSetTarget) {
        push_unique(&mut self.before, set);
    }

    pub fn push_after(&mut self, set: SystemSetTarget) {
        push_unique(&mut self.after, set);
    }

    /// Return the first `before`/`after` target that is one of `members`, the
    /// callable sets the configured system itself joins.
    #[must_use]
    pub fn relation_to_member<'a>(
        &'a self,
        members: &[DynamicSetLabel],
    ) -> Option<(&'static str, &'a DynamicSetLabel)> {
        let member = |target: &'a SystemSetTarget| match target {
            SystemSetTarget::Dynamic(label) if members.contains(label) => Some(label),
            _ => None,
        };
        self.before
            .iter()
            .find_map(member)
            .map(|label| ("before", label))
            .or_else(|| {
                self.after
                    .iter()
                    .find_map(member)
                    .map(|label| ("after", label))
            })
    }
}

fn push_unique(labels: &mut Vec<SystemSetTarget>, label: SystemSetTarget) {
    if !labels.contains(&label) {
        labels.push(label);
    }
}

/// App-local identity of one interpreter-defined state machine.
///
/// Raw adapter keys are canonicalized to registry-issued IDs before execution.
/// Registry IDs remain distinct when an interpreter reuses a class address.
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

/// Exact identity of one state value within its machine.
///
/// Named members use their name. Unnamed values, such as a `Flag`'s zero
/// value, use a value representation; the tag keeps the two forms disjoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StateMemberKey {
    Named(String),
    Unnamed(String),
}

impl From<&str> for StateMemberKey {
    fn from(name: &str) -> Self {
        Self::Named(name.to_owned())
    }
}

impl From<&StateMemberKey> for StateMemberKey {
    fn from(key: &StateMemberKey) -> Self {
        key.clone()
    }
}

/// Interpreter-neutral label for `OnEnter` and `OnExit` schedules.
///
/// The machine id prevents equal member keys from aliasing schedules that
/// belong to different state types. The member key survives class redefinition.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateScheduleLabel {
    machine_id: StateMachineId,
    kind: StateScheduleKind,
    member: StateMemberKey,
}

impl StateScheduleLabel {
    #[must_use]
    pub fn on_enter(machine_id: StateMachineId, member: impl Into<StateMemberKey>) -> Self {
        Self {
            machine_id,
            kind: StateScheduleKind::Enter,
            member: member.into(),
        }
    }

    #[must_use]
    pub fn on_exit(machine_id: StateMachineId, member: impl Into<StateMemberKey>) -> Self {
        Self {
            machine_id,
            kind: StateScheduleKind::Exit,
            member: member.into(),
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
    pub fn member(&self) -> &StateMemberKey {
        &self.member
    }
}

/// Interpreter-neutral label for `OnTransition` schedules.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransitionScheduleLabel {
    machine_id: StateMachineId,
    exit_member: StateMemberKey,
    enter_member: StateMemberKey,
}

impl TransitionScheduleLabel {
    #[must_use]
    pub fn new(
        machine_id: StateMachineId,
        exit_member: impl Into<StateMemberKey>,
        enter_member: impl Into<StateMemberKey>,
    ) -> Self {
        Self {
            machine_id,
            exit_member: exit_member.into(),
            enter_member: enter_member.into(),
        }
    }

    #[must_use]
    pub const fn machine_id(&self) -> StateMachineId {
        self.machine_id
    }

    #[must_use]
    pub fn exit_member(&self) -> &StateMemberKey {
        &self.exit_member
    }

    #[must_use]
    pub fn enter_member(&self) -> &StateMemberKey {
        &self.enter_member
    }
}

/// App-local public names for schedules whose exact identity is not readable.
///
/// Backend adapters lower interpreter metadata into owned strings and register
/// it against the already-interned Bevy label. The registry does not influence
/// schedule identity or retain interpreter objects.
#[derive(Resource, Default)]
pub struct ScheduleDisplayNameRegistry {
    names: HashMap<InternedScheduleLabel, String>,
}

impl ScheduleDisplayNameRegistry {
    pub fn insert(&mut self, label: InternedScheduleLabel, name: impl Into<String>) {
        self.names.insert(label, name.into());
    }

    #[must_use]
    pub fn get(&self, label: InternedScheduleLabel) -> Option<&str> {
        self.names.get(&label).map(String::as_str)
    }

    /// Look up the exact interned label exposed by [`Schedules::iter`].
    #[must_use]
    pub fn get_by_label(&self, label: &dyn ScheduleLabel) -> Option<&str> {
        self.names.iter().find_map(|(registered, name)| {
            std::ptr::eq::<dyn ScheduleLabel>(registered.0, label).then_some(name.as_str())
        })
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
            StateScheduleLabel::on_enter(first, "READY"),
            StateScheduleLabel::on_enter(first, "READY")
        );
        assert_ne!(
            StateScheduleLabel::on_enter(first, "READY"),
            StateScheduleLabel::on_enter(second, "READY")
        );
        assert_ne!(
            StateScheduleLabel::on_enter(first, "READY"),
            StateScheduleLabel::on_exit(first, "READY")
        );
    }

    #[test]
    fn dynamic_set_namespaces_do_not_alias() {
        let named = DynamicSetLabel::named("game.Movement");
        let callable = DynamicSetLabel::callable("game.Movement");

        assert_ne!(named, callable);
        assert_eq!(named.kind(), DynamicSetKind::Named);
        assert_eq!(named.qualified_name(), "game.Movement");
    }

    #[test]
    fn schedule_build_message_matches_only_the_bevy_graph_panic() {
        let owned: Box<dyn Any + Send> =
            Box::new("Error when initializing schedule Update: cycle".to_string());
        assert_eq!(
            schedule_build_message(owned.as_ref()).as_deref(),
            Some("Error when initializing schedule Update: cycle")
        );

        let borrowed: Box<dyn Any + Send> = Box::new("Error when initializing schedule Main");
        assert_eq!(
            schedule_build_message(borrowed.as_ref()).as_deref(),
            Some("Error when initializing schedule Main")
        );

        let unrelated: Box<dyn Any + Send> = Box::new("Main not found".to_string());
        assert_eq!(schedule_build_message(unrelated.as_ref()), None);

        let not_a_string: Box<dyn Any + Send> = Box::new(7u32);
        assert_eq!(schedule_build_message(not_a_string.as_ref()), None);
    }

    #[test]
    fn dynamic_set_debug_prints_the_qualified_name() {
        assert_eq!(
            format!("{:?}", DynamicSetLabel::callable("game.advance")),
            "game.advance"
        );
        assert_eq!(
            format!("{:?}", DynamicSetLabel::named("game.Movement")),
            "SystemSet(\"game.Movement\")"
        );
    }

    #[test]
    fn schedule_ordering_deduplicates_labels() {
        let movement = DynamicSetLabel::named("game.Movement");
        let input = DynamicSetLabel::named("game.Input");
        let mut ordering = ScheduleOrdering::default();

        ordering.push_in_set(movement.clone().into());
        ordering.push_in_set(movement.clone().into());
        ordering.push_after(input.clone().into());
        ordering.push_after(input.into());

        assert_eq!(ordering.in_sets, vec![movement.into()]);
        assert_eq!(ordering.after.len(), 1);
    }

    #[test]
    fn transition_schedule_labels_include_machine_identity() {
        let first = StateMachineId::new(1);
        let second = StateMachineId::new(2);

        assert_eq!(
            TransitionScheduleLabel::new(first, "READY", "RUNNING"),
            TransitionScheduleLabel::new(first, "READY", "RUNNING")
        );
        assert_ne!(
            TransitionScheduleLabel::new(first, "READY", "RUNNING"),
            TransitionScheduleLabel::new(second, "READY", "RUNNING")
        );
        assert_ne!(
            TransitionScheduleLabel::new(first, "READY", "RUNNING"),
            TransitionScheduleLabel::new(first, "READY", "PAUSED")
        );
    }

    #[test]
    fn schedule_display_names_use_exact_labels_and_owned_values() {
        let first = StateScheduleLabel::on_enter(StateMachineId::new(1), "READY").intern();
        let second = StateScheduleLabel::on_enter(StateMachineId::new(2), "READY").intern();
        let mut registry = ScheduleDisplayNameRegistry::default();
        let mut name = String::from("OnEnter(game.First.READY)");

        registry.insert(first, name.clone());
        name.clear();
        registry.insert(second, "OnEnter(game.Second.READY)");

        assert_eq!(registry.get(first), Some("OnEnter(game.First.READY)"));
        assert_eq!(registry.get(second), Some("OnEnter(game.Second.READY)"));
        assert_eq!(
            registry.get_by_label(first.0),
            Some("OnEnter(game.First.READY)")
        );
        assert_eq!(registry.get(Update.intern()), None);
    }
}
