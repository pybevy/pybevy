use std::{
    any::TypeId,
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy::ecs::{
    schedule::{
        Chain, InternedSystemSet, Schedule, ScheduleConfigs, Schedules, SingleThreadedExecutor,
    },
    world::World,
};
use pybevy_ecs::shared::schedule::{StateScheduleLabel, TransitionScheduleLabel};
use pybevy_reload::{
    DefsFingerprint, KEEP_ALIVE_GENERATIONS, ReloadError, ReloadMode, ReloadRuntime, SystemStage,
    is_verbose, lock_or_recover,
};
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyType};

use super::{cleanup, registry::DynamicSystemRegistry, util::get_python_gc_objects};
use crate::{
    app::{
        PyStage,
        app::{PendingStateDefinition, PendingStateSystems, PyApp},
        chained_systems::{PyChainedSystemSets, PyChainedSystems},
    },
    ecs::{
        conditional_system::PyConditionalSystem,
        dynamic_system::{DynamicSystemHandle, LastErrorBuffer, SystemErrorBuffer},
        observer_registry::ObserverRegistry,
        python_message::{
            clear_python_messages, prune_python_message_aliases, register_python_message,
        },
        resource_type::register_custom_resource,
        state::{
            PyOnEnterSchedule, PyOnExitSchedule, PyOnTransitionSchedule,
            canonicalize_state_schedule_label, canonicalize_transition_schedule_label,
            ensure_state_transition_system_registered, register_reloaded_state_machine,
        },
        system_config::{
            InstalledSystemSetConfigs, PySystemConfig, SystemSetConfigIdentity,
            build_scheduled_system, build_set_config, system_set_config_identity,
        },
        system_interpreter::retire_main_handle,
        world::PyWorld,
    },
};

/// Container for definitions loaded from the Python loader function.
pub(crate) struct PendingDefinitions {
    pub systems: Vec<(PyStage, Vec<Py<PyAny>>)>,
    pub set_configs: Vec<(PyStage, Vec<Py<PyAny>>)>,
    pub resources: Vec<Py<PyAny>>,
    pub states: Vec<PendingStateDefinition>,
    pub state_systems: Vec<PendingStateSystems>,
    pub messages: Vec<Py<PyType>>,
    pub observers: Vec<Py<PyAny>>,
    pub plugins: Vec<String>,
    pub component_layout_changes: Vec<String>,
}

struct PreparedSystemSetConfig {
    schedule: PyStage,
    config: ScheduleConfigs<InternedSystemSet>,
    identities: Vec<SystemSetConfigIdentity>,
}

pub(crate) fn collect_system_names(system: &Bound<'_, PyAny>, names: &mut HashSet<String>) {
    if let Ok(config) = system.extract::<PySystemConfig>() {
        collect_system_names(config.system.bind(system.py()), names);
    } else if let Ok(conditional) = system.extract::<PyConditionalSystem>() {
        collect_system_names(conditional.system.bind(system.py()), names);
    } else if let Ok(chained) = system.extract::<PyChainedSystems>() {
        for inner in chained.systems.bind(system.py()).iter() {
            collect_system_names(&inner, names);
        }
    } else if let Ok(name) = system.getattr("__name__")
        && let Ok(name) = name.extract::<String>()
    {
        names.insert(name);
    } else if let Ok(iter) = system.try_iter() {
        for inner in iter.flatten() {
            collect_system_names(&inner, names);
        }
    }
}

fn system_source_location(system: &Bound<'_, PyAny>) -> Option<String> {
    if let Ok(config) = system.extract::<PySystemConfig>() {
        return system_source_location(config.system.bind(system.py()));
    }
    if let Ok(conditional) = system.extract::<PyConditionalSystem>() {
        return system_source_location(conditional.system.bind(system.py()));
    }

    let code = system.getattr("__code__").ok()?;
    let file = code.getattr("co_filename").ok()?.extract::<String>().ok()?;
    let line = code
        .getattr("co_firstlineno")
        .ok()?
        .extract::<usize>()
        .ok()?;
    let name = system
        .getattr("__qualname__")
        .ok()
        .and_then(|name| name.extract::<String>().ok())
        .unwrap_or_else(|| "<system>".to_string());
    Some(format!("  File \"{file}\", line {line}, in {name}"))
}

