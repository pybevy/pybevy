//! PyO3 adapter for the interpreter-neutral dynamic-system runtime.
//!
//! This module is intentionally compile-only during migration Merge B. Every
//! adapter probe constructs a [`PreparedSystem`] from the same legacy
//! `DynamicSystem` state, but schedules and observers continue to execute
//! through their established Main paths. The prepared value is not retained in
//! production yet: doing so would keep cloned Python parameter references alive
//! after the legacy hot-reload handle has been retired.

#![allow(dead_code)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{component::ComponentId, system::SystemParamValidationError, world::World},
    prelude::{Commands, Resource},
};
#[cfg(debug_assertions)]
use pybevy_ecs::shared::access_audit::assert_query_access_declared;
use pybevy_ecs::shared::{
    access_validation as shared_validation,
    command_queue_helpers::create_commands_from_queue,
    param_spec::{
        build_declared_access, condition_param_rejection, condition_rejection_message,
        conflict_error_message, to_param_accesses,
    },
    system_runtime::{
        CallMetadata, CallOutcome, CallablePreflight, DynamicConditionCore, DynamicSystemCore,
        ErrorReport, InitializedRunState, InterpreterCallContext, InterpreterFailure,
        InvocationKind, OutputMode, PreparedSystem, StoredErrorPolicy, SystemFlags, SystemHandle,
        SystemInterpreter, UnitOutput,
    },
};
use pyo3::{exceptions::PyRuntimeError, ffi::PyTypeObject, prelude::*, types::PyTuple};
use smallvec::SmallVec;

use super::{
    commands::CommandErrorSink,
    dynamic_system::{
        BufferedSystemError, DynamicSystem, DynamicSystemHandle, DynamicSystemInner, MainResolver,
        SystemErrorBuffer, execute_prepared_observer, lock_or_recover, lower_param_type,
        lower_params, validate_system_params,
    },
    messages::{CursorStorage, MessageType},
    observer::PyOn,
    query::query_runtime::CachedQuery,
    system::{SystemFunction, SystemParam, SystemParamType},
    view::cached_view::CachedPyView,
};

/// Immutable parsed signature used by every context for one Python callable.
pub(crate) struct MainParamPlan {
    retained: DynamicSystemHandle,
    name: String,
}

pub(crate) struct MainPreparedCall {
    callable: Py<PyAny>,
    params: SmallVec<[SystemParam; 8]>,
    message_cursors: Vec<CursorStorage>,
}

/// Reusable state belonging only to the scheduled adapter path.
pub(crate) struct MainScheduledRunState {
    query_caches: Vec<CachedQuery>,
    view_caches: Vec<Result<Arc<CachedPyView>, Arc<str>>>,
    args: SmallVec<[Py<PyAny>; 8]>,
}

/// Observer state is deliberately fresh per callback. The current Main
/// observer helper owns its transient query/view caches inside the call.
pub(crate) struct MainObserverRunState;

#[derive(Clone)]
pub(crate) struct MainFailureSink {
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
    retain_exception: bool,
}

/// The real Main/PyO3 implementation of the neutral interpreter seam.
pub(crate) struct MainInterpreter {
    module_name: String,
    function_name: String,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
}

pub(crate) type MainDynamicSystem = DynamicSystemCore<MainInterpreter, UnitOutput>;
pub(crate) type MainDynamicCondition = DynamicConditionCore<MainInterpreter>;

/// Everything one registered observer needs after the registry borrow ends.
pub(crate) struct MainPreparedObserver {
    pub(crate) interpreter: MainInterpreter,
    pub(crate) retained: SystemHandle<MainInterpreter>,
    pub(crate) params: MainParamPlan,
    pub(crate) persistent: (),
    pub(crate) failure_sink: MainFailureSink,
    pub(crate) metadata: CallMetadata,
    pub(crate) expected_generation: Option<u32>,
}

/// Off-World handles captured by observers at registration time.
#[derive(Clone, Resource)]
pub(crate) struct ObserverRuntimeSinks {
    pub(crate) error_state: Arc<Mutex<Vec<PyErr>>>,
    pub(crate) error_buffer: SystemErrorBuffer,
}

