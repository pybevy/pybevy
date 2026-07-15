//! Interpreter-neutral execution primitives for dynamic Python systems.
//!
//! This module owns the Bevy-facing system shell and the unsafe contract that
//! interpreter adapters must satisfy. It deliberately contains no
//! interpreter-specific types. Main continues to use its existing `DynamicSystem` until
//! the adapter migration; the core here is exercised with a fake interpreter
//! so its ordering and safety boundaries can be reviewed independently.

use std::{
    collections::HashSet,
    marker::PhantomData,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use bevy::{
    ecs::{
        change_detection::{CheckChangeTicks, Tick},
        entity::Entity,
        query::FilteredAccessSet,
        system::{
            ReadOnlySystem, RunSystemError, System, SystemIn, SystemParamValidationError,
            SystemStateFlags,
        },
        world::{CommandQueue, DeferredWorld, World, WorldId, unsafe_world_cell::UnsafeWorldCell},
    },
    platform::time::Instant,
    prelude::{DebugName, Resource, Time},
};
use pybevy_storage::{ValidityFlag, ValidityGuard};

use super::{
    parity_trace::{ParityOpSink, ParityRunHandle, ParityTraceResource},
    run_scaffold::RunTicks,
    system_flags::compute_system_flags,
};

/// Stage in which a dynamic system was registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemStage {
    Startup,
    UpdateOrLast,
}

/// Active hot-reload generation shared by schedule conditions and dynamic systems.
#[derive(Resource, Clone)]
pub struct HotReloadGeneration {
    pub current: u32,
    generation_counter: Arc<AtomicU32>,
    startup_run_for_generations: Arc<Mutex<HashSet<u32>>>,
}