fn annotate_registration_error(py: Python<'_>, system: &Bound<'_, PyAny>, error: PyErr) -> PyErr {
    if error.traceback(py).is_some() {
        return error;
    }
    let Some(location) = system_source_location(system) else {
        return error;
    };
    PyRuntimeError::new_err(format!("{error}\n{location}"))
}

fn reload_error_from_py(error: &PyErr, message_prefix: &str, is_load_failure: bool) -> ReloadError {
    let message = format!("{message_prefix}{error}");
    let traceback = Python::attach(|py| {
        error
            .traceback(py)
            .and_then(|traceback| traceback.format().ok())
    })
    .or_else(|| {
        message.find("File \"").map(|start| {
            message[start..]
                .lines()
                .next()
                .unwrap_or(&message[start..])
                .to_string()
        })
    });
    ReloadError {
        message,
        traceback,
        is_load_failure,
    }
}

enum ReloadStateSchedule {
    State(StateScheduleLabel),
    Transition(TransitionScheduleLabel),
}

/// Add a batch of Python system functions to a Bevy schedule.
/// Handles both regular systems and chained systems (pipes).
#[allow(clippy::too_many_arguments)]
fn add_systems_to_schedule(
    schedule: &mut Schedule,
    systems: Vec<Py<PyAny>>,
    generation: u32,
    error_state: &Arc<Mutex<Vec<PyErr>>>,
    error_buffer: &SystemErrorBuffer,
    system_stage: SystemStage,
    stage: PyStage,
    system_handles: &mut Vec<DynamicSystemHandle>,
) {
    let is_startup = stage.is_startup();

    for system_func in systems {
        let result = Python::attach(|py| -> PyResult<()> {
            let system_bound = system_func.bind(py);

            if let Ok(chained) = system_bound.extract::<PyChainedSystems>() {
                let systems_tuple = chained.systems.bind(py);
                let mut configs = Vec::new();

                for sys in systems_tuple.iter() {
                    let (config, handles) = build_scheduled_system(
                        &sys,
                        generation,
                        error_state.clone(),
                        error_buffer.clone(),
                        system_stage,
                        is_startup,
                    )
                    .map_err(|error| annotate_registration_error(py, &sys, error))?;
                    system_handles.extend(handles);
                    configs.push(config);
                }

                if configs.is_empty() {
                    return Err(PyRuntimeError::new_err("Empty chained systems"));
                }

                let chained = ScheduleConfigs::Configs {
                    configs,
                    collective_conditions: Vec::new(),
                    metadata: Chain::Chained(Default::default()),
                };

                schedule.add_systems(chained);
            } else {
                let (config, handles) = build_scheduled_system(
                    system_bound,
                    generation,
                    error_state.clone(),
                    error_buffer.clone(),
                    system_stage,
                    is_startup,
                )
                .map_err(|error| annotate_registration_error(py, system_bound, error))?;
                system_handles.extend(handles);
                schedule.add_systems(config);
            }
            Ok(())
        });

        if let Err(e) = result {
            eprintln!(
                "❌ [Hot Reload] Failed to add system to {:?} schedule: {}",
                stage, e
            );
            eprintln!("   Reload cancelled to prevent broken app state.");
            let mut error_lock = lock_or_recover(error_state);
            error_lock.push(PyRuntimeError::new_err(format!(
                "Hot reload failed: Could not add system to {:?} schedule: {}",
                stage, e
            )));
            return;
        }
    }
}

/// Hash a Python callable's name and code object into `hasher`.
///
/// `marshal.dumps(func.__code__)` covers the bytecode, literals, and nested
/// code objects deterministically. It also embeds the first line number, so
/// shifting a function within its file counts as a change (conservative).
/// Callables without `__code__` contribute only their qualified name.
fn hash_callable_code(py: Python<'_>, obj: &Bound<'_, PyAny>, hasher: &mut impl std::hash::Hasher) {
    use std::hash::Hash;

    if let Ok(name) = obj
        .getattr("__qualname__")
        .and_then(|n| n.extract::<String>())
    {
        name.hash(hasher);
    }
    if let Ok(code) = obj.getattr("__code__")
        && let Ok(dumped) = py
            .import("marshal")
            .and_then(|m| m.call_method1("dumps", (code,)))
        && let Ok(bytes) = dumped.extract::<Vec<u8>>()
    {
        bytes.hash(hasher);
    }
    // Keyword defaults live outside __code__ but change call behavior.
    if let Ok(defaults) = obj.getattr("__defaults__")
        && let Ok(repr) = defaults.repr()
    {
        repr.to_string().hash(hasher);
    }
}