/// Parse and prepare one scheduled Main system for the neutral runtime shell.
#[allow(clippy::too_many_arguments)]
fn prepare_callable(
    func: Py<PyAny>,
    generation: u32,
    stage: pybevy_reload::SystemStage,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
    kind: InvocationKind,
) -> PyResult<PreparedSystem<MainInterpreter>> {
    let (system_func, module_name, function_name) = Python::attach(|py| {
        let func_bound = func.bind(py);
        let name = func_bound
            .getattr("__name__")
            .ok()
            .and_then(|name| name.extract::<String>().ok())
            .unwrap_or_else(|| "DynamicSystem".to_string());
        let mut module_name = func_bound
            .getattr("__module__")
            .ok()
            .and_then(|module| module.extract::<String>().ok())
            .unwrap_or_else(|| "__main__".to_string());
        if module_name == "<run_path>" {
            module_name = "__main__".to_string();
        }
        let system_func = SystemFunction::new(py, func_bound.clone())?;
        Ok::<_, PyErr>((system_func, module_name, name))
    })?;

    validate_system_params(&system_func.params, &function_name)?;
    let message_reader_count = system_func
        .params
        .iter()
        .filter(|param| {
            matches!(
                param.ty,
                SystemParamType::MessageReader { .. } | SystemParamType::MessageMutator { .. }
            )
        })
        .count();
    let message_cursor_storage = (0..message_reader_count)
        .map(|_| Arc::new(Mutex::new(None)))
        .collect();
    let retained = Arc::new(Mutex::new(DynamicSystemInner {
        system_func: Some(system_func),
        cached_func: Some(func),
        cached_generation: generation,
        message_cursor_storage,
        gutted: false,
    }));

    let prepared = {
        let retained_guard = lock_or_recover(&retained);
        MainInterpreter::prepare(
            retained_guard
                .system_func
                .as_ref()
                .expect("newly prepared system retains its parsed function"),
            retained.clone(),
            module_name,
            function_name,
            generation,
            stage,
            error_state,
            error_buffer,
            kind,
        )
    };
    Ok(prepared)
}

pub(crate) fn new_main_system(
    func: Py<PyAny>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
    stage: pybevy_reload::SystemStage,
) -> PyResult<MainDynamicSystem> {
    Ok(MainDynamicSystem::new(prepare_callable(
        func,
        generation,
        stage,
        error_state,
        error_buffer,
        InvocationKind::System,
    )?))
}

pub(crate) fn new_main_condition(
    func: Py<PyAny>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    stage: pybevy_reload::SystemStage,
) -> PyResult<MainDynamicCondition> {
    let prepared = prepare_callable(
        func,
        generation,
        stage,
        error_state,
        Arc::new(Mutex::new(None)),
        InvocationKind::Condition,
    )?;
    MainDynamicCondition::new(prepared).map_err(PyRuntimeError::new_err)
}

