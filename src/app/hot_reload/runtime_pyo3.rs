use std::{
    any::TypeId,
    collections::HashSet,
    sync::{Arc, Mutex},
};

use bevy::ecs::{
    schedule::{Chain, IntoScheduleConfigs, Schedule, ScheduleConfigs, Schedules},
    world::World,
};
use pybevy_reload::{
    DefsFingerprint, KEEP_ALIVE_GENERATIONS, ReloadError, ReloadRuntime, SystemStage,
    generation_matches, is_verbose, lock_or_recover, startup_or_reload,
};
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyType};

use super::{cleanup, registry::DynamicSystemRegistry, util::get_python_gc_objects};
use crate::{
    app::{PyStage, app::PyApp, chained_systems::PyChainedSystems},
    ecs::{
        conditional_system::{PyConditionalSystem, build_conditional_system_config},
        dynamic_system::{DynamicSystem, DynamicSystemHandle, LastErrorBuffer, SystemErrorBuffer},
        messages::MessageRegistry,
        observer_registry::ObserverRegistry,
        world::PyWorld,
    },
};

/// Container for definitions loaded from the Python loader function.
pub(crate) struct PendingDefinitions {
    pub systems: Vec<(PyStage, Vec<Py<PyAny>>)>,
    pub resources: Vec<Py<PyAny>>,
    pub messages: Vec<Py<PyType>>,
    pub observers: Vec<Py<PyAny>>,
    pub plugins: Vec<String>,
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