/// Hash a registered system into `hasher`, unwrapping conditional and
/// chained wrappers the same way `system_names` does.
fn hash_system_code(
    py: Python<'_>,
    sys_bound: &Bound<'_, PyAny>,
    hasher: &mut impl std::hash::Hasher,
) {
    if let Ok(config) = sys_bound.extract::<PySystemConfig>() {
        hash_callable_code(py, config.system.bind(py), hasher);
        for condition in &config.conditions {
            hash_callable_code(py, condition.bind(py), hasher);
        }
    } else if let Ok(conditional) = sys_bound.extract::<PyConditionalSystem>() {
        hash_callable_code(py, conditional.system.bind(py), hasher);
        conditional.condition.for_each_leaf(&mut |condition| {
            hash_callable_code(py, condition.bind(py), hasher);
        });
    } else if let Ok(chained) = sys_bound.extract::<PyChainedSystems>() {
        for inner in chained.systems.bind(py).iter() {
            hash_callable_code(py, &inner, hasher);
        }
    } else {
        hash_callable_code(py, sys_bound, hasher);
    }
}

pub(crate) struct Pyo3ReloadRuntime {
    pub loader_func: Py<PyAny>,
    pub error_state: Arc<Mutex<Vec<PyErr>>>,
    pending_set_configs: Vec<PreparedSystemSetConfig>,
    component_layout_reload_pending: bool,
}

impl Pyo3ReloadRuntime {
    pub(crate) fn new(loader_func: Py<PyAny>, error_state: Arc<Mutex<Vec<PyErr>>>) -> Self {
        Self {
            loader_func,
            error_state,
            pending_set_configs: Vec::new(),
            component_layout_reload_pending: false,
        }
    }

    fn installed_set_config<'a>(
        &'a self,
        world: &'a World,
        schedule: PyStage,
        identity: &SystemSetConfigIdentity,
    ) -> Option<&'a SystemSetConfigIdentity> {
        self.pending_set_configs
            .iter()
            .filter(|prepared| prepared.schedule == schedule)
            .flat_map(|prepared| prepared.identities.iter())
            .find(|installed| installed.set() == identity.set())
            .or_else(|| {
                world
                    .get_resource::<InstalledSystemSetConfigs>()
                    .and_then(|installed| installed.get(schedule, identity.set()))
            })
    }

    fn prepare_set_configs(
        &mut self,
        world: &World,
        pending: Vec<(PyStage, Vec<Py<PyAny>>)>,
        generation: u32,
    ) -> Result<(), ReloadError> {
        Python::attach(|py| -> PyResult<()> {
            for (schedule, values) in pending {
                let system_stage = if schedule.is_startup() {
                    SystemStage::Startup
                } else {
                    SystemStage::UpdateOrLast
                };
                for value in values {
                    let value = value.bind(py);
                    if let Ok(chained) = value.extract::<PyChainedSystemSets>() {
                        let members = chained.sets.bind(py);
                        let mut configs = Vec::new();
                        let mut identities = Vec::new();
                        let mut existing = 0usize;
                        for member in members.iter() {
                            let identity = system_set_config_identity(&member)?;
                            if let Some(installed) =
                                self.installed_set_config(world, schedule, &identity)
                            {
                                if installed != &identity {
                                    return Err(changed_set_config_error(&identity));
                                }
                                existing += 1;
                            }
                            configs.push(build_set_config(
                                &member,
                                generation,
                                self.error_state.clone(),
                                system_stage,
                            )?);
                            identities.push(identity);
                        }
                        if existing == identities.len() {
                            continue;
                        }
                        if existing != 0 {
                            return Err(PyRuntimeError::new_err(
                                "chained SystemSet configuration changed during hot reload; use run_scene to rebuild the schedule graph",
                            ));
                        }
                        self.pending_set_configs.push(PreparedSystemSetConfig {
                            schedule,
                            config: ScheduleConfigs::Configs {
                                configs,
                                collective_conditions: Vec::new(),
                                metadata: Chain::Chained(Default::default()),
                            },
                            identities,
                        });
                    } else {
                        let identity = system_set_config_identity(value)?;
                        if let Some(installed) =
                            self.installed_set_config(world, schedule, &identity)
                        {
                            if installed != &identity {
                                return Err(changed_set_config_error(&identity));
                            }
                            continue;
                        }
                        let config = build_set_config(
                            value,
                            generation,
                            self.error_state.clone(),
                            system_stage,
                        )?;
                        self.pending_set_configs.push(PreparedSystemSetConfig {
                            schedule,
                            config,
                            identities: vec![identity],
                        });
                    }
                }
            }
            Ok(())
        })
        .map_err(|error| reload_error_from_py(&error, "", false))
    }
}