/// Prepare an observer once at registration, retaining the same callable and
/// immutable parameter plan used by the scheduled interpreter path.
pub(crate) fn new_main_observer(
    system_func: SystemFunction,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
) -> MainPreparedObserver {
    let (module_name, function_name) = Python::attach(|py| {
        let func = system_func.func.bind(py);
        let name = func
            .getattr("__name__")
            .ok()
            .and_then(|name| name.extract::<String>().ok())
            .unwrap_or_else(|| "Observer".to_string());
        let mut module = func
            .getattr("__module__")
            .ok()
            .and_then(|module| module.extract::<String>().ok())
            .unwrap_or_else(|| "__main__".to_string());
        if module == "<run_path>" {
            module = "__main__".to_string();
        }
        (module, name)
    });
    let message_reader_count = system_func
        .params
        .iter()
        .filter(|param| {
            matches!(
                param.ty,
                SystemParamType::MessageReader { .. } | SystemParamType::MessageMutator { .. }
            )
        })
        .count();
    let message_cursor_storage = (0..message_reader_count)
        .map(|_| Arc::new(Mutex::new(None)))
        .collect();
    let callable = Python::attach(|py| system_func.func.clone_ref(py));
    let retained = Arc::new(Mutex::new(DynamicSystemInner {
        system_func: Some(system_func),
        cached_func: Some(callable),
        cached_generation: generation,
        message_cursor_storage,
        gutted: false,
    }));
    let prepared = {
        let retained_guard = lock_or_recover(&retained);
        MainInterpreter::prepare(
            retained_guard
                .system_func
                .as_ref()
                .expect("new observer retains its parsed function"),
            retained.clone(),
            module_name,
            function_name,
            generation,
            pybevy_reload::SystemStage::UpdateOrLast,
            error_state,
            error_buffer,
            InvocationKind::Observer,
        )
    };
    let failure_sink = MainFailureSink {
        error_state: prepared.interpreter.error_state.clone(),
        error_buffer: prepared.interpreter.error_buffer.clone(),
        // Observer failures are reports, never delayed exception tokens.
        retain_exception: false,
    };
    MainPreparedObserver {
        interpreter: prepared.interpreter,
        retained: prepared.retained,
        params: prepared.params,
        persistent: (),
        failure_sink,
        metadata: prepared.metadata,
        expected_generation: prepared.expected_generation,
    }
}

/// Retire a Main system without running Python finalizers under its mutex.
pub(crate) fn retire_main_handle(retained: &DynamicSystemHandle) {
    Python::attach(|_| {
        let (system_func, cached_func, cursors) = {
            let mut retained = lock_or_recover(retained);
            if retained.gutted {
                return;
            }
            retained.gutted = true;
            (
                retained.system_func.take(),
                retained.cached_func.take(),
                std::mem::take(&mut retained.message_cursor_storage),
            )
        };
        drop(system_func);
        drop(cached_func);
        drop(cursors);
    });
}

impl MainInterpreter {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        system_func: &SystemFunction,
        retained: DynamicSystemHandle,
        module_name: String,
        function_name: String,
        generation: u32,
        stage: pybevy_reload::SystemStage,
        error_state: Arc<Mutex<Vec<PyErr>>>,
        error_buffer: SystemErrorBuffer,
        kind: InvocationKind,
    ) -> PreparedSystem<Self> {
        let needs_exclusive = system_func
            .params
            .iter()
            .any(|param| matches!(param.ty, SystemParamType::World));
        let needs_commands = system_func
            .params
            .iter()
            .any(|param| matches!(param.ty, SystemParamType::Commands));

        let name = function_name.clone();
        let plan_retained = retained.clone();
        PreparedSystem {
            interpreter: Self {
                module_name,
                function_name,
                error_state,
                error_buffer,
            },
            params: MainParamPlan {
                retained: plan_retained,
                name: name.clone(),
            },
            retained,
            metadata: CallMetadata { name, kind },
            flags: SystemFlags {
                needs_exclusive,
                needs_commands,
            },
            stage,
            expected_generation: Some(generation),
        }
    }

    fn failure(py: Python<'_>, error: PyErr) -> InterpreterFailure<PyErr> {
        let message = error.to_string();
        let traceback = error.traceback(py).map(|traceback| {
            traceback
                .format()
                .unwrap_or_else(|_| "(traceback format failed)".into())
        });
        InterpreterFailure {
            report: ErrorReport { message, traceback },
            exception: Some(error),
        }
    }

    fn call(
        py: Python<'_>,
        prepared: Py<PyAny>,
        args: &[Py<PyAny>],
        output: OutputMode,
    ) -> Result<CallOutcome, InterpreterFailure<PyErr>> {
        let result = if args.is_empty() {
            prepared.bind(py).call0()
        } else {
            let tuple = PyTuple::new(py, args).expect("Failed to create PyTuple");
            prepared.bind(py).call1(tuple)
        };
        let outcome = match result {
            Ok(value) => match output {
                OutputMode::Unit => CallOutcome::Unit,
                OutputMode::Bool => match value.is_truthy() {
                    Ok(value) => CallOutcome::Bool(value),
                    Err(error) => return Err(Self::failure(py, error)),
                },
            },
            Err(error) => return Err(Self::failure(py, error)),
        };
        // `prepared` and all temporary return values drop before leaving this
        // attached Python context, including along every error branch above.
        drop(prepared);
        Ok(outcome)
    }
}

