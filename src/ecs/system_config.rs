use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use bevy::ecs::{
    schedule::{InternedSystemSet, IntoScheduleConfigs, ScheduleConfigs},
    system::ScheduleSystem,
};
use pybevy_ecs::shared::schedule::{DynamicSetLabel, ScheduleOrdering};
use pybevy_reload::{SystemStage, generation_matches, startup_or_reload};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyType,
};

use crate::ecs::{
    conditional_system::PyConditionalSystem,
    dynamic_system::{DynamicSystemHandle, SystemErrorBuffer},
    system_interpreter::{new_main_condition, new_main_persistent_condition, new_main_system},
};

/// A stable, interpreter-independent Bevy system-set identity.
#[pyclass(name = "SystemSet", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PySystemSet {
    pub(crate) label: DynamicSetLabel,
}

impl PySystemSet {
    fn from_qualified_name(qualified_name: String) -> Self {
        Self {
            label: DynamicSetLabel::named(qualified_name),
        }
    }

    fn config(&self) -> PySystemSetConfig {
        PySystemSetConfig {
            set: self.clone(),
            ordering: ScheduleOrdering::default(),
            conditions: Vec::new(),
        }
    }
}

#[pymethods]
impl PySystemSet {
    #[new]
    pub fn new(name: String) -> PyResult<Self> {
        let name = name.trim();
        if name.is_empty() {
            return Err(PyValueError::new_err("SystemSet name must not be empty"));
        }
        Ok(Self::from_qualified_name(name.to_owned()))
    }

    #[getter]
    pub fn name(&self) -> &str {
        self.label.qualified_name()
    }

    pub fn in_set(&self, parent: &Bound<'_, PyAny>) -> PyResult<PySystemSetConfig> {
        let mut config = self.config();
        let parent = system_set_value(parent)?.ok_or_else(|| {
            PyTypeError::new_err("in_set() parent must be a SystemSet or system-set enum member")
        })?;
        add_relation(
            &self.label,
            &mut config.ordering,
            parent.label,
            Relation::InSet,
        )?;
        Ok(config)
    }

    pub fn before(&self, target: &Bound<'_, PyAny>) -> PyResult<PySystemSetConfig> {
        let mut config = self.config();
        let target = ordering_target(target)?;
        add_relation(&self.label, &mut config.ordering, target, Relation::Before)?;
        Ok(config)
    }

    pub fn after(&self, target: &Bound<'_, PyAny>) -> PyResult<PySystemSetConfig> {
        let mut config = self.config();
        let target = ordering_target(target)?;
        add_relation(&self.label, &mut config.ordering, target, Relation::After)?;
        Ok(config)
    }

    pub fn run_if(&self, py: Python<'_>, condition: Py<PyAny>) -> PySystemSetConfig {
        let mut config = self.config();
        config.conditions.push(condition.clone_ref(py));
        config
    }

    fn __repr__(&self) -> String {
        format!("SystemSet({:?})", self.label.qualified_name())
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.label.hash(&mut hasher);
        hasher.finish()
    }

    fn __richcmp__(&self, other: &Self, op: pyo3::basic::CompareOp) -> bool {
        match op {
            pyo3::basic::CompareOp::Eq => self.label == other.label,
            pyo3::basic::CompareOp::Ne => self.label != other.label,
            _ => false,
        }
    }
}

/// Fluent configuration for one Python system callable.
#[pyclass(name = "SystemConfig", from_py_object)]
pub struct PySystemConfig {
    pub(crate) system: Py<PyAny>,
    pub(crate) ordering: ScheduleOrdering,
    pub(crate) conditions: Vec<Py<PyAny>>,
}

impl Clone for PySystemConfig {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            system: self.system.clone_ref(py),
            ordering: self.ordering.clone(),
            conditions: self
                .conditions
                .iter()
                .map(|condition| condition.clone_ref(py))
                .collect(),
        })
    }
}

#[pymethods]
impl PySystemConfig {
    #[new]
    pub fn new(system: Py<PyAny>) -> Self {
        Self {
            system,
            ordering: ScheduleOrdering::default(),
            conditions: Vec::new(),
        }
    }