fn changed_set_config_error(identity: &SystemSetConfigIdentity) -> PyErr {
    PyRuntimeError::new_err(format!(
        "SystemSet configuration for '{}' changed during hot reload; use run_scene to rebuild the schedule graph",
        identity.set().qualified_name()
    ))
}

impl ReloadRuntime for Pyo3ReloadRuntime {
    type Defs = PendingDefinitions;
    type SystemHandle = DynamicSystemHandle;

    fn load_definitions(&mut self, generation: u32) -> Result<PendingDefinitions, ReloadError> {
        let result = Python::attach(|py| -> PyResult<PendingDefinitions> {
            let temp_app = PyApp::create_reload_temp(generation);
            let temp_app_py = Py::new(py, temp_app)?;

            let create_app_bound = self.loader_func.bind(py).call0()?;
            let component_layout_changes = py
                .import("pybevy.decorators")?
                .getattr("_component_layout_reload_names")?
                .call0()?
                .extract::<Vec<String>>()?;
            let _result_app = create_app_bound.call1((temp_app_py.clone_ref(py),))?;

            let temp_app_ref = temp_app_py.borrow(py);
            Ok(PendingDefinitions {
                systems: temp_app_ref.take_pending_systems(),
                set_configs: temp_app_ref.take_pending_set_configs(),
                resources: temp_app_ref.take_pending_resources(),
                states: temp_app_ref.take_pending_states(),
                state_systems: temp_app_ref.take_pending_state_systems(),
                messages: temp_app_ref.take_pending_messages(),
                observers: temp_app_ref.take_pending_observers(),
                plugins: temp_app_ref.take_pending_plugins(),
                component_layout_changes,
            })
        });
        let result = result.map_err(|e| {
            let error = reload_error_from_py(&e, "Reload loader failed: ", true);
            Python::attach(|py| e.print(py));
            error
        });
        if let Ok(defs) = &result {
            self.component_layout_reload_pending = !defs.component_layout_changes.is_empty();
        }
        result
    }

    fn defs_fingerprint(&self, defs: &PendingDefinitions) -> DefsFingerprint {
        use std::hash::{Hash, Hasher};

        Python::attach(|py| {
            let mut startup_hasher = std::hash::DefaultHasher::new();
            let mut has_startup = false;
            for (stage, systems) in &defs.systems {
                if !stage.is_startup() || systems.is_empty() {
                    continue;
                }
                has_startup = true;
                // Both the startup stage and the registration order are part
                // of the identity: reordering changes execution order.
                format!("{stage:?}").hash(&mut startup_hasher);
                for sys in systems {
                    hash_system_code(py, sys.bind(py), &mut startup_hasher);
                }
            }

            // Resource identity is the type, not the value: Partial reloads
            // never re-insert resources, and re-applying initial values would
            // be a Full-reload semantic anyway.
            let mut resource_names: Vec<String> = defs
                .resources
                .iter()
                .map(|res| {
                    res.bind(py)
                        .get_type()
                        .fully_qualified_name()
                        .map(|name| name.to_string())
                        .unwrap_or_else(|_| "<unknown>".to_string())
                })
                .collect();
            resource_names.extend(defs.states.iter().map(|state| {
                state
                    .state_type
                    .bind(py)
                    .fully_qualified_name()
                    .map(|name| format!("State[{name}]"))
                    .unwrap_or_else(|_| "State[<unknown>]".to_string())
            }));
            resource_names.sort();
            let mut resources_hasher = std::hash::DefaultHasher::new();
            resource_names.hash(&mut resources_hasher);

            let mut observer_hasher = std::hash::DefaultHasher::new();
            for observer in &defs.observers {
                hash_system_code(py, observer.bind(py), &mut observer_hasher);
            }

            DefsFingerprint {
                startup_code: startup_hasher.finish(),
                resource_types: resources_hasher.finish(),
                observer_code: observer_hasher.finish(),
                component_layout_changed: !defs.component_layout_changes.is_empty(),
                has_startup,
                has_resources: !defs.resources.is_empty() || !defs.states.is_empty(),
                has_observers: !defs.observers.is_empty(),
            }
        })
    }