// SAFETY: initialization below derives declared access and runtime caches from
// the same lowered parameter plan. Both call paths use the core-supplied
// validity flag, do not retain world cells, format failures before sink locks,
// and own/drop their callable snapshot inside `Python::attach`.
unsafe impl SystemInterpreter for MainInterpreter {
    type Event = Py<PyOn>;
    type PreparedCall = MainPreparedCall;
    type ParamPlan = MainParamPlan;
    type ScheduledRunState = MainScheduledRunState;
    type ObserverPersistentState = ();
    type ObserverRunState = MainObserverRunState;
    type RetainedState = DynamicSystemInner;
    type ExceptionToken = PyErr;
    type FailureSink = MainFailureSink;

    fn initialize_scheduled(
        &self,
        plan: &Self::ParamPlan,
        world: &mut World,
    ) -> InitializedRunState<Self::ScheduledRunState, Self::FailureSink> {
        let params = {
            let retained = lock_or_recover(&plan.retained);
            retained
                .system_func
                .as_ref()
                .map(|system_func| system_func.params.clone())
                .unwrap_or_default()
        };
        let specs = lower_params(&params);
        let mut custom_component_ids = HashMap::<*const PyTypeObject, ComponentId>::new();
        let declared = {
            let mut resolver = MainResolver {
                custom_component_ids: &mut custom_component_ids,
            };
            build_declared_access(world, &specs, &mut resolver)
        };

        #[allow(clippy::arc_with_non_send_sync)]
        let snapshot = Arc::new(custom_component_ids.clone());
        let query_caches: Vec<CachedQuery> = params
            .iter()
            .filter_map(|param| match &param.ty {
                SystemParamType::Query { param } => {
                    Some(CachedQuery::build(world, param.clone(), snapshot.clone()))
                }
                _ => None,
            })
            .collect();
        let view_caches = params
            .iter()
            .filter_map(|param| match &param.ty {
                SystemParamType::View { param: view } => {
                    // SAFETY: the descriptor is the same one lowered into the
                    // declared access above and initialization owns the World.
                    Some(unsafe { CachedPyView::build(world, view, &custom_component_ids) }.map_err(
                        |error| Arc::<str>::from(format!(
                            "System `{}` View parameter `{}` could not be initialized: {error}",
                            plan.name, param.name
                        )),
                    ))
                }
                _ => None,
            })
            .collect();

        let accesses = to_param_accesses(
            &specs,
            |component| match component {
                super::component_type::PyComponentType::Custom(type_ptr) => custom_component_ids
                    .get(type_ptr)
                    .copied()
                    .unwrap_or_else(|| component.register_simple(world)),
                _ => component.register_simple(world),
            },
            MessageType::validation_identity,
        );
        let validation = shared_validation::validate_access(&accesses)
            .err()
            .map(|conflict| {
                SystemParamValidationError::new::<DynamicSystem>(
                    false,
                    conflict_error_message(&plan.name, &conflict),
                    format!("parameter_{}", conflict.param_idx),
                )
            });

        #[cfg(debug_assertions)]
        for (index, cached) in query_caches.iter().enumerate() {
            assert_query_access_declared(
                &plan.name,
                index,
                &declared.set,
                &cached.component_access(),
            );
        }

        let initialized = InitializedRunState {
            state: MainScheduledRunState {
                query_caches,
                view_caches,
                args: SmallVec::new(),
            },
            failure_sink: MainFailureSink {
                error_state: self.error_state.clone(),
                error_buffer: self.error_buffer.clone(),
                retain_exception: world
                    .get_resource::<pybevy_reload::HotReloadGeneration>()
                    .is_none(),
            },
            access: declared.set,
            validation,
        };
        // `SystemParam` may contain `Py` references. Drop the initialization
        // snapshot while attached, never on an arbitrary scheduler thread.
        Python::attach(|_| drop(params));
        initialized
    }

