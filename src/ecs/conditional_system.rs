use std::sync::{Arc, Mutex};

use bevy::ecs::{
    schedule::{IntoScheduleConfigs, ScheduleConfigs},
    system::ScheduleSystem,
};
use pybevy_reload::SystemStage;
use pyo3::{prelude::*, types::PyDict};

use crate::ecs::system_interpreter::{MainDynamicSystem as DynamicSystem, new_main_condition};

/// Wrapper for a system with a run condition
/// Similar to Bevy's IntoSystemConfigs::run_if()
#[pyclass(name = "ConditionalSystem", from_py_object)]
pub struct PyConditionalSystem {
    /// The system function to run
    pub system: Py<PyAny>,
    /// The condition function that must return bool
    pub condition: Py<PyAny>,
}

impl Clone for PyConditionalSystem {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            system: self.system.clone_ref(py),
            condition: self.condition.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyConditionalSystem {
    #[new]
    pub fn new(system: Py<PyAny>, condition: Py<PyAny>) -> Self {
        Self { system, condition }
    }

    /// Proxy to the inner system's __name__ for introspection paths.
    #[getter]
    pub fn __name__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.system.bind(py).getattr("__name__")?.unbind())
    }

    /// Combine this condition with another using AND logic
    /// Both conditions must return true for the system to run
    /// Usage: run_if(system, condition1).and_(condition2)
    pub fn and_(&self, condition: Py<PyAny>) -> PyResult<Self> {
        Python::attach(|py| {
            // Create a Python function that combines both conditions with AND
            // Use functools.wraps to preserve the first condition's annotations
            let self_condition = self.condition.clone_ref(py);
            let functools = py.import("functools")?;

            let locals = PyDict::new(py);
            locals.set_item("c1", self_condition)?;
            locals.set_item("c2", condition)?;
            locals.set_item("functools", functools)?;

            let code = c"\
def make_and(c1, c2, functools):
    @functools.wraps(c1)
    def combined(*args, **kwargs):
        return c1(*args, **kwargs) and c2(*args, **kwargs)
    return combined
result = make_and(c1, c2, functools)";
            py.run(code, None, Some(&locals))?;

            let combined = locals.get_item("result")?.unwrap().unbind();

            Ok(Self {
                system: self.system.clone_ref(py),
                condition: combined,
            })
        })
    }

    /// Combine this condition with another using OR logic
    /// Either condition must return true for the system to run
    /// Usage: run_if(system, condition1).or_(condition2)
    pub fn or_(&self, condition: Py<PyAny>) -> PyResult<Self> {
        Python::attach(|py| {
            // Create a Python function that combines both conditions with OR
            // Use functools.wraps to preserve the first condition's annotations
            let self_condition = self.condition.clone_ref(py);
            let functools = py.import("functools")?;

            let locals = PyDict::new(py);
            locals.set_item("c1", self_condition)?;
            locals.set_item("c2", condition)?;
            locals.set_item("functools", functools)?;

            let code = c"\
def make_or(c1, c2, functools):
    @functools.wraps(c1)
    def combined(*args, **kwargs):
        return c1(*args, **kwargs) or c2(*args, **kwargs)
    return combined
result = make_or(c1, c2, functools)";
            py.run(code, None, Some(&locals))?;

            let combined = locals.get_item("result")?.unwrap().unbind();

            Ok(Self {
                system: self.system.clone_ref(py),
                condition: combined,
            })
        })
    }

    /// Negate this condition using NOT logic
    /// The condition must return false for the system to run
    /// Usage: run_if(system, condition).not_()
    pub fn not_(&self) -> PyResult<Self> {
        Python::attach(|py| {
            // Create a Python function that negates the condition
            // Use functools.wraps to preserve the condition's annotations
            let self_condition = self.condition.clone_ref(py);
            let functools = py.import("functools")?;

            let locals = PyDict::new(py);
            locals.set_item("c", self_condition)?;
            locals.set_item("functools", functools)?;

            let code = c"\
def make_not(c, functools):
    @functools.wraps(c)
    def negated(*args, **kwargs):
        return not c(*args, **kwargs)
    return negated
result = make_not(c, functools)";
            py.run(code, None, Some(&locals))?;

            let negated = locals.get_item("result")?.unwrap().unbind();

            Ok(Self {
                system: self.system.clone_ref(py),
                condition: negated,
            })
        })
    }
}

/// Helper function to create a conditional system
/// Usage: run_if(system_func, condition_func)
#[pyfunction]
pub fn run_if(system: Py<PyAny>, condition: Py<PyAny>) -> PyResult<PyConditionalSystem> {
    Ok(PyConditionalSystem::new(system, condition))
}

/// Apply the appropriate run_if (DynamicCondition or generation-gated closure) to a DynamicSystem.
pub(crate) fn build_conditional_system_config(
    dynamic_system: DynamicSystem,
    condition: Py<PyAny>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    system_stage: SystemStage,
    _is_startup: bool,
) -> PyResult<ScheduleConfigs<ScheduleSystem>> {
    let dynamic_condition = new_main_condition(condition, generation, error_state, system_stage)?;
    Ok(dynamic_system.run_if(dynamic_condition))
}