    fn plugin_names(&self, defs: &PendingDefinitions) -> Vec<String> {
        defs.plugins.clone()
    }

    fn system_names(&self, defs: &PendingDefinitions) -> std::collections::HashSet<String> {
        Python::attach(|py| {
            let mut names = std::collections::HashSet::new();
            for (_stage, systems) in &defs.systems {
                for sys in systems {
                    collect_system_names(sys.bind(py), &mut names);
                }
            }
            for pending in &defs.state_systems {
                for system in &pending.systems {
                    collect_system_names(system.bind(py), &mut names);
                }
            }
            names
        })
    }

    /// Register this generation's systems, or gut the ones already added.
    ///
    /// A failure partway through leaves earlier systems in the schedule already
    /// carrying this generation, so they would run a half-applied scene. The
    /// orchestrator has committed the generation by this point and never sees
    /// handles from an error return, so retiring them here is what makes
    /// "reload cancelled" true: gutted systems resolve as retired and do
    /// nothing, and they release their Python callables instead of pinning
    /// them for the life of the process.
    fn register_systems(
        &mut self,
        world: &mut World,
        defs: PendingDefinitions,
        generation: u32,
    ) -> Result<Vec<DynamicSystemHandle>, ReloadError> {
        let mut system_handles: Vec<DynamicSystemHandle> = Vec::new();
        self.pending_set_configs.clear();
        self.prepare_set_configs(world, defs.set_configs, generation)?;

        // Drain any errors left over from a previous reload attempt. The error
        // queue is shared across reloads, and a later error check would otherwise
        // pick up a stale failure even when this attempt added every system cleanly,
        // leaving the JSON `failure_reason` permanently sticky. Drop drained PyErrs
        // only after releasing the queue mutex.
        let stale_errors = {
            let mut error_lock = lock_or_recover(&self.error_state);
            std::mem::take(&mut *error_lock)
        };
        drop(stale_errors);

        // Reloaded systems write buffered errors into the same off-world slot the
        // Last-schedule drain reads. It lives on the world as `LastErrorBuffer`,
        // inserted at app construction; fall back to a throwaway if somehow absent.
        let error_buffer: SystemErrorBuffer = world
            .get_resource::<LastErrorBuffer>()
            .map(|r| r.buffer.clone())
            .unwrap_or_else(|| {
                debug_assert!(false, "LastErrorBuffer inserted at app construction");
                let buffer = SystemErrorBuffer::default();
                world.insert_resource(LastErrorBuffer {
                    buffer: buffer.clone(),
                });
                buffer
            });

        for (stage, systems) in defs.systems {
            if systems.is_empty() {
                continue;
            }

            if is_verbose() {
                eprintln!("   → Adding {} systems to {:?}", systems.len(), stage);
            }

            let label = stage.intern_label();
            let system_stage = if stage.is_startup() {
                SystemStage::Startup
            } else {
                SystemStage::UpdateOrLast
            };

            // Ensure the schedule exists
            {
                let mut schedules = world.resource_mut::<Schedules>();
                if !schedules.contains(label) {
                    if is_verbose() {
                        eprintln!(
                            "   → Creating {:?} schedule (not present during reload)",
                            stage
                        );
                    }
                    schedules.insert(Schedule::new(label));
                }
            }

            world.schedule_scope(label, |_world, schedule| {
                add_systems_to_schedule(
                    schedule,
                    systems,
                    generation,
                    &self.error_state,
                    &error_buffer,
                    system_stage,
                    stage,
                    &mut system_handles,
                )
            });

            // Check for errors stored during system addition
            let error = lock_or_recover(&self.error_state).pop();
            if let Some(error) = error {
                let error = reload_error_from_py(&error, "", false);
                self.clear_param_cache();
                self.retire_handles(&system_handles);
                return Err(error);
            }
        }

        for pending in defs.state_systems {
            let label = Python::attach(|py| -> PyResult<ReloadStateSchedule> {
                let schedule = pending.schedule.bind(py);
                if let Ok(on_enter) = schedule.cast::<PyOnEnterSchedule>() {
                    return Ok(ReloadStateSchedule::State(
                        canonicalize_state_schedule_label(
                            world,
                            on_enter.borrow().to_bevy_label(py)?,
                        ),
                    ));
                }
                if let Ok(on_exit) = schedule.cast::<PyOnExitSchedule>() {
                    return Ok(ReloadStateSchedule::State(
                        canonicalize_state_schedule_label(
                            world,
                            on_exit.borrow().to_bevy_label(py)?,
                        ),
                    ));
                }
                if let Ok(on_transition) = schedule.cast::<PyOnTransitionSchedule>() {
                    return Ok(ReloadStateSchedule::Transition(
                        canonicalize_transition_schedule_label(
                            world,
                            on_transition.borrow().to_bevy_label(py)?,
                        ),
                    ));
                }
                Err(PyRuntimeError::new_err(
                    "invalid state schedule collected during reload",
                ))
            })
            .map_err(|error| reload_error_from_py(&error, "", false))?;

            macro_rules! register_state_systems {
                ($label:expr) => {{
                    let label = $label;
                    if !world.resource::<Schedules>().contains(label.clone()) {
                        world
                            .resource_mut::<Schedules>()
                            .insert(Schedule::new(label.clone()));
                    }
                    world.schedule_scope(label, |_world, schedule| {
                        schedule.set_executor(SingleThreadedExecutor::new());
                        for system in pending.systems {
                            let result = Python::attach(|py| {
                                build_scheduled_system(
                                    system.bind(py),
                                    generation,
                                    self.error_state.clone(),
                                    error_buffer.clone(),
                                    SystemStage::UpdateOrLast,
                                    false,
                                )
                            });
                            match result {
                                Ok((config, handles)) => {
                                    system_handles.extend(handles);
                                    schedule.add_systems(config);
                                }
                                Err(error) => {
                                    lock_or_recover(&self.error_state).push(error);
                                    break;
                                }
                            }
                        }
                    });
                }};
            }

            match label {
                ReloadStateSchedule::State(label) => register_state_systems!(label),
                ReloadStateSchedule::Transition(label) => register_state_systems!(label),
            }

            let error = lock_or_recover(&self.error_state).pop();
            if let Some(error) = error {
                self.retire_handles(&system_handles);
                return Err(reload_error_from_py(&error, "", false));
            }
        }

        Ok(system_handles)
    }