    #[getter]
    pub fn __name__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.system.bind(py).getattr("__name__")?.unbind())
    }

    #[getter]
    pub fn __qualname__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.system.bind(py).getattr("__qualname__")?.unbind())
    }

    #[getter]
    pub fn __module__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.system.bind(py).getattr("__module__")?.unbind())
    }

    pub fn in_set(&self, set: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut result = self.clone();
        let set = system_set_value(set)?.ok_or_else(|| {
            PyTypeError::new_err("in_set() value must be a SystemSet or system-set enum member")
        })?;
        result.ordering.push_in_set(set.label);
        Ok(result)
    }

    pub fn before(&self, target: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut result = self.clone();
        result.ordering.push_before(ordering_target(target)?);
        Ok(result)
    }

    pub fn after(&self, target: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut result = self.clone();
        result.ordering.push_after(ordering_target(target)?);
        Ok(result)
    }

    pub fn run_if(&self, py: Python<'_>, condition: Py<PyAny>) -> Self {
        let mut result = self.clone();
        result.conditions.push(condition.clone_ref(py));
        result
    }
}

/// Fluent configuration for one Python system set.
#[pyclass(name = "SystemSetConfig", from_py_object)]
pub struct PySystemSetConfig {
    pub(crate) set: PySystemSet,
    pub(crate) ordering: ScheduleOrdering,
    pub(crate) conditions: Vec<Py<PyAny>>,
}

impl Clone for PySystemSetConfig {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            set: self.set.clone(),
            ordering: self.ordering.clone(),
            conditions: self
                .conditions
                .iter()
                .map(|condition| condition.clone_ref(py))
                .collect(),
        })
    }
}

#[pymethods]
impl PySystemSetConfig {
    pub fn in_set(&self, parent: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut result = self.clone();
        let parent = system_set_value(parent)?.ok_or_else(|| {
            PyTypeError::new_err("in_set() parent must be a SystemSet or system-set enum member")
        })?;
        add_relation(
            &self.set.label,
            &mut result.ordering,
            parent.label,
            Relation::InSet,
        )?;
        Ok(result)
    }

    pub fn before(&self, target: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut result = self.clone();
        let target = ordering_target(target)?;
        add_relation(
            &self.set.label,
            &mut result.ordering,
            target,
            Relation::Before,
        )?;
        Ok(result)
    }

    pub fn after(&self, target: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut result = self.clone();
        let target = ordering_target(target)?;
        add_relation(
            &self.set.label,
            &mut result.ordering,
            target,
            Relation::After,
        )?;
        Ok(result)
    }

    pub fn run_if(&self, py: Python<'_>, condition: Py<PyAny>) -> Self {
        let mut result = self.clone();
        result.conditions.push(condition.clone_ref(py));
        result
    }
}

#[pyfunction]
#[pyo3(signature = (system, /))]
pub fn system(system: Py<PyAny>) -> PySystemConfig {
    PySystemConfig::new(system)
}

#[pyfunction]
#[pyo3(signature = (cls, /))]
pub fn system_set(cls: &Bound<'_, PyType>) -> PyResult<PySystemSet> {
    let qualified_name = qualified_name(cls.as_any(), "system set")?;
    Ok(PySystemSet::from_qualified_name(qualified_name))
}

pub(crate) fn callable_set(callable: &Bound<'_, PyAny>) -> PyResult<DynamicSetLabel> {
    Ok(DynamicSetLabel::callable(qualified_name(
        callable,
        "system callable",
    )?))
}

/// Build one Main/PyO3 system config from a plain callable or public wrapper.
///
/// Generation gating, user conditions, callable identity, and explicit
/// ordering all pass through this single path so initial registration and hot
/// reload cannot silently diverge.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_scheduled_system(
    value: &Bound<'_, PyAny>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
    system_stage: SystemStage,
    is_startup: bool,
) -> PyResult<(ScheduleConfigs<ScheduleSystem>, DynamicSystemHandle)> {
    let py = value.py();
    let (callable, ordering, conditions) = if let Ok(config) = value.extract::<PySystemConfig>() {
        (config.system, config.ordering, config.conditions)
    } else if let Ok(conditional) = value.extract::<PyConditionalSystem>() {
        (
            conditional.system,
            ScheduleOrdering::default(),
            vec![conditional.condition],
        )
    } else {
        (
            value.clone().unbind(),
            ScheduleOrdering::default(),
            Vec::new(),
        )
    };

    let identity = callable_set(callable.bind(py))?;
    let dynamic_system = new_main_system(
        callable,
        generation,
        error_state.clone(),
        error_buffer,
        system_stage,
    )?;
    let handle = dynamic_system.handle().clone();
    let mut config = if is_startup {
        dynamic_system.run_if(startup_or_reload(generation))
    } else {
        dynamic_system.run_if(generation_matches(generation))
    };

    for condition in conditions {
        let condition =
            new_main_condition(condition, generation, error_state.clone(), system_stage)?;
        config = config.run_if(condition);
    }

    Ok((apply_system_ordering(config, identity, &ordering), handle))
}