            if let Ok(conditional) = system_bound.extract::<PyConditionalSystem>() {
                let system_inner = conditional.system.clone_ref(py);
                let condition = conditional.condition;

                let dynamic_system = DynamicSystem::new(
                    system_inner,
                    generation,
                    error_state.clone(),
                    error_buffer.clone(),
                    system_stage,
                )?;
                system_handles.push(dynamic_system.handle());

                let config = build_conditional_system_config(
                    dynamic_system,
                    condition,
                    generation,
                    error_state.clone(),
                    system_stage,
                    is_startup,
                )?;
                schedule.add_systems(config);
            } else if let Ok(chained) = system_bound.extract::<PyChainedSystems>() {
                let systems_tuple = chained.systems.bind(py);
                let mut dynamic_systems = Vec::new();

                for sys in systems_tuple.iter() {
                    let dynamic_system = DynamicSystem::new(
                        sys.unbind(),
                        generation,
                        error_state.clone(),
                        error_buffer.clone(),
                        system_stage,
                    )?;
                    system_handles.push(dynamic_system.handle());
                    dynamic_systems.push(dynamic_system);
                }

                if dynamic_systems.is_empty() {
                    return Err(PyRuntimeError::new_err("Empty chained systems"));
                }

                let configs: Vec<ScheduleConfigs<_>> = dynamic_systems
                    .into_iter()
                    .map(|sys| {
                        if is_startup {
                            sys.run_if(startup_or_reload(generation))
                        } else {
                            sys.run_if(generation_matches(generation))
                        }
                    })
                    .collect();

                let chained = ScheduleConfigs::Configs {
                    configs,
                    collective_conditions: Vec::new(),
                    metadata: Chain::Chained(Default::default()),
                };

                schedule.add_systems(chained);
            } else {
                let dynamic_system = DynamicSystem::new(
                    system_func,
                    generation,
                    error_state.clone(),
                    error_buffer.clone(),
                    system_stage,
                )?;
                system_handles.push(dynamic_system.handle());
                if is_startup {
                    schedule.add_systems(dynamic_system.run_if(startup_or_reload(generation)));
                } else {
                    schedule.add_systems(dynamic_system.run_if(generation_matches(generation)));
                }
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
    if let Ok(conditional) = sys_bound.extract::<PyConditionalSystem>() {
        hash_callable_code(py, conditional.system.bind(py), hasher);
        hash_callable_code(py, conditional.condition.bind(py), hasher);
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
}

impl ReloadRuntime for Pyo3ReloadRuntime {
    type Defs = PendingDefinitions;
    type SystemHandle = DynamicSystemHandle;

    fn load_definitions(&mut self, generation: u32) -> Result<PendingDefinitions, ReloadError> {
        Python::attach(|py| -> PyResult<PendingDefinitions> {
            let temp_app = PyApp::create_reload_temp(generation);
            let temp_app_py = Py::new(py, temp_app)?;

            let create_app_bound = self.loader_func.bind(py).call0()?;
            let _result_app = create_app_bound.call1((temp_app_py.clone_ref(py),))?;

            let temp_app_ref = temp_app_py.borrow(py);
            Ok(PendingDefinitions {
                systems: temp_app_ref.take_pending_systems(),
                resources: temp_app_ref.take_pending_resources(),
                messages: temp_app_ref.take_pending_messages(),
                observers: temp_app_ref.take_pending_observers(),
                plugins: temp_app_ref.take_pending_plugins(),
            })
        })
        .map_err(|e| {
            let message = format!("Reload loader failed: {}", e);
            Python::attach(|py| e.print(py));
            ReloadError {
                message,
                is_load_failure: true,
            }
        })
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
                has_startup,
                has_resources: !defs.resources.is_empty(),
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
                    let sys_bound = sys.bind(py);
                    if let Ok(conditional) = sys_bound.extract::<PyConditionalSystem>() {
                        if let Ok(name) = conditional.system.bind(py).getattr("__name__")
                            && let Ok(s) = name.extract::<String>()
                        {
                            names.insert(s);
                        }
                    } else if let Ok(chained) = sys_bound.extract::<PyChainedSystems>() {
                        for inner in chained.systems.bind(py).iter() {
                            if let Ok(name) = inner.getattr("__name__")
                                && let Ok(s) = name.extract::<String>()
                            {
                                names.insert(s);
                            }
                        }
                    } else if let Ok(name) = sys_bound.getattr("__name__")
                        && let Ok(s) = name.extract::<String>()
                    {
                        names.insert(s);
                    }
                }
            }
            names
        })
    }

    fn register_systems(
        &mut self,
        world: &mut World,
        defs: PendingDefinitions,
        generation: u32,
    ) -> Result<Vec<DynamicSystemHandle>, ReloadError> {
        let mut system_handles: Vec<DynamicSystemHandle> = Vec::new();

        // Drain any errors left over from a previous reload attempt. The error
        // queue is shared across reloads, and `error_lock.last()` below would
        // otherwise pick up a stale failure even when this attempt added every
        // system cleanly, leaving the JSON `failure_reason` permanently sticky.
        {
            let mut error_lock = lock_or_recover(&self.error_state);
            error_lock.clear();
        }

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
            {
                let error_lock = lock_or_recover(&self.error_state);
                if let Some(err) = error_lock.last() {
                    let msg = err.to_string();
                    drop(error_lock);
                    self.clear_param_cache();
                    return Err(ReloadError {
                        message: msg,
                        is_load_failure: false,
                    });
                }
            }
        }

        Ok(system_handles)
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
        .map_err(|e| ReloadError {
            message: e.to_string(),
            is_load_failure: false,
        })
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
                let type_ptr = bound.as_type_ptr();

                let already_registered = world
                    .get_resource::<MessageRegistry>()
                    .is_some_and(|reg| reg.get(type_ptr).is_some());

                if !already_registered {
                    let class_name = bound
                        .getattr("__name__")
                        .and_then(|n| n.extract::<String>())
                        .unwrap_or_default();

                    if let Some(mut registry) = world.get_resource_mut::<MessageRegistry>()
                        && registry.alias_by_name(
                            type_ptr,
                            &class_name,
                            msg_type.clone_ref(py),
                            generation,
                        )
                        && is_verbose()
                    {
                        eprintln!(
                            "   → Aliased message '{}' with new type pointer",
                            class_name
                        );
                    }
                }
            }
            Ok(())
        })
        .map_err(|e| ReloadError {
            message: e.to_string(),
            is_load_failure: false,
        })
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
        .map_err(|e| ReloadError {
            message: e.to_string(),
            is_load_failure: false,
        })
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
            registry.cleanup_old_generations(keep_after);

            if is_verbose() {
                eprintln!(
                    "   → Registered system handles for gen {} and gutted systems older than gen {}",
                    generation, keep_after
                );
            }
        }
    }

    fn prune_messages(&mut self, world: &mut World, keep_after_generation: u32) {
        if let Some(mut msg_registry) = world.get_resource_mut::<MessageRegistry>() {
            let keep_after = keep_after_generation.saturating_sub(KEEP_ALIVE_GENERATIONS);
            msg_registry.prune_old_generations(keep_after);
        }
    }

    fn clear_custom_resources(&mut self, world: &mut World, verbose: bool) {
        cleanup::clear_custom_resources(world, verbose);
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
            if initial.contains(&type_id) {
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

    fn print_error(&self, error: &ReloadError) {
        eprintln!(
            "❌ [Hot Reload] {} — old systems still running",
            error.message
        );
    }
}
