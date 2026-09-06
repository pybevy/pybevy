use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{
        resource::Resource,
        schedule::{
            BoxedCondition, InternedSystemSet, IntoScheduleConfigs, ScheduleConfigs,
            SystemCondition, common_conditions::not,
        },
        system::{BoxedSystem, PipeSystem, ScheduleSystem},
    },
    prelude::DebugName,
};
use pybevy_ecs::shared::{
    schedule::{ConditionExpr, DynamicSetLabel, ScheduleOrdering, SystemSetTarget},
    system_runtime::{ErasedConditionSystem, ErasedSystem},
};
use pybevy_reload::{ReloadGenerationSet, SystemStage, generation_matches, startup_or_reload};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyType,
};

use crate::{
    app::PyStage,
    ecs::{
        conditional_system::{PyConditionalSystem, extract_condition_expr},
        dynamic_system::{DynamicSystemHandle, SystemErrorBuffer},
        system_interpreter::{
            new_main_condition, new_main_persistent_condition, new_main_system,
            new_main_unit_target, new_main_value_source, new_main_value_target,
        },
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SystemSetConfigIdentity {
    set: SystemSetTarget,
    ordering: ScheduleOrdering,
    conditions: Vec<DynamicSetLabel>,
}

impl SystemSetConfigIdentity {
    pub(crate) fn set(&self) -> &SystemSetTarget {
        &self.set
    }
}

#[derive(Resource, Default)]
pub(crate) struct InstalledSystemSetConfigs {
    configs: HashMap<(PyStage, SystemSetTarget), SystemSetConfigIdentity>,
}

impl InstalledSystemSetConfigs {
    pub(crate) fn get(
        &self,
        schedule: PyStage,
        set: &SystemSetTarget,
    ) -> Option<&SystemSetConfigIdentity> {
        self.configs.get(&(schedule, set.clone()))
    }

    pub(crate) fn insert(&mut self, schedule: PyStage, identity: SystemSetConfigIdentity) {
        self.configs
            .insert((schedule, identity.set.clone()), identity);
    }
}

/// A stable, interpreter-independent Bevy system-set identity.
#[pyclass(name = "SystemSet", module = "pybevy.ecs", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub struct PySystemSet {
    pub(crate) label: SystemSetTarget,
}

impl PySystemSet {
    fn from_qualified_name(qualified_name: String) -> Self {
        Self {
            label: DynamicSetLabel::named(qualified_name).into(),
        }
    }

    pub(crate) fn from_native(qualified_name: String, label: InternedSystemSet) -> Self {
        Self {
            label: SystemSetTarget::native(label, qualified_name),
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
    pub(crate) pipe_stages: Vec<Py<PyAny>>,
    pub(crate) ordering: ScheduleOrdering,
    pub(crate) conditions: Vec<Py<PyAny>>,
}

impl Clone for PySystemConfig {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            system: self.system.clone_ref(py),
            pipe_stages: self
                .pipe_stages
                .iter()
                .map(|stage| stage.clone_ref(py))
                .collect(),
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
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.system)?;
        for stage in &self.pipe_stages {
            visit.call(stage)?;
        }
        for condition in &self.conditions {
            visit.call(condition)?;
        }
        Ok(())
    }

    #[new]
    pub fn new(system: Py<PyAny>) -> Self {
        Self {
            system,
            pipe_stages: Vec::new(),
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

    /// Return a new compound system that passes this system's output to `target`.
    pub fn pipe(&self, py: Python<'_>, target: Py<PyAny>) -> PyResult<Self> {
        if !target.bind(py).is_callable() {
            return Err(PyTypeError::new_err("pipe() target must be callable"));
        }
        let mut result = self.clone();
        result.pipe_stages.push(target.clone_ref(py));
        Ok(result)
    }
}

/// Fluent configuration for one Python system set.
#[pyclass(name = "SystemSetConfig", module = "pybevy.ecs", from_py_object)]
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
    /// Report held Python objects to the cyclic GC.
    ///
    /// A Rust-held `Py` reference is invisible to the collector, and user
    /// scene objects reach back here through their defining module's dict, so
    /// without this the cycle is uncollectable and every hot reload leaks a
    /// whole generation. Traverse stays read-only and takes no locks.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for condition in &self.conditions {
            visit.call(condition)?;
        }
        Ok(())
    }

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
#[pyo3(signature = (source, target, /))]
pub fn pipe(source: Py<PyAny>, target: Py<PyAny>, py: Python<'_>) -> PyResult<PySystemConfig> {
    if !source.bind(py).is_callable() {
        return Err(PyTypeError::new_err("pipe() source must be callable"));
    }
    PySystemConfig::new(source).pipe(py, target)
}

#[pyfunction]
#[pyo3(signature = (cls, /))]
pub fn system_set(cls: &Bound<'_, PyType>) -> PyResult<PySystemSet> {
    let qualified_name = qualified_name(cls.as_any(), "system set")?;
    Ok(PySystemSet::from_qualified_name(qualified_name))
}

pub(crate) fn register_native_system_sets(root: &Bound<'_, PyModule>) -> PyResult<()> {
    for registration in pybevy_core::inventory::iter::<pybevy_core::NativeSystemSetRegistration> {
        let module = root.getattr(registration.module)?.cast_into::<PyModule>()?;
        let qualified_name = format!("pybevy.{}.{}", registration.module, registration.name);
        module.add(
            registration.name,
            PySystemSet::from_native(qualified_name, (registration.intern)()),
        )?;
    }
    Ok(())
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
) -> PyResult<(ScheduleConfigs<ScheduleSystem>, Vec<DynamicSystemHandle>)> {
    let py = value.py();
    let (callable, pipe_stages, ordering, conditions, combined_condition) =
        if let Ok(config) = value.extract::<PySystemConfig>() {
            (
                config.system,
                config.pipe_stages,
                config.ordering,
                config.conditions,
                None,
            )
        } else if let Ok(conditional) = value.extract::<PyConditionalSystem>() {
            (
                conditional.system,
                Vec::new(),
                ScheduleOrdering::default(),
                Vec::new(),
                Some(conditional.condition),
            )
        } else {
            (
                value.clone().unbind(),
                Vec::new(),
                ScheduleOrdering::default(),
                Vec::new(),
                None,
            )
        };

    let identity = callable_set(callable.bind(py))?;
    let pipe_identities = pipe_stages
        .iter()
        .map(|stage| callable_set(stage.bind(py)))
        .collect::<PyResult<Vec<_>>>()?;
    let (schedule_system, handles) = if pipe_stages.is_empty() {
        let dynamic_system = new_main_system(
            callable,
            generation,
            error_state.clone(),
            error_buffer,
            system_stage,
        )?;
        let handle = dynamic_system.handle().clone();
        (Box::new(dynamic_system) as ScheduleSystem, vec![handle])
    } else {
        build_pipe_system(
            callable,
            pipe_stages,
            generation,
            error_state.clone(),
            error_buffer,
            system_stage,
            py,
        )?
    };
    let mut config = if is_startup {
        schedule_system.run_if(startup_or_reload(generation))
    } else {
        schedule_system.run_if(generation_matches(generation))
    }
    .in_set(ReloadGenerationSet(generation));

    for condition in conditions {
        let condition = build_condition_expr(
            extract_condition_expr(condition)?,
            generation,
            error_state.clone(),
            system_stage,
        )?;
        config.run_if_dyn(condition);
    }
    if let Some(condition) = combined_condition {
        let condition =
            build_condition_expr(condition, generation, error_state.clone(), system_stage)?;
        config.run_if_dyn(condition);
    }

    let mut config = apply_system_ordering(config, identity, &ordering);
    for pipe_identity in pipe_identities {
        // Every callable has a stable identity set, including downstream pipe
        // stages, so ordering against any stage orders the compound system.
        config = config.in_set(pipe_identity);
    }

    Ok((config, handles))
}

#[allow(clippy::too_many_arguments)]
fn build_pipe_system(
    source: Py<PyAny>,
    targets: Vec<Py<PyAny>>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    error_buffer: SystemErrorBuffer,
    system_stage: SystemStage,
    py: Python<'_>,
) -> PyResult<(ScheduleSystem, Vec<DynamicSystemHandle>)> {
    let mut names = vec![qualified_name(source.bind(py), "system callable")?];
    for target in &targets {
        names.push(qualified_name(target.bind(py), "system callable")?);
    }
    let debug_name = || DebugName::owned(names.join(" |> "));

    let source = new_main_value_source(
        source,
        generation,
        error_state.clone(),
        error_buffer.clone(),
        system_stage,
    )?;
    let mut handles = vec![source.handle().clone()];
    let mut pipeline: BoxedSystem<(), Option<Py<PyAny>>> = Box::new(source);
    let mut targets = targets.into_iter().peekable();

    while let Some(target) = targets.next() {
        if targets.peek().is_some() {
            let target = new_main_value_target(
                target,
                generation,
                error_state.clone(),
                error_buffer.clone(),
                system_stage,
            )?;
            handles.push(target.handle().clone());
            pipeline = Box::new(PipeSystem::new(
                ErasedSystem::new(pipeline),
                target,
                debug_name(),
            ));
        } else {
            let target =
                new_main_unit_target(target, generation, error_state, error_buffer, system_stage)?;
            handles.push(target.handle().clone());
            let pipeline = Box::new(PipeSystem::new(
                ErasedSystem::new(pipeline),
                target,
                debug_name(),
            ));
            return Ok((pipeline, handles));
        }
    }

    unreachable!("pipe construction requires at least one target")
}

fn build_condition_expr(
    expression: ConditionExpr<Py<PyAny>>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    system_stage: SystemStage,
) -> PyResult<BoxedCondition> {
    match expression {
        ConditionExpr::Leaf(condition) => Ok(Box::new(new_main_condition(
            condition,
            generation,
            error_state,
            system_stage,
        )?)),
        ConditionExpr::And(left, right) => {
            let left = build_condition_expr(*left, generation, error_state.clone(), system_stage)?;
            let right = build_condition_expr(*right, generation, error_state, system_stage)?;
            Ok(Box::new(
                ErasedConditionSystem::new(left).and_then(ErasedConditionSystem::new(right)),
            ))
        }
        ConditionExpr::Or(left, right) => {
            let left = build_condition_expr(*left, generation, error_state.clone(), system_stage)?;
            let right = build_condition_expr(*right, generation, error_state, system_stage)?;
            Ok(Box::new(
                ErasedConditionSystem::new(left).or_else(ErasedConditionSystem::new(right)),
            ))
        }
        ConditionExpr::Not(condition) => {
            let condition =
                build_condition_expr(*condition, generation, error_state, system_stage)?;
            Ok(Box::new(not(ErasedConditionSystem::new(condition))))
        }
    }
}

fn build_persistent_condition_expr(
    expression: ConditionExpr<Py<PyAny>>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    system_stage: SystemStage,
) -> PyResult<BoxedCondition> {
    match expression {
        ConditionExpr::Leaf(condition) => Ok(Box::new(new_main_persistent_condition(
            condition,
            generation,
            error_state,
            system_stage,
        )?)),
        ConditionExpr::And(left, right) => {
            let left = build_persistent_condition_expr(
                *left,
                generation,
                error_state.clone(),
                system_stage,
            )?;
            let right =
                build_persistent_condition_expr(*right, generation, error_state, system_stage)?;
            Ok(Box::new(
                ErasedConditionSystem::new(left).and_then(ErasedConditionSystem::new(right)),
            ))
        }
        ConditionExpr::Or(left, right) => {
            let left = build_persistent_condition_expr(
                *left,
                generation,
                error_state.clone(),
                system_stage,
            )?;
            let right =
                build_persistent_condition_expr(*right, generation, error_state, system_stage)?;
            Ok(Box::new(
                ErasedConditionSystem::new(left).or_else(ErasedConditionSystem::new(right)),
            ))
        }
        ConditionExpr::Not(condition) => {
            let condition =
                build_persistent_condition_expr(*condition, generation, error_state, system_stage)?;
            Ok(Box::new(not(ErasedConditionSystem::new(condition))))
        }
    }
}

pub(crate) fn build_set_config(
    value: &Bound<'_, PyAny>,
    generation: u32,
    error_state: Arc<Mutex<Vec<PyErr>>>,
    system_stage: SystemStage,
) -> PyResult<ScheduleConfigs<InternedSystemSet>> {
    let (set, ordering, conditions) = system_set_config_parts(value)?;

    let mut config = apply_set_ordering(set.label.intern().into_configs(), &ordering);
    for condition in conditions {
        let condition = build_persistent_condition_expr(
            extract_condition_expr(condition)?,
            generation,
            error_state.clone(),
            system_stage,
        )?;
        config.run_if_dyn(condition);
    }
    Ok(config)
}

pub(crate) fn system_set_config_identity(
    value: &Bound<'_, PyAny>,
) -> PyResult<SystemSetConfigIdentity> {
    let (set, ordering, conditions) = system_set_config_parts(value)?;
    let conditions = conditions
        .iter()
        .map(|condition| callable_set(condition.bind(value.py())))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(SystemSetConfigIdentity {
        set: set.label,
        ordering,
        conditions,
    })
}

fn system_set_config_parts(
    value: &Bound<'_, PyAny>,
) -> PyResult<(PySystemSet, ScheduleOrdering, Vec<Py<PyAny>>)> {
    if let Ok(config) = value.extract::<PySystemSetConfig>() {
        Ok((config.set, config.ordering, config.conditions))
    } else if let Some(set) = system_set_value(value)? {
        Ok((set, ScheduleOrdering::default(), Vec::new()))
    } else {
        Err(PyTypeError::new_err(
            "configure_sets() values must be SystemSet or SystemSetConfig objects",
        ))
    }
}

pub(crate) fn apply_system_ordering(
    mut config: ScheduleConfigs<ScheduleSystem>,
    callable: DynamicSetLabel,
    ordering: &ScheduleOrdering,
) -> ScheduleConfigs<ScheduleSystem> {
    config = config.in_set(callable);
    for set in &ordering.in_sets {
        config = config.in_set(set.intern());
    }
    for target in &ordering.before {
        config = config.before(target.intern());
    }
    for target in &ordering.after {
        config = config.after(target.intern());
    }
    config
}

pub(crate) fn apply_set_ordering(
    mut config: ScheduleConfigs<InternedSystemSet>,
    ordering: &ScheduleOrdering,
) -> ScheduleConfigs<InternedSystemSet> {
    for parent in &ordering.in_sets {
        config = config.in_set(parent.intern());
    }
    for target in &ordering.before {
        config = config.before(target.intern());
    }
    for target in &ordering.after {
        config = config.after(target.intern());
    }
    config
}

fn ordering_target(target: &Bound<'_, PyAny>) -> PyResult<SystemSetTarget> {
    if let Some(set) = system_set_value(target)? {
        return Ok(set.label);
    }
    if let Ok(config) = target.extract::<PySystemSetConfig>() {
        return Ok(config.set.label);
    }
    if let Ok(config) = target.extract::<PySystemConfig>() {
        return callable_set(config.system.bind(target.py())).map(Into::into);
    }
    if target.is_callable() {
        return callable_set(target).map(Into::into);
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
    source: &SystemSetTarget,
    ordering: &mut ScheduleOrdering,
    target: SystemSetTarget,
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