    fn commit_schedule_configs(&mut self, world: &mut World) {
        for prepared in self.pending_set_configs.drain(..) {
            let label = prepared.schedule.intern_label();
            if !world.resource::<Schedules>().contains(label) {
                world
                    .resource_mut::<Schedules>()
                    .insert(Schedule::new(label));
            }
            world.schedule_scope(label, |_world, schedule| {
                schedule.configure_sets(prepared.config);
            });
            let mut installed =
                world.get_resource_or_insert_with(InstalledSystemSetConfigs::default);
            for identity in prepared.identities {
                installed.insert(prepared.schedule, identity);
            }
        }
        if self.component_layout_reload_pending {
            let cleared = Python::attach(|py| {
                py.import("pybevy.decorators")?
                    .getattr("_commit_component_layout_reload")?
                    .call0()?;
                Ok::<(), PyErr>(())
            });
            if let Err(error) = cleared {
                eprintln!("Could not commit the custom component layout reload: {error}");
            } else {
                self.component_layout_reload_pending = false;
            }
        }
    }

    fn register_resources(
        &mut self,
        world: &mut World,
        defs: &PendingDefinitions,
    ) -> Result<(), ReloadError> {
        if defs.resources.is_empty() {
            return Ok(());
        }
        Python::attach(|py| -> PyResult<()> {
            PyWorld::with_temporary(world, py, |py_world| {
                for resource in &defs.resources {
                    let resource_bound = resource.clone_ref(py).into_bound(py);
                    py_world.insert_resource(py, resource_bound)?;
                }
                Ok(())
            })
        })
        .map_err(|error| reload_error_from_py(&error, "", false))
    }