impl HotReloadGeneration {
    pub fn new(generation_counter: Arc<AtomicU32>) -> Self {
        Self {
            current: generation_counter.load(Ordering::SeqCst),
            generation_counter,
            startup_run_for_generations: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn update(&mut self) {
        self.current = self.generation_counter.load(Ordering::SeqCst);
    }

    pub fn mark_startup_run(&self) {
        if let Ok(mut set) = self.startup_run_for_generations.lock() {
            set.insert(self.current);
        }
    }

    pub fn has_startup_run(&self, generation: u32) -> bool {
        self.startup_run_for_generations
            .lock()
            .map(|set| set.contains(&generation))
            .unwrap_or(false)
    }

    /// Forget a failed generation so a later retry is allowed to run Startup.
    pub fn forget_startup_run(&self, generation: u32) {
        if let Ok(mut set) = self.startup_run_for_generations.lock() {
            set.remove(&generation);
        }
    }

    /// Retain only recent Startup markers after a successful reload.
    pub fn retain_startup_runs_since(&self, minimum_generation: u32) {
        if let Ok(mut set) = self.startup_run_for_generations.lock() {
            set.retain(|generation| *generation >= minimum_generation);
        }
    }
}

/// Run when the active generation matches, or when hot reload is disabled.
pub fn generation_matches(
    expected_generation: u32,
) -> impl FnMut(Option<bevy::ecs::system::Res<HotReloadGeneration>>) -> bool + Clone {
    move |generation_res| {
        generation_res
            .as_ref()
            .is_none_or(|generation| generation.current == expected_generation)
    }
}

/// Run Startup once for the matching hot-reload generation.
pub fn startup_or_reload(
    expected_generation: u32,
) -> impl FnMut(Option<bevy::ecs::system::Res<HotReloadGeneration>>) -> bool + Clone {
    move |generation_res| {
        generation_res.as_ref().is_none_or(|generation| {
            generation.current == expected_generation
                && !generation.has_startup_run(expected_generation)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvocationKind {
    System,
    Condition,
    Observer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMode {
    Unit,
    Bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallOutcome {
    Unit,
    Bool(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorPolicy {
    RaiseAfterUpdate,
    ReportAndContinue,
    PropagateToCaller,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredErrorPolicy {
    RaiseAfterUpdate,
    ReportAndContinue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorReport {
    pub message: String,
    pub traceback: Option<String>,
}

#[derive(Debug)]
pub struct InterpreterFailure<Token> {
    pub report: ErrorReport,
    pub exception: Option<Token>,
}

pub enum CallablePreflight<Call, Token> {
    Ready(Call),
    Retired,
    Failed(InterpreterFailure<Token>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallMetadata {
    pub name: String,
    pub kind: InvocationKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SystemFlags {
    pub needs_exclusive: bool,
    pub needs_commands: bool,
}

pub struct TriggerContext<'a, Event> {
    pub event: &'a Event,
    pub target: Option<Entity>,
}

pub struct InterpreterCallContext<'w, 'a, Event> {
    pub world: UnsafeWorldCell<'w>,
    pub ticks: RunTicks,
    pub validity: &'a ValidityFlag,
    pub commands: &'a mut CommandQueue,
    pub trigger: Option<TriggerContext<'a, Event>>,
    pub output: OutputMode,
    pub kind: InvocationKind,
    pub error_policy: ErrorPolicy,
    pub parity_trace: Option<&'a ParityRunHandle>,
}

pub struct InitializedRunState<State, Sink> {
    pub state: State,
    pub failure_sink: Sink,
    pub access: FilteredAccessSet,
    pub validation: Option<SystemParamValidationError>,
}

pub type SystemHandle<B> = Arc<Mutex<<B as SystemInterpreter>::RetainedState>>;

/// Interpreter adapter contract for the shared dynamic-system core.
///
/// # Safety
///
/// Implementations must declare every ECS access performed while building or
/// calling parameters, use the supplied validity flag for every run-scoped raw
/// pointer, and prevent world cells or wrapper pointers from escaping the
/// validity window. They must format failures in the originating interpreter
/// context before taking a sink lock. No retained-state, registry, or sink lock
/// may be held while invoking interpreter code or dropping the final
/// interpreter reference. Each call method consumes `PreparedCall` and must
/// drop it before leaving the interpreter context, including during unwinding.
pub unsafe trait SystemInterpreter: Send + Sync + 'static {
    type Event: Send + Sync + 'static;
    type PreparedCall: Send + Sync + 'static;
    type ParamPlan: Send + Sync + 'static;
    type ScheduledRunState: Send + Sync + 'static;
    type ObserverPersistentState: Send + Sync + 'static;
    type ObserverRunState: Send + 'static;
    type RetainedState: Send + 'static;
    type ExceptionToken: Send + 'static;
    type FailureSink: Clone + Send + Sync + 'static;

    fn initialize_scheduled(
        &self,
        params: &Self::ParamPlan,
        world: &mut World,
    ) -> InitializedRunState<Self::ScheduledRunState, Self::FailureSink>;

    fn validate_condition(&self, params: &Self::ParamPlan) -> Result<(), String>;

    fn resolve_callable(
        &self,
        retained: &SystemHandle<Self>,
        current_generation: Option<u32>,
    ) -> CallablePreflight<Self::PreparedCall, Self::ExceptionToken>;

    fn make_observer_run_state(
        &self,
        params: &Self::ParamPlan,
        persistent: &Self::ObserverPersistentState,
        world: &mut World,
    ) -> Self::ObserverRunState;

    /// Return the stable Python-visible type name for one observer event.
    fn observer_event_type_name(&self, event: &Self::Event) -> String;

    /// Build scheduled arguments and invoke one prepared call.
    ///
    /// # Safety
    /// The caller must provide the World cell, ticks, access, validity flag,
    /// and command queue established for this system run. The implementation
    /// obligations are the ones documented on [`SystemInterpreter`].
    unsafe fn build_args_and_call_scheduled(
        &self,
        prepared: Self::PreparedCall,
        params: &Self::ParamPlan,
        state: &mut Self::ScheduledRunState,
        ctx: InterpreterCallContext<'_, '_, Self::Event>,
    ) -> Result<CallOutcome, InterpreterFailure<Self::ExceptionToken>>;

    /// Build fresh observer arguments and invoke one prepared call.
    ///
    /// # Safety
    /// The caller must hold exclusive access to the supplied World for the
    /// complete call and queue-application sequence. Run-scoped values must use
    /// `ctx.validity` and be dropped before returning.
    unsafe fn build_args_and_call_observer(
        &self,
        prepared: Self::PreparedCall,
        params: &Self::ParamPlan,
        state: &mut Self::ObserverRunState,
        ctx: InterpreterCallContext<'_, '_, Self::Event>,
    ) -> Result<CallOutcome, InterpreterFailure<Self::ExceptionToken>>;

    fn store_failure(
        &self,
        sink: &Self::FailureSink,
        failure: InterpreterFailure<Self::ExceptionToken>,
        metadata: &CallMetadata,
        policy: StoredErrorPolicy,
    );

    fn retire(&self, retained: &SystemHandle<Self>);
}

pub trait OutputPolicy: Send + Sync + 'static {
    type Out: 'static;
    const MODE: OutputMode;

    fn skipped() -> Self::Out;
    fn finish(outcome: CallOutcome) -> Self::Out;
}

pub struct UnitOutput;

impl OutputPolicy for UnitOutput {
    type Out = ();
    const MODE: OutputMode = OutputMode::Unit;

    fn skipped() -> Self::Out {}

    fn finish(_outcome: CallOutcome) -> Self::Out {}
}

pub struct BoolOutput;

impl OutputPolicy for BoolOutput {
    type Out = bool;
    const MODE: OutputMode = OutputMode::Bool;

    fn skipped() -> Self::Out {
        false
    }

    fn finish(outcome: CallOutcome) -> Self::Out {
        match outcome {
            CallOutcome::Bool(value) => value,
            CallOutcome::Unit => {
                panic!("SystemInterpreter returned unit output for a bool condition")
            }
        }
    }
}

pub trait RunProfileSink: Send + Sync + 'static {
    fn record(
        &self,
        system_name: &str,
        duration: Duration,
        stage: SystemStage,
        app_time_seconds: f64,
    );
}

#[derive(Resource, Clone)]
pub struct RunProfileSinkResource(pub Arc<dyn RunProfileSink>);

pub struct PreparedSystem<B: SystemInterpreter> {
    pub interpreter: B,
    pub retained: SystemHandle<B>,
    pub params: B::ParamPlan,
    pub metadata: CallMetadata,
    pub flags: SystemFlags,
    pub stage: SystemStage,
    pub expected_generation: Option<u32>,
}

pub struct DynamicSystemCore<B: SystemInterpreter, O: OutputPolicy> {
    interpreter: B,
    retained: SystemHandle<B>,
    params: B::ParamPlan,
    state: Option<B::ScheduledRunState>,
    metadata: CallMetadata,
    flags: SystemFlags,
    stage: SystemStage,
    expected_generation: Option<u32>,
    last_run: Tick,
    command_queue: CommandQueue,
    validation: Option<SystemParamValidationError>,
    failure_sink: Option<B::FailureSink>,
    profiler: Option<Arc<dyn RunProfileSink>>,
    world_id: Option<WorldId>,
    parity_trace: Option<Arc<ParityOpSink>>,
    pending_trace_runs: Vec<ParityRunHandle>,
    next_trace_run_index: u64,
    _output: PhantomData<O>,
}

impl<B: SystemInterpreter, O: OutputPolicy> DynamicSystemCore<B, O> {
    pub fn new(prepared: PreparedSystem<B>) -> Self {
        Self {
            interpreter: prepared.interpreter,
            retained: prepared.retained,
            params: prepared.params,
            state: None,
            metadata: prepared.metadata,
            flags: prepared.flags,
            stage: prepared.stage,
            expected_generation: prepared.expected_generation,
            last_run: Tick::new(0),
            command_queue: CommandQueue::default(),
            validation: None,
            failure_sink: None,
            profiler: None,
            world_id: None,
            parity_trace: None,
            pending_trace_runs: Vec::new(),
            next_trace_run_index: 0,
            _output: PhantomData,
        }
    }

    pub fn handle(&self) -> &SystemHandle<B> {
        &self.retained
    }

    /// Update the profiling stage before this system is inserted into a schedule.
    pub fn set_stage(&mut self, stage: SystemStage) {
        self.stage = stage;
    }

    /// Add the defense-in-depth generation guard before schedule insertion.
    pub fn set_expected_generation(&mut self, generation: u32) {
        self.expected_generation = Some(generation);
    }

    fn sink(&self) -> &B::FailureSink {
        self.failure_sink
            .as_ref()
            .expect("DynamicSystemCore must be initialized before it runs")
    }

    fn store_failure(
        &self,
        failure: InterpreterFailure<B::ExceptionToken>,
        policy: StoredErrorPolicy,
    ) {
        self.interpreter
            .store_failure(self.sink(), failure, &self.metadata, policy);
    }
}

impl<B: SystemInterpreter, O: OutputPolicy> System for DynamicSystemCore<B, O> {
    type In = ();
    type Out = O::Out;

    fn name(&self) -> DebugName {
        DebugName::owned(self.metadata.name.clone())
    }

    fn flags(&self) -> SystemStateFlags {
        compute_system_flags(self.flags.needs_exclusive, self.flags.needs_commands)
    }

    unsafe fn run_unsafe(
        &mut self,
        _input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        if let Some(validation) = &self.validation {
            return Err(validation.clone().into());
        }
        assert_eq!(
            self.world_id,
            Some(world.id()),
            "Encountered a mismatched World. A System cannot be used with Worlds other than the one it was initialized with."
        );

        // SAFETY: initialize declares this resource read for non-exclusive
        // systems; exclusive systems own the whole World.
        let current_generation = unsafe { world.get_resource::<HotReloadGeneration>() }
            .map(|generation| generation.current);
        if let (Some(expected), Some(current)) = (self.expected_generation, current_generation)
            && expected != current
        {
            return Ok(O::skipped());
        }

        let prepared = match self
            .interpreter
            .resolve_callable(&self.retained, current_generation)
        {
            CallablePreflight::Ready(prepared) => prepared,
            CallablePreflight::Retired => return Ok(O::skipped()),
            CallablePreflight::Failed(failure) => {
                self.store_failure(failure, StoredErrorPolicy::RaiseAfterUpdate);
                return Ok(O::skipped());
            }
        };

        world.increment_change_tick();
        let this_run = world.change_tick();
        let ticks = RunTicks {
            last_run: self.last_run,
            this_run,
        };
        let mut local_queue = CommandQueue::default();
        let trace_run = if self.metadata.kind == InvocationKind::System {
            self.parity_trace.as_ref().map(|sink| {
                let run_index = self.next_trace_run_index;
                self.next_trace_run_index += 1;
                sink.start_run(&self.metadata.name, run_index)
                    .unwrap_or_else(|error| panic!("{error}"))
            })
        } else {
            None
        };
        let started = Instant::now();

        let call_result = {
            let validity = ValidityFlag::new();
            let validity_guard = ValidityGuard::new(validity.clone());
            let state = self
                .state
                .as_mut()
                .expect("DynamicSystemCore must be initialized before it runs");
            let ctx = InterpreterCallContext {
                world,
                ticks,
                validity: &validity,
                commands: &mut local_queue,
                trigger: None,
                output: O::MODE,
                kind: self.metadata.kind,
                error_policy: ErrorPolicy::RaiseAfterUpdate,
                parity_trace: trace_run.as_ref(),
            };
            // SAFETY: the scheduler enforces the access returned by initialize;
            // this core owns the single validity flag and local queue for the
            // run, and the unsafe backend contract requires all arguments to be
            // dropped before returning.
            let result = unsafe {
                self.interpreter
                    .build_args_and_call_scheduled(prepared, &self.params, state, ctx)
            };
            drop(validity_guard);
            result
        };
        let duration = started.elapsed();

        self.command_queue.append(&mut local_queue);
        if let Some(trace_run) = trace_run {
            self.pending_trace_runs.push(trace_run);
        }

        let output = match call_result {
            Ok(outcome) => O::finish(outcome),
            Err(failure) => {
                self.store_failure(failure, StoredErrorPolicy::RaiseAfterUpdate);
                O::skipped()
            }
        };

        if let Some(profiler) = &self.profiler {
            // SAFETY: initialize declares the Time read whenever a profiler is
            // captured. Exclusive systems own the complete World.
            let app_time_seconds = unsafe { world.get_resource::<Time>() }
                .map(|time| time.elapsed_secs_f64())
                .unwrap_or(0.0);
            profiler.record(&self.metadata.name, duration, self.stage, app_time_seconds);
        }

        self.last_run = this_run;
        Ok(output)
    }

    fn apply_deferred(&mut self, world: &mut World) {
        let resolved = self
            .parity_trace
            .as_ref()
            .map(|sink| sink.resolve_before_flush(&self.pending_trace_runs, world));
        self.command_queue.apply(world);
        if let (Some(sink), Some(resolved)) = (&self.parity_trace, resolved) {
            let resolved = resolved.unwrap_or_else(|error| panic!("{error}"));
            sink.record_flushed(&resolved)
                .unwrap_or_else(|error| panic!("{error}"));
        }
        self.pending_trace_runs.clear();
    }

    fn queue_deferred(&mut self, _world: DeferredWorld) {}

    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        if let Some(world_id) = self.world_id {
            assert_eq!(
                world_id,
                world.id(),
                "Encountered a mismatched World. A System cannot be initialized with multiple Worlds."
            );
        }
        self.world_id = Some(world.id());

        self.parity_trace = parity_trace_sink(world);

        let initialized = self.interpreter.initialize_scheduled(&self.params, world);
        self.state = Some(initialized.state);
        self.failure_sink = Some(initialized.failure_sink);
        self.validation = initialized.validation;
        self.profiler = world
            .get_resource::<RunProfileSinkResource>()
            .map(|resource| resource.0.clone());

        if self.flags.needs_exclusive {
            return FilteredAccessSet::default();
        }

        let mut access = initialized.access;
        access.add_resource_read(world.register_component::<HotReloadGeneration>());
        if self.profiler.is_some() {
            access.add_resource_read(world.register_component::<Time>());
        }
        access
    }

    fn check_change_tick(&mut self, check: CheckChangeTicks) {
        self.last_run.check_tick(check);
    }

    fn get_last_run(&self) -> Tick {
        self.last_run
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.last_run = last_run;
    }
}

pub struct DynamicConditionCore<B: SystemInterpreter>(DynamicSystemCore<B, BoolOutput>);

impl<B: SystemInterpreter> DynamicConditionCore<B> {
    pub fn new(prepared: PreparedSystem<B>) -> Result<Self, String> {
        prepared.interpreter.validate_condition(&prepared.params)?;
        Ok(Self(DynamicSystemCore::new(prepared)))
    }
}

impl<B: SystemInterpreter> System for DynamicConditionCore<B> {
    type In = ();
    type Out = bool;

    fn name(&self) -> DebugName {
        self.0.name()
    }

    fn flags(&self) -> SystemStateFlags {
        self.0.flags()
    }

    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        // SAFETY: the condition constructor validates the stronger read-only
        // contract and the inner system has the same initialized access.
        unsafe { self.0.run_unsafe(input, world) }
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.0.apply_deferred(world);
    }

    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.0.queue_deferred(world);
    }

    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        self.0.initialize(world)
    }

    fn check_change_tick(&mut self, check: CheckChangeTicks) {
        self.0.check_change_tick(check);
    }

    fn get_last_run(&self) -> Tick {
        self.0.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.0.set_last_run(last_run);
    }
}

// SAFETY: `DynamicConditionCore::new` accepts only parameter plans that the
// unsafe interpreter implementation certifies as read-only. Its own
// generation/profiling infrastructure accesses are reads.
unsafe impl<B: SystemInterpreter> ReadOnlySystem for DynamicConditionCore<B> {}

pub type ObserverDispatchResult<B> =
    Result<(), InterpreterFailure<<B as SystemInterpreter>::ExceptionToken>>;

/// Execute one already-selected observer without retaining registry borrows.
///
/// # Safety
/// `world` must be the exclusive World for this dispatch. `params` and
/// `persistent` must describe `retained`, and the unsafe interpreter contract
/// must be upheld for the complete invocation.
#[allow(clippy::too_many_arguments)]
pub unsafe fn execute_observer<B: SystemInterpreter>(
    interpreter: &B,
    retained: &SystemHandle<B>,
    params: &B::ParamPlan,
    persistent: &B::ObserverPersistentState,
    failure_sink: &B::FailureSink,
    metadata: &CallMetadata,
    current_generation: Option<u32>,
    event: &B::Event,
    target: Option<Entity>,
    policy: ErrorPolicy,
    world: &mut World,
) -> ObserverDispatchResult<B> {
    let prepared = match interpreter.resolve_callable(retained, current_generation) {
        CallablePreflight::Ready(prepared) => prepared,
        CallablePreflight::Retired => return Ok(()),
        CallablePreflight::Failed(failure) => {
            return finish_observer_failure(interpreter, failure_sink, metadata, policy, failure);
        }
    };
    let mut state = interpreter.make_observer_run_state(params, persistent, world);
    let ticks = RunTicks {
        last_run: Tick::new(0),
        this_run: world.change_tick(),
    };
    let mut local_queue = CommandQueue::default();
    let trace_sink = parity_trace_sink(world);
    let trace_run = trace_sink.as_ref().map(|sink| {
        let trigger_type = interpreter.observer_event_type_name(event);
        sink.start_observer(&metadata.name, &trigger_type, target, world)
            .unwrap_or_else(|error| panic!("{error}"))
    });

    let call_result = {
        let validity = ValidityFlag::new();
        let validity_guard = ValidityGuard::new(validity.clone());
        let ctx = InterpreterCallContext {
            world: world.as_unsafe_world_cell(),
            ticks,
            validity: &validity,
            commands: &mut local_queue,
            trigger: Some(TriggerContext { event, target }),
            output: OutputMode::Unit,
            kind: InvocationKind::Observer,
            error_policy: policy,
            parity_trace: trace_run.as_ref().map(|trace| trace.run_handle()),
        };
        // SAFETY: the caller supplies exclusive World access, this function
        // owns the fresh state/validity/queue, and the backend contract requires
        // all run-scoped arguments to be dropped before returning.
        let result =
            unsafe { interpreter.build_args_and_call_observer(prepared, params, &mut state, ctx) };
        drop(validity_guard);
        result
    };

    let resolved = trace_sink
        .as_ref()
        .zip(trace_run.as_ref())
        .map(|(sink, trace)| {
            sink.resolve_observer_before_flush(trace, world)
                .unwrap_or_else(|error| panic!("{error}"))
        });
    local_queue.apply(world);
    if let (Some(sink), Some(trace), Some(resolved)) =
        (trace_sink.as_ref(), trace_run.as_ref(), resolved.as_ref())
    {
        sink.record_observer_flushed(trace, resolved)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    match call_result {
        Ok(_) => Ok(()),
        Err(failure) => {
            finish_observer_failure(interpreter, failure_sink, metadata, policy, failure)
        }
    }
}

fn parity_trace_sink(world: &mut World) -> Option<Arc<ParityOpSink>> {
    if !world.contains_resource::<ParityTraceResource>() {
        match ParityOpSink::from_env() {
            Ok(Some(sink)) => {
                world.insert_resource(ParityTraceResource(Arc::new(sink)));
            }
            Ok(None) => {}
            Err(error) => panic!("{error}"),
        }
    }
    world
        .get_resource::<ParityTraceResource>()
        .map(|resource| Arc::clone(&resource.0))
}

fn finish_observer_failure<B: SystemInterpreter>(
    interpreter: &B,
    failure_sink: &B::FailureSink,
    metadata: &CallMetadata,
    policy: ErrorPolicy,
    failure: InterpreterFailure<B::ExceptionToken>,
) -> ObserverDispatchResult<B> {
    match policy {
        ErrorPolicy::PropagateToCaller => Err(failure),
        ErrorPolicy::ReportAndContinue => {
            interpreter.store_failure(
                failure_sink,
                failure,
                metadata,
                StoredErrorPolicy::ReportAndContinue,
            );
            Ok(())
        }
        ErrorPolicy::RaiseAfterUpdate => {
            interpreter.store_failure(
                failure_sink,
                failure,
                metadata,
                StoredErrorPolicy::RaiseAfterUpdate,
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::atomic::{AtomicBool, AtomicUsize},
    };

    use bevy::ecs::system::System;

    use super::*;

    #[derive(Resource, Default)]
    struct Counter(u32);

    #[derive(Clone)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone, Copy)]
    enum FakeCall {
        Unit,
        Bool(bool),
    }

    enum FakeRetained {
        Ready(FakeCall),
        Retired,
        Failed,
    }

    #[derive(Clone)]
    struct FakePlan {
        fail: bool,
        panic: bool,
        queue_counter: bool,
        recurse_observer: bool,
        condition_valid: bool,
    }

    impl Default for FakePlan {
        fn default() -> Self {
            Self {
                fail: false,
                panic: false,
                queue_counter: false,
                recurse_observer: false,
                condition_valid: true,
            }
        }
    }

    #[derive(Default)]
    struct FakeScheduledState {
        runs: usize,
    }

    struct FakeObserverState {
        id: usize,
    }

    type FailureLog = Arc<Mutex<Vec<(String, StoredErrorPolicy)>>>;

    struct FakeInterpreter {
        sink: FailureLog,
        last_validity: Arc<Mutex<Option<ValidityFlag>>>,
        last_ticks: Arc<Mutex<Option<RunTicks>>>,
        observer_state_ids: Arc<Mutex<Vec<usize>>>,
        next_observer_state: AtomicUsize,
        observer_depth: AtomicUsize,
        observer_handle: Mutex<Option<SystemHandle<Self>>>,
    }

    impl FakeInterpreter {
        fn new(sink: FailureLog, last_validity: Arc<Mutex<Option<ValidityFlag>>>) -> Self {
            Self {
                sink,
                last_validity,
                last_ticks: Arc::new(Mutex::new(None)),
                observer_state_ids: Arc::new(Mutex::new(Vec::new())),
                next_observer_state: AtomicUsize::new(0),
                observer_depth: AtomicUsize::new(0),
                observer_handle: Mutex::new(None),
            }
        }

        fn failure(message: &str) -> InterpreterFailure<String> {
            InterpreterFailure {
                report: ErrorReport {
                    message: message.to_string(),
                    traceback: Some("fake traceback".to_string()),
                },
                exception: Some(format!("token: {message}")),
            }
        }

        fn assert_retained_unlocked(&self) {
            let handle = self
                .observer_handle
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .as_ref()
                .expect("observer handle installed")
                .clone();
            assert!(
                handle.try_lock().is_ok(),
                "retained-state lock must not be held during invocation"
            );
        }
    }

    // SAFETY: the fake declares no parameter access, creates no raw-pointer
    // wrappers, uses only the supplied queue and validity flag, and stores
    // owned strings without invoking code while its sink lock is held.
    unsafe impl SystemInterpreter for FakeInterpreter {
        type Event = ();
        type PreparedCall = FakeCall;
        type ParamPlan = FakePlan;
        type ScheduledRunState = FakeScheduledState;
        type ObserverPersistentState = ();
        type ObserverRunState = FakeObserverState;
        type RetainedState = FakeRetained;
        type ExceptionToken = String;
        type FailureSink = FailureLog;

        fn initialize_scheduled(
            &self,
            _params: &Self::ParamPlan,
            _world: &mut World,
        ) -> InitializedRunState<Self::ScheduledRunState, Self::FailureSink> {
            InitializedRunState {
                state: FakeScheduledState::default(),
                failure_sink: self.sink.clone(),
                access: FilteredAccessSet::default(),
                validation: None,
            }
        }

        fn validate_condition(&self, params: &Self::ParamPlan) -> Result<(), String> {
            params
                .condition_valid
                .then_some(())
                .ok_or_else(|| "condition plan is not read-only".to_string())
        }

        fn resolve_callable(
            &self,
            retained: &SystemHandle<Self>,
            _current_generation: Option<u32>,
        ) -> CallablePreflight<Self::PreparedCall, Self::ExceptionToken> {
            let retained = retained.lock().unwrap_or_else(|poison| poison.into_inner());
            match *retained {
                FakeRetained::Ready(call) => CallablePreflight::Ready(call),
                FakeRetained::Retired => CallablePreflight::Retired,
                FakeRetained::Failed => CallablePreflight::Failed(Self::failure("preflight")),
            }
        }

        fn make_observer_run_state(
            &self,
            _params: &Self::ParamPlan,
            _persistent: &Self::ObserverPersistentState,
            _world: &mut World,
        ) -> Self::ObserverRunState {
            FakeObserverState {
                id: self.next_observer_state.fetch_add(1, Ordering::SeqCst),
            }
        }

        fn observer_event_type_name(&self, _event: &Self::Event) -> String {
            "FakeEvent".to_string()
        }

        unsafe fn build_args_and_call_scheduled(
            &self,
            prepared: Self::PreparedCall,
            params: &Self::ParamPlan,
            state: &mut Self::ScheduledRunState,
            ctx: InterpreterCallContext<'_, '_, Self::Event>,
        ) -> Result<CallOutcome, InterpreterFailure<Self::ExceptionToken>> {
            self.assert_retained_unlocked();
            *self
                .last_validity
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = Some(ctx.validity.clone());
            *self
                .last_ticks
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = Some(ctx.ticks);
            state.runs += 1;
            if params.queue_counter {
                ctx.commands.push(|world: &mut World| {
                    world.resource_mut::<Counter>().0 += 1;
                });
            }
            assert!(ctx.validity.check().is_ok());
            assert!(ctx.trigger.is_none());
            if params.panic {
                panic!("fake interpreter panic");
            }
            if params.fail {
                return Err(Self::failure("scheduled"));
            }
            Ok(match prepared {
                FakeCall::Unit => CallOutcome::Unit,
                FakeCall::Bool(value) => CallOutcome::Bool(value),
            })
        }

        unsafe fn build_args_and_call_observer(
            &self,
            _prepared: Self::PreparedCall,
            params: &Self::ParamPlan,
            state: &mut Self::ObserverRunState,
            ctx: InterpreterCallContext<'_, '_, Self::Event>,
        ) -> Result<CallOutcome, InterpreterFailure<Self::ExceptionToken>> {
            self.assert_retained_unlocked();
            self.observer_state_ids
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .push(state.id);
            if params.queue_counter {
                ctx.commands.push(|world: &mut World| {
                    world.resource_mut::<Counter>().0 += 1;
                });
            }

            if params.recurse_observer && self.observer_depth.fetch_add(1, Ordering::SeqCst) == 0 {
                let handle = self
                    .observer_handle
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .as_ref()
                    .expect("observer handle installed")
                    .clone();
                let metadata = CallMetadata {
                    name: "nested".to_string(),
                    kind: InvocationKind::Observer,
                };
                let event = ctx.trigger.as_ref().expect("observer trigger").event;
                // SAFETY: execute_observer's caller supplied an exclusive World
                // cell. The fake holds no references into it, and the nested
                // invocation creates a separate state/validity/queue scope.
                let world = unsafe { ctx.world.world_mut() };
                // SAFETY: the handle, plan, and fake persistent state all
                // describe this fake interpreter; `world` is exclusive.
                unsafe {
                    execute_observer(
                        self,
                        &handle,
                        params,
                        &(),
                        &self.sink,
                        &metadata,
                        None,
                        event,
                        None,
                        ErrorPolicy::ReportAndContinue,
                        world,
                    )
                }
                .expect("nested observer succeeds");
            }
            self.observer_depth.store(0, Ordering::SeqCst);

            if params.fail {
                Err(Self::failure("observer"))
            } else {
                Ok(CallOutcome::Unit)
            }
        }

        fn store_failure(
            &self,
            sink: &Self::FailureSink,
            failure: InterpreterFailure<Self::ExceptionToken>,
            _metadata: &CallMetadata,
            policy: StoredErrorPolicy,
        ) {
            let message = failure.report.message;
            sink.lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .push((message, policy));
            drop(failure.exception);
        }

        fn retire(&self, retained: &SystemHandle<Self>) {
            *retained.lock().unwrap_or_else(|poison| poison.into_inner()) = FakeRetained::Retired;
        }
    }

    struct FakeFixture {
        prepared: PreparedSystem<FakeInterpreter>,
        sink: FailureLog,
        validity: Arc<Mutex<Option<ValidityFlag>>>,
        observer_ids: Arc<Mutex<Vec<usize>>>,
        ticks: Arc<Mutex<Option<RunTicks>>>,
    }

    fn fixture(call: FakeCall, params: FakePlan, kind: InvocationKind) -> FakeFixture {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let validity = Arc::new(Mutex::new(None));
        let interpreter = FakeInterpreter::new(sink.clone(), validity.clone());
        let observer_ids = interpreter.observer_state_ids.clone();
        let ticks = interpreter.last_ticks.clone();
        let retained = Arc::new(Mutex::new(FakeRetained::Ready(call)));
        *interpreter
            .observer_handle
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(retained.clone());
        FakeFixture {
            prepared: PreparedSystem {
                interpreter,
                retained,
                params,
                metadata: CallMetadata {
                    name: "fake".to_string(),
                    kind,
                },
                flags: SystemFlags::default(),
                stage: SystemStage::UpdateOrLast,
                expected_generation: None,
            },
            sink,
            validity,
            observer_ids,
            ticks,
        }
    }

    #[test]
    fn unit_and_bool_systems_use_type_level_outputs() {
        let mut world = World::new();
        let unit = fixture(FakeCall::Unit, FakePlan::default(), InvocationKind::System);
        let mut unit_system = DynamicSystemCore::<_, UnitOutput>::new(unit.prepared);
        unit_system.initialize(&mut world);
        unit_system.run((), &mut world).unwrap();

        let condition = fixture(
            FakeCall::Bool(true),
            FakePlan::default(),
            InvocationKind::Condition,
        );
        let mut condition = DynamicConditionCore::new(condition.prepared).unwrap();
        condition.initialize(&mut world);
        assert!(condition.run((), &mut world).unwrap());
    }

    #[test]
    fn traced_system_records_run_before_its_flush_boundary() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut world = World::new();
        world.insert_resource(ParityTraceResource(Arc::new(ParityOpSink::new(Box::new(
            SharedWriter(output.clone()),
        )))));
        let fixture = fixture(FakeCall::Unit, FakePlan::default(), InvocationKind::System);
        let mut system = DynamicSystemCore::<_, UnitOutput>::new(fixture.prepared);
        system.initialize(&mut world);
        system.run((), &mut world).unwrap();
        system.apply_deferred(&mut world);

        let text = String::from_utf8(
            output
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone(),
        )
        .unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"record\":\"system_run\""));
        assert!(lines[1].contains("\"record\":\"flush_boundary\""));
    }

    #[test]
    fn condition_rejects_a_non_read_only_plan() {
        let fixture = fixture(
            FakeCall::Bool(true),
            FakePlan {
                condition_valid: false,
                ..Default::default()
            },
            InvocationKind::Condition,
        );
        assert!(DynamicConditionCore::new(fixture.prepared).is_err());
    }

    #[test]
    fn stale_generation_skips_before_tick_and_call() {
        let mut world = World::new();
        let counter = Arc::new(AtomicU32::new(7));
        world.insert_resource(HotReloadGeneration::new(counter));
        let mut fixture = fixture(FakeCall::Unit, FakePlan::default(), InvocationKind::System);
        fixture.prepared.expected_generation = Some(8);
        let validity = fixture.validity.clone();
        let mut system = DynamicSystemCore::<_, UnitOutput>::new(fixture.prepared);
        system.initialize(&mut world);
        let before = world.change_tick();
        system.run((), &mut world).unwrap();
        assert_eq!(world.change_tick(), before);
        assert!(validity.lock().unwrap().is_none());
    }

    #[test]
    fn run_uses_the_freshly_incremented_tick_as_this_run_and_last_run() {
        let mut world = World::new();
        let fixture = fixture(FakeCall::Unit, FakePlan::default(), InvocationKind::System);
        let ticks = fixture.ticks.clone();
        let mut system = DynamicSystemCore::<_, UnitOutput>::new(fixture.prepared);
        system.initialize(&mut world);

        let before = world.change_tick();
        system.run((), &mut world).unwrap();
        let after = world.change_tick();
        let captured = ticks.lock().unwrap().expect("run ticks captured");

        assert_ne!(after, before);
        assert_eq!(captured.this_run, after);
        assert_eq!(system.get_last_run(), after);
    }

    #[test]
    fn retired_and_failed_preflight_skip_before_tick() {
        for retained_state in [FakeRetained::Retired, FakeRetained::Failed] {
            let mut world = World::new();
            let fixture = fixture(FakeCall::Unit, FakePlan::default(), InvocationKind::System);
            *fixture.prepared.retained.lock().unwrap() = retained_state;
            let sink = fixture.sink.clone();
            let mut system = DynamicSystemCore::<_, UnitOutput>::new(fixture.prepared);
            system.initialize(&mut world);
            let before = world.change_tick();
            system.run((), &mut world).unwrap();
            assert_eq!(world.change_tick(), before);
            let failures = sink.lock().unwrap();
            if matches!(*system.handle().lock().unwrap(), FakeRetained::Failed) {
                assert_eq!(failures.len(), 1);
            } else {
                assert!(failures.is_empty());
            }
        }
    }

    #[test]
    fn commands_survive_failure_and_apply_after_validity_ends() {
        let mut world = World::new();
        world.insert_resource(Counter::default());
        let fixture = fixture(
            FakeCall::Unit,
            FakePlan {
                fail: true,
                queue_counter: true,
                ..Default::default()
            },
            InvocationKind::System,
        );
        let validity = fixture.validity.clone();
        let sink = fixture.sink.clone();
        let mut system = DynamicSystemCore::<_, UnitOutput>::new(fixture.prepared);
        system.initialize(&mut world);
        system.run((), &mut world).unwrap();
        assert_eq!(world.resource::<Counter>().0, 1);
        assert!(validity.lock().unwrap().as_ref().unwrap().check().is_err());
        assert_eq!(
            sink.lock().unwrap().as_slice(),
            &[("scheduled".to_string(), StoredErrorPolicy::RaiseAfterUpdate,)]
        );
    }

    #[test]
    fn panic_unwinding_invalidates_the_run_scope() {
        let mut world = World::new();
        let fixture = fixture(
            FakeCall::Unit,
            FakePlan {
                panic: true,
                ..Default::default()
            },
            InvocationKind::System,
        );
        let validity = fixture.validity.clone();
        let mut system = DynamicSystemCore::<_, UnitOutput>::new(fixture.prepared);
        system.initialize(&mut world);
        assert!(catch_unwind(AssertUnwindSafe(|| system.run((), &mut world))).is_err());
        assert!(validity.lock().unwrap().as_ref().unwrap().check().is_err());
    }

    #[test]
    fn observer_reporting_and_propagation_apply_commands_first() {
        for policy in [
            ErrorPolicy::ReportAndContinue,
            ErrorPolicy::PropagateToCaller,
        ] {
            let mut world = World::new();
            world.insert_resource(Counter::default());
            let fixture = fixture(
                FakeCall::Unit,
                FakePlan {
                    fail: true,
                    queue_counter: true,
                    ..Default::default()
                },
                InvocationKind::Observer,
            );
            let PreparedSystem {
                interpreter,
                retained,
                params,
                metadata,
                ..
            } = fixture.prepared;
            // SAFETY: the fake plan/handle describe this interpreter and the
            // test owns the World exclusively.
            let result = unsafe {
                execute_observer(
                    &interpreter,
                    &retained,
                    &params,
                    &(),
                    &fixture.sink,
                    &metadata,
                    None,
                    &(),
                    None,
                    policy,
                    &mut world,
                )
            };
            assert_eq!(world.resource::<Counter>().0, 1);
            match policy {
                ErrorPolicy::ReportAndContinue => {
                    assert!(result.is_ok());
                    assert_eq!(fixture.sink.lock().unwrap().len(), 1);
                }
                ErrorPolicy::PropagateToCaller => {
                    assert!(result.is_err());
                    assert!(fixture.sink.lock().unwrap().is_empty());
                }
                ErrorPolicy::RaiseAfterUpdate => unreachable!(),
            }
        }
    }

    #[test]
    fn observer_trace_wraps_callback_and_private_queue_flush() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut world = World::new();
        world.insert_resource(Counter::default());
        world.insert_resource(ParityTraceResource(Arc::new(ParityOpSink::new(Box::new(
            SharedWriter(output.clone()),
        )))));
        let fixture = fixture(
            FakeCall::Unit,
            FakePlan {
                queue_counter: true,
                ..Default::default()
            },
            InvocationKind::Observer,
        );
        let PreparedSystem {
            interpreter,
            retained,
            params,
            metadata,
            ..
        } = fixture.prepared;
        // SAFETY: the fake handle and plan match the interpreter, and this test
        // owns the World exclusively for the full callback and queue flush.
        unsafe {
            execute_observer(
                &interpreter,
                &retained,
                &params,
                &(),
                &fixture.sink,
                &metadata,
                None,
                &(),
                None,
                ErrorPolicy::ReportAndContinue,
                &mut world,
            )
        }
        .unwrap();

        assert_eq!(world.resource::<Counter>().0, 1);
        let text = String::from_utf8(
            output
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone(),
        )
        .unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"record\":\"observer_entry\""));
        assert!(lines[1].contains("\"record\":\"observer_flush\""));
    }

    #[test]
    fn recursive_observer_uses_fresh_state() {
        let mut world = World::new();
        let fixture = fixture(
            FakeCall::Unit,
            FakePlan {
                recurse_observer: true,
                ..Default::default()
            },
            InvocationKind::Observer,
        );
        let PreparedSystem {
            interpreter,
            retained,
            params,
            metadata,
            ..
        } = fixture.prepared;
        // SAFETY: the fake plan/handle describe this interpreter and the test
        // owns the World exclusively.
        unsafe {
            execute_observer(
                &interpreter,
                &retained,
                &params,
                &(),
                &fixture.sink,
                &metadata,
                None,
                &(),
                None,
                ErrorPolicy::ReportAndContinue,
                &mut world,
            )
        }
        .unwrap();
        assert_eq!(fixture.observer_ids.lock().unwrap().as_slice(), &[0, 1]);
    }

    struct FakeProfiler {
        called: AtomicBool,
    }

    impl RunProfileSink for FakeProfiler {
        fn record(
            &self,
            _system_name: &str,
            _duration: Duration,
            _stage: SystemStage,
            _app_time_seconds: f64,
        ) {
            self.called.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn initialization_declares_infrastructure_reads_and_profiles() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let profiler = Arc::new(FakeProfiler {
            called: AtomicBool::new(false),
        });
        world.insert_resource(RunProfileSinkResource(profiler.clone()));
        let fixture = fixture(FakeCall::Unit, FakePlan::default(), InvocationKind::System);
        let mut system = DynamicSystemCore::<_, UnitOutput>::new(fixture.prepared);
        let access = system.initialize(&mut world);
        let generation_id = world.register_component::<HotReloadGeneration>();
        let time_id = world.register_component::<Time<()>>();
        assert!(access.combined_access().has_read(generation_id));
        assert!(access.combined_access().has_read(time_id));
        system.run((), &mut world).unwrap();
        assert!(profiler.called.load(Ordering::SeqCst));
    }
}