pub(crate) fn build_set_config(
    value: &Bound<'_, PyAny>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    system_stage: SystemStage,
) -> PyResult<ScheduleConfigs<InternedSystemSet>> {
    let (set, ordering, conditions) = if let Ok(config) = value.extract::<PySystemSetConfig>() {
        (config.set, config.ordering, config.conditions)
    } else if let Some(set) = system_set_value(value)? {
        (set, ScheduleOrdering::default(), Vec::new())
    } else {
        return Err(PyTypeError::new_err(
            "configure_sets() values must be SystemSet or SystemSetConfig objects",
        ));
    };

    let mut config = apply_set_ordering(set.label.into_configs(), &ordering);
    for condition in conditions {
        let condition = new_main_persistent_condition(
            condition,
            generation,
            error_state.clone(),
            system_stage,
        )?;
        config = config.run_if(condition);
    }
    Ok(config)
}

pub(crate) fn apply_system_ordering(
    mut config: ScheduleConfigs<ScheduleSystem>,
    callable: DynamicSetLabel,
    ordering: &ScheduleOrdering,
) -> ScheduleConfigs<ScheduleSystem> {
    config = config.in_set(callable);
    for set in &ordering.in_sets {
        config = config.in_set(set.clone());
    }
    for target in &ordering.before {
        config = config.before(target.clone());
    }
    for target in &ordering.after {
        config = config.after(target.clone());
    }
    config
}

pub(crate) fn apply_set_ordering(
    mut config: ScheduleConfigs<InternedSystemSet>,
    ordering: &ScheduleOrdering,
) -> ScheduleConfigs<InternedSystemSet> {
    for parent in &ordering.in_sets {
        config = config.in_set(parent.clone());
    }
    for target in &ordering.before {
        config = config.before(target.clone());
    }
    for target in &ordering.after {
        config = config.after(target.clone());
    }
    config
}

fn ordering_target(target: &Bound<'_, PyAny>) -> PyResult<DynamicSetLabel> {
    if let Some(set) = system_set_value(target)? {
        return Ok(set.label);
    }
    if let Ok(config) = target.extract::<PySystemSetConfig>() {
        return Ok(config.set.label);
    }
    if let Ok(config) = target.extract::<PySystemConfig>() {
        return callable_set(config.system.bind(target.py()));
    }
    if target.is_callable() {
        return callable_set(target);
    }
    Err(PyTypeError::new_err(
        "ordering target must be a SystemSet or a callable system",
    ))
}

pub(crate) fn system_set_value(value: &Bound<'_, PyAny>) -> PyResult<Option<PySystemSet>> {
    if let Ok(set) = value.extract::<PySystemSet>() {
        return Ok(Some(set));
    }
    let Ok(marker) = value.getattr("_pybevy_system_set") else {
        return Ok(None);
    };
    marker
        .extract::<PySystemSet>()
        .map(Some)
        .map_err(|_| PyTypeError::new_err("_pybevy_system_set must contain a SystemSet instance"))
}

fn qualified_name(value: &Bound<'_, PyAny>, role: &str) -> PyResult<String> {
    let mut module = match value
        .getattr("__module__")
        .and_then(|value| value.extract::<String>())
    {
        Ok(module) => module,
        Err(_) if role == "system callable" => "__main__".to_owned(),
        Err(_) => {
            return Err(PyTypeError::new_err(format!(
                "{role} has no string __module__"
            )));
        }
    };
    let qualname = value
        .getattr("__qualname__")
        .and_then(|value| value.extract::<String>())
        .map_err(|_| PyTypeError::new_err(format!("{role} has no string __qualname__")))?;
    if module == "<run_path>" {
        module = "__main__".to_owned();
    }
    Ok(format!("{module}.{qualname}"))
}

enum Relation {
    InSet,
    Before,
    After,
}

fn add_relation(
    source: &DynamicSetLabel,
    ordering: &mut ScheduleOrdering,
    target: DynamicSetLabel,
    relation: Relation,
) -> PyResult<()> {
    if source == &target {
        return Err(PyValueError::new_err(
            "a SystemSet cannot be ordered relative to itself",
        ));
    }
    match relation {
        Relation::InSet => ordering.push_in_set(target),
        Relation::Before => ordering.push_before(target),
        Relation::After => ordering.push_after(target),
    }
    Ok(())
}