    fn validate_condition(&self, plan: &Self::ParamPlan) -> Result<(), String> {
        let params = {
            let retained = lock_or_recover(&plan.retained);
            retained
                .system_func
                .as_ref()
                .map(|system_func| system_func.params.clone())
                .unwrap_or_default()
        };
        let result = params.iter().enumerate().find_map(|(index, param)| {
            condition_param_rejection(&lower_param_type(&param.ty)).map(|kind| {
                Err(condition_rejection_message(
                    &plan.name,
                    index,
                    &param.name,
                    kind,
                ))
            })
        });
        Python::attach(|_| drop(params));
        result.unwrap_or(Ok(()))
    }

    fn resolve_callable(
        &self,
        retained: &SystemHandle<Self>,
        current_generation: Option<u32>,
    ) -> CallablePreflight<Self::PreparedCall, Self::ExceptionToken> {
        Python::attach(|py| {
            let generation = current_generation.unwrap_or(0);
            let cached_snapshot = {
                let retained = lock_or_recover(retained);
                if retained.gutted {
                    return CallablePreflight::Retired;
                }
                if self.module_name == "__main__"
                    || (retained.cached_generation == generation && retained.cached_func.is_some())
                {
                    retained.cached_func.as_ref().and_then(|callable| {
                        retained
                            .system_func
                            .as_ref()
                            .map(|system_func| MainPreparedCall {
                                callable: callable.clone_ref(py),
                                params: system_func.params.clone(),
                                message_cursors: retained.message_cursor_storage.clone(),
                            })
                    })
                } else {
                    None
                }
            };
            if let Some(snapshot) = cached_snapshot {
                return CallablePreflight::Ready(snapshot);
            }

            let mut candidate = match py
                .import(self.module_name.as_str())
                .and_then(|module| module.getattr(self.function_name.as_str()))
            {
                Ok(callable) => Some(callable.unbind()),
                Err(error) => return CallablePreflight::Failed(Self::failure(py, error)),
            };

            let (result, old_callable) = {
                let mut retained = lock_or_recover(retained);
                if retained.gutted {
                    (CallablePreflight::Retired, None)
                } else {
                    let old = if retained.cached_generation != generation
                        || retained.cached_func.is_none()
                    {
                        let old = retained
                            .cached_func
                            .replace(candidate.take().expect("refresh candidate is present"));
                        retained.cached_generation = generation;
                        old
                    } else {
                        None
                    };
                    let snapshot = retained.cached_func.as_ref().and_then(|callable| {
                        retained
                            .system_func
                            .as_ref()
                            .map(|system_func| MainPreparedCall {
                                callable: callable.clone_ref(py),
                                params: system_func.params.clone(),
                                message_cursors: retained.message_cursor_storage.clone(),
                            })
                    });
                    (
                        snapshot
                            .map(CallablePreflight::Ready)
                            .unwrap_or(CallablePreflight::Retired),
                        old,
                    )
                }
            };
            // A losing candidate and replaced callable may run Python
            // finalizers, so drop them only after releasing the mutex.
            drop(candidate);
            drop(old_callable);
            result
        })
    }

    fn make_observer_run_state(
        &self,
        _params: &Self::ParamPlan,
        _persistent: &Self::ObserverPersistentState,
        _world: &mut World,
    ) -> Self::ObserverRunState {
        MainObserverRunState
    }

    fn observer_event_type_name(&self, event: &Self::Event) -> String {
        Python::attach(|py| {
            event
                .bind(py)
                .borrow()
                .event_data
                .bind(py)
                .get_type()
                .name()
                .map(|name| name.to_string())
                .unwrap_or_else(|_| "Unknown".to_string())
        })
    }

