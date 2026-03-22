/// Dynamic condition system that can take system parameters and returns bool
///
/// This is a specialized version of DynamicSystem that:
/// - Returns bool instead of ()
/// - Extracts the bool return value from the Python function
/// - Can be used with Bevy's run_if() as a proper condition system
use std::sync::{Arc, Mutex};

use bevy::{
    ecs::{
        change_detection::{CheckChangeTicks, Tick},
        system::{ReadOnlySystem, RunSystemError, System, SystemIn, SystemParamValidationError},
        world::{DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
    },
    prelude::*,
};
use pyo3::prelude::*;

use crate::ecs::dynamic_system::DynamicSystem;

/// A condition system that wraps a Python function returning bool
/// Unlike DynamicSystem which returns (), this extracts and returns the bool result
pub struct DynamicCondition {
    /// The underlying DynamicSystem that does all the parameter handling
    inner: DynamicSystem,
    /// Expected generation for hot reload
    generation: u32,
    /// System stage (Startup vs Update/Last)
    stage: crate::app::hot_reload::SystemStage,
}

impl DynamicCondition {
    pub fn new(
        func: Py<PyAny>,
        generation: u32,
        error_state: Arc<Mutex<Vec<PyErr>>>,
        stage: crate::app::hot_reload::SystemStage,
    ) -> PyResult<Self> {
        let wrapped_func = Python::attach(|py| -> PyResult<Py<PyAny>> {
            // Import functools
            let functools = py.import("functools")?;

            // Create Python wrapper that stores the return value
            let wrapper_code = c"
def create_wrapper(original_func, result_container, functools):
    @functools.wraps(original_func)
    def wrapper(*args, **kwargs):
        try:
            ret = original_func(*args, **kwargs)
            # Store the bool result
            result_container['value'] = bool(ret) if ret is not None else False
            return ret
        except Exception as e:
            result_container['value'] = False
            raise
    # functools.wraps preserves __name__, __module__, __annotations__, __signature__, etc.
    return wrapper
";

            let locals = pyo3::types::PyDict::new(py);
            py.run(wrapper_code, None, Some(&locals))?;

            let create_wrapper = locals.get_item("create_wrapper")?.unwrap();
            let result_dict = pyo3::types::PyDict::new(py);
            result_dict.set_item("value", false)?;

            // Store the result dict so we can read from it
            let wrapper = create_wrapper.call1((func, &result_dict, functools))?;

            // We need to keep the result_dict alive and accessible
            // Store it in the wrapper as an attribute
            wrapper.setattr("_result_dict", &result_dict)?;

            Ok(wrapper.unbind())
        })?;

        let inner = DynamicSystem::new(wrapped_func, generation, error_state, stage)?;

        Ok(Self {
            inner,
            generation,
            stage,
        })
    }

    /// Get the last bool result after running the condition
    fn get_result(&self, py: Python) -> bool {
        // Try to get the result from the wrapper's _result_dict
        if let Ok(inner_func) = self.inner.get_cached_function() {
            if let Ok(result_dict) = inner_func.bind(py).getattr("_result_dict") {
                if let Ok(value) = result_dict.get_item("value") {
                    if let Ok(b) = value.extract::<bool>() {
                        return b;
                    }
                }
            }
        }
        false
    }
}

// Forward SystemParams to inner DynamicSystem
impl System for DynamicCondition {
    type In = ();
    type Out = bool;

    fn name(&self) -> DebugName {
        DebugName::owned(format!("condition_{}", self.inner.name()))
    }

    fn flags(&self) -> bevy::ecs::system::SystemStateFlags {
        self.inner.flags()
    }

    unsafe fn run_unsafe(
        &mut self,
        _input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        use crate::app::hot_reload::HotReloadGeneration;

        // First check generation (hot reload logic)
        let world_ref = unsafe { world.world() };
        let gen_check = match world_ref.get_resource::<HotReloadGeneration>() {
            Some(res) => match self.stage {
                crate::app::hot_reload::SystemStage::Startup => {
                    // Startup: run if current == expected OR current == expected + 1 (reload)
                    res.current == self.generation || res.current == self.generation + 1
                }
                crate::app::hot_reload::SystemStage::UpdateOrLast => {
                    // Update/Last: only run if current == expected
                    res.current == self.generation
                }
            },
            None => true, // No hot reload, all systems run
        };

        if !gen_check {
            return Ok(false);
        }

        // Run the inner system (which calls our wrapper that stores the result)
        unsafe { self.inner.run_unsafe((), world)? };

        // Extract and return the bool result
        Ok(Python::attach(|py| self.get_result(py)))
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.inner.apply_deferred(world);
    }

    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.inner.queue_deferred(world);
    }

    fn initialize(&mut self, world: &mut World) -> bevy::ecs::query::FilteredAccessSet {
        self.inner.initialize(world)
    }

    unsafe fn validate_param_unsafe(
        &mut self,
        world: UnsafeWorldCell,
    ) -> Result<(), SystemParamValidationError> {
        unsafe { self.inner.validate_param_unsafe(world) }
    }

    fn check_change_tick(&mut self, change_tick: CheckChangeTicks) {
        self.inner.check_change_tick(change_tick);
    }

    fn get_last_run(&self) -> Tick {
        self.inner.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.inner.set_last_run(last_run);
    }
}

// Conditions must be ReadOnlySystem (they don't mutate world state)
unsafe impl ReadOnlySystem for DynamicCondition {}