    fn rebind_resources(
        &mut self,
        world: &mut World,
        _defs: &PendingDefinitions,
    ) -> Result<(), ReloadError> {
        Python::attach(|py| -> PyResult<()> {
            let resource_types = world
                .get_resource::<pybevy_core::CustomResourceInfo>()
                .map(|info| {
                    info.iter()
                        .filter_map(|(_, entry)| {
                            entry.type_object.as_ref().map(|ty| ty.clone_ref(py))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            for resource_type in resource_types {
                let resource_type = resource_type.into_bound(py).cast_into::<PyType>()?;
                register_custom_resource(world, resource_type.as_type_ptr(), py);
            }
            Ok(())
        })
        .map_err(|error| reload_error_from_py(&error, "", false))
    }

    fn register_states(
        &mut self,
        world: &mut World,
        defs: &PendingDefinitions,
        mode: ReloadMode,
    ) -> Result<(), ReloadError> {
        Python::attach(|py| -> PyResult<()> {
            for state in &defs.states {
                register_reloaded_state_machine(
                    py,
                    world,
                    state.state_type.clone_ref(py),
                    state.initial_state.clone_ref(py),
                    mode == ReloadMode::Full,
                )?;
            }
            if !defs.states.is_empty() {
                ensure_state_transition_system_registered(world);
            }
            Ok(())
        })
        .map_err(|error| reload_error_from_py(&error, "", false))
    }

    fn register_messages(
        &mut self,
        world: &mut World,
        defs: &PendingDefinitions,
        generation: u32,
    ) -> Result<(), ReloadError> {
        if defs.messages.is_empty() {
            return Ok(());
        }
        Python::attach(|py| -> PyResult<()> {
            for msg_type in &defs.messages {
                let bound = msg_type.bind(py);
                let outcome = register_python_message(py, world, bound, generation)?;
                if is_verbose() {
                    eprintln!(
                        "   → Registered message '{}' as {:?}",
                        bound.name()?,
                        outcome
                    );
                }
            }
            Ok(())
        })
        .map_err(|error| reload_error_from_py(&error, "", false))
    }

    fn register_observers(
        &mut self,
        world: &mut World,
        defs: &PendingDefinitions,
    ) -> Result<(), ReloadError> {
        if defs.observers.is_empty() {
            return Ok(());
        }
        Python::attach(|py| -> PyResult<()> {
            // Clear old observers and despawn their entities
            let old_entries = world
                .get_resource_mut::<ObserverRegistry>()
                .map(|mut registry| registry.clear_all());

            if let Some(old_entries) = old_entries {
                for entry in &old_entries {
                    if world.get_entity(entry.observer_entity).is_ok() {
                        world.despawn(entry.observer_entity);
                    }
                }
                // Prepared Python handles drop only after the registry borrow
                // and observer-entity despawns have both completed.
                drop(old_entries);
            }

            // Register new observers
            for observer_func in &defs.observers {
                let func_bound = observer_func.bind(py);
                ObserverRegistry::register_observer(py, func_bound, world)?;
            }

            if is_verbose() {
                eprintln!("   → Re-registered {} observers", defs.observers.len());
            }

            Ok(())
        })
        .map_err(|error| reload_error_from_py(&error, "", false))
    }

    fn register_handles(
        &mut self,
        world: &mut World,
        generation: u32,
        handles: Vec<DynamicSystemHandle>,
    ) {
        if let Some(mut registry) = world.get_resource_mut::<DynamicSystemRegistry>() {
            for handle in handles {
                registry.register(generation, handle);
            }
            let keep_after = generation.saturating_sub(KEEP_ALIVE_GENERATIONS);
            let retired_generations =
                registry.cleanup_old_generations(keep_after, retire_main_handle);
            drop(registry);
            super::systems::queue_schedule_compaction(world, retired_generations);

            if is_verbose() {
                eprintln!(
                    "   → Registered system handles for gen {} and gutted systems older than gen {}",
                    generation, keep_after
                );
            }
        }
    }

    fn retire_handles(&mut self, handles: &[DynamicSystemHandle]) {
        for handle in handles {
            retire_main_handle(handle);
        }
    }

    fn take_pending_system_error(&mut self, world: &mut World) -> Option<String> {
        let buffer = world.get_resource::<LastErrorBuffer>()?.buffer.clone();
        let error = lock_or_recover(&buffer).take()?;
        Some(match error.traceback {
            Some(traceback) if !traceback.is_empty() => {
                format!("{}\n{}", error.error, traceback)
            }
            _ => error.error,
        })
    }

    fn prune_messages(&mut self, world: &mut World, keep_after_generation: u32) {
        let keep_after = keep_after_generation.saturating_sub(KEEP_ALIVE_GENERATIONS);
        prune_python_message_aliases(world, keep_after);
        if let Some(mut registry) =
            world.get_resource_mut::<pybevy_core::custom_component::CustomComponentRegistry>()
        {
            registry.prune_aliases(keep_after);
        }
        if let Some(mut registry) =
            world.get_resource_mut::<pybevy_core::custom_resource::CustomResourceRegistry>()
        {
            registry.prune_aliases(keep_after);
        }
    }

    fn clear_custom_resources(&mut self, world: &mut World, verbose: bool) {
        clear_python_messages(world);
        cleanup::clear_custom_resources(world, verbose);
        // Generated State[T]/NextState[T] descriptors are cached against the
        // user's @state enum, so the cache pins one scene generation per
        // reload unless it is dropped here with the rest of the class tables.
        if let Err(error) = Python::attach(crate::ecs::state::clear_state_resource_type_caches)
            && verbose
        {
            eprintln!("   → Could not clear state resource caches: {error}");
        }
        // Full reload is the only caller of this hook. Drop the control-facing
        // class table so a component removed from the new scene cannot be
        // constructed through MCP using a rollback-generation type alias.
        // Live classes repopulate the table when Startup or registration paths
        // call register_custom_component; Partial reload never clears it.
        let retired_info = world
            .get_resource_mut::<pybevy_core::CustomComponentInfo>()
            .map(|mut info| std::mem::take(&mut *info));
        // Releasing retained Python classes can run finalization, so do it
        // after the Bevy resource borrow has ended and while attached.
        Python::attach(|_| drop(retired_info));
    }

    fn snapshot_native_resources(&self, world: &World) -> HashSet<TypeId> {
        let mut initial = HashSet::new();
        for bridge in pybevy_core::registry::global_registry::all_resource_bridges() {
            if bridge.contains_in_world(world) {
                initial.insert(bridge.bevy_type_id());
            }
        }
        initial
    }

    fn clear_native_resources(&self, world: &mut World, initial: &HashSet<TypeId>, verbose: bool) {
        for bridge in pybevy_core::registry::global_registry::all_resource_bridges() {
            let type_id = bridge.bevy_type_id();
            if bridge.preserve_on_reload() {
                if verbose && bridge.contains_in_world(world) {
                    eprintln!("   → Preserved engine resource {}", bridge.name());
                }
            } else if initial.contains(&type_id) {
                // Bevy-plugin resource: reset to default
                if !bridge.reset_to_default(world) && verbose {
                    eprintln!("   → Cannot reset {} (no Default)", bridge.name());
                }
            } else if bridge.contains_in_world(world) {
                // User-only resource: remove entirely
                bridge.remove(world);
                if verbose {
                    eprintln!("   → Removed user resource {}", bridge.name());
                }
            }
        }
    }

    fn detect_system_delta(
        &mut self,
        world: &mut World,
        new_systems: std::collections::HashSet<String>,
    ) -> Vec<String> {
        if let Some(mut registry) = world.get_resource_mut::<DynamicSystemRegistry>() {
            registry.detect_system_delta(new_systems)
        } else {
            Vec::new()
        }
    }

    fn clear_param_cache(&mut self) {
        crate::ecs::dynamic_system::clear_system_param_cache();
    }

    fn trigger_gc(&mut self) {
        Python::attach(|py| {
            if let Ok(gc) = py.import("gc") {
                let _ = gc.call_method0("collect");
            }
        });
    }

    fn gc_object_count(&self) -> usize {
        get_python_gc_objects()
    }
}