    unsafe fn build_args_and_call_scheduled(
        &self,
        prepared: Self::PreparedCall,
        plan: &Self::ParamPlan,
        state: &mut Self::ScheduledRunState,
        ctx: InterpreterCallContext<'_, '_, Self::Event>,
    ) -> Result<CallOutcome, InterpreterFailure<Self::ExceptionToken>> {
        Python::attach(|py| {
            state.args.clear();
            let queue_ptr = ctx.commands as *mut _;
            let MainPreparedCall {
                callable,
                params,
                message_cursors,
            } = prepared;
            let needs_commands = params
                .iter()
                .any(|param| matches!(param.ty, SystemParamType::Commands));
            // SAFETY: the neutral core owns the live local queue and matching
            // world cell for this complete callback.
            let mut commands: Option<Commands<'_, '_>> = needs_commands
                .then(|| unsafe { create_commands_from_queue(&mut *queue_ptr, ctx.world) });
            // SAFETY: initialization built these caches from this exact plan
            // and the scheduler enforces the returned declared access.
            let error = unsafe {
                DynamicSystem::build_run_args(
                    py,
                    &params,
                    &state.query_caches,
                    &state.view_caches,
                    &message_cursors,
                    &mut commands,
                    &mut state.args,
                    ctx.world,
                    ctx.validity,
                    ctx.ticks.last_run,
                    ctx.ticks.this_run,
                    &plan.name,
                    &CommandErrorSink::new(
                        self.error_state.clone(),
                        self.error_buffer.clone(),
                        ctx.world
                            .get_resource::<pybevy_reload::HotReloadGeneration>()
                            .is_none(),
                    ),
                    ctx.parity_trace.cloned(),
                )
            };
            if let Some(error) = error {
                state.args.clear();
                drop(callable);
                drop(params);
                drop(message_cursors);
                return Err(Self::failure(py, error));
            }
            let result = Self::call(py, callable, &state.args, ctx.output);
            // Run-scoped wrappers must drop while Python is attached and
            // before the neutral core invalidates `ctx.validity`.
            state.args.clear();
            result
        })
    }

    unsafe fn build_args_and_call_observer(
        &self,
        prepared: Self::PreparedCall,
        _plan: &Self::ParamPlan,
        _state: &mut Self::ObserverRunState,
        ctx: InterpreterCallContext<'_, '_, Self::Event>,
    ) -> Result<CallOutcome, InterpreterFailure<Self::ExceptionToken>> {
        Python::attach(|py| {
            let MainPreparedCall {
                callable,
                params,
                message_cursors,
            } = prepared;
            let Some(trigger) = ctx.trigger else {
                drop(callable);
                drop(params);
                drop(message_cursors);
                return Err(Self::failure(
                    py,
                    PyRuntimeError::new_err("observer invocation is missing its On trigger"),
                ));
            };
            // SAFETY: observer execution is exclusive by the neutral contract;
            // every wrapper uses the validity flag supplied by that core.
            let result = unsafe {
                execute_prepared_observer(
                    py,
                    callable.bind(py),
                    &params,
                    ctx.world,
                    ctx.commands,
                    trigger.event.clone_ref(py),
                    ctx.validity,
                    &CommandErrorSink::new(
                        self.error_state.clone(),
                        self.error_buffer.clone(),
                        false,
                    ),
                    &message_cursors,
                    ctx.parity_trace.cloned(),
                )
            };
            drop(callable);
            drop(params);
            drop(message_cursors);
            result
                .map(|()| CallOutcome::Unit)
                .map_err(|error| Self::failure(py, error))
        })
    }

    fn store_failure(
        &self,
        sink: &Self::FailureSink,
        mut failure: InterpreterFailure<Self::ExceptionToken>,
        _metadata: &CallMetadata,
        policy: StoredErrorPolicy,
    ) {
        {
            let mut buffered = lock_or_recover(&sink.error_buffer);
            *buffered = Some(BufferedSystemError {
                error: failure.report.message.clone(),
                traceback: failure.report.traceback.clone(),
            });
        }
        if policy == StoredErrorPolicy::RaiseAfterUpdate
            && sink.retain_exception
            && let Some(exception) = failure.exception.take()
        {
            lock_or_recover(&sink.error_state).push(exception);
        }
        // Any unretained token is dropped in an attached interpreter context,
        // outside both sink locks, so Python finalizers cannot run under them.
        Python::attach(|_| drop(failure));
    }

    fn retire(&self, retained: &SystemHandle<Self>) {
        retire_main_handle(retained);
    }
}
