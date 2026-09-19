//! Structural fingerprints deciding whether a reload stays Partial or escalates.

use std::hash::{DefaultHasher, Hash, Hasher};

use pybevy_reload::DefsFingerprint;
use pyo3::{prelude::*, types::PyType};

use crate::{
    app::{PyStage, chained_systems::PyChainedSystems},
    ecs::{conditional_system::PyConditionalSystem, system_config::PySystemConfig},
};

/// Hash a code object by its fields (recursing into nested code objects).
/// Not `marshal.dumps`: its intern-dependent bytes falsely flag unrelated edits.
fn hash_code_object(code: &Bound<'_, PyAny>, hasher: &mut impl Hasher) {
    // Signature and flag fields.
    for attr in [
        "co_argcount",
        "co_posonlyargcount",
        "co_kwonlyargcount",
        "co_nlocals",
        "co_stacksize",
        "co_flags",
    ] {
        if let Ok(value) = code.getattr(attr).and_then(|v| v.extract::<i64>()) {
            value.hash(hasher);
        }
    }

    // Executable bytecode and exception-handler structure. Source locations
    // are deliberately excluded because moving unchanged code is not semantic.
    for attr in ["co_code", "co_exceptiontable"] {
        if let Ok(bytes) = code.getattr(attr).and_then(|v| v.extract::<Vec<u8>>()) {
            bytes.hash(hasher);
        }
    }

    // Name tuples (order matters).
    for attr in ["co_names", "co_varnames", "co_freevars", "co_cellvars"] {
        if let Ok(repr) = code.getattr(attr).and_then(|v| v.repr()) {
            repr.to_string().hash(hasher);
        }
    }
    if let Ok(name) = code
        .getattr("co_qualname")
        .and_then(|v| v.extract::<String>())
    {
        name.hash(hasher);
    }

    // Recurse into nested code objects; hash other consts by repr.
    if let Ok(consts) = code.getattr("co_consts")
        && let Ok(iter) = consts.try_iter()
    {
        for item in iter {
            let Ok(item) = item else { continue };
            if item.hasattr("co_code").unwrap_or(false) {
                b"\x00code\x00".hash(hasher);
                hash_code_object(&item, hasher);
            } else if let Ok(repr) = item.repr() {
                repr.to_string().hash(hasher);
            }
        }
    }
}

/// Hash a callable's qualname and code object (qualname only if no `__code__`).
fn hash_callable_code(obj: &Bound<'_, PyAny>, hasher: &mut impl Hasher) {
    if let Ok(name) = obj
        .getattr("__qualname__")
        .and_then(|n| n.extract::<String>())
    {
        name.hash(hasher);
    }
    if let Ok(code) = obj.getattr("__code__") {
        hash_code_object(&code, hasher);
    }
    // Positional and keyword-only defaults affect behavior but live outside __code__.
    for attr in ["__defaults__", "__kwdefaults__"] {
        if let Ok(repr) = obj.getattr(attr).and_then(|v| v.repr()) {
            repr.to_string().hash(hasher);
        }
    }
}

/// Hash a registered system into `hasher`, unwrapping conditional and
/// chained wrappers the same way `system_names` does.
pub(super) fn hash_system_code(
    py: Python<'_>,
    sys_bound: &Bound<'_, PyAny>,
    hasher: &mut impl Hasher,
) {
    if let Ok(config) = sys_bound.extract::<PySystemConfig>() {
        hash_callable_code(config.system.bind(py), hasher);
        for condition in &config.conditions {
            hash_callable_code(condition.bind(py), hasher);
        }
    } else if let Ok(conditional) = sys_bound.extract::<PyConditionalSystem>() {
        hash_callable_code(conditional.system.bind(py), hasher);
        conditional.condition.for_each_leaf(&mut |condition| {
            hash_callable_code(condition.bind(py), hasher);
        });
    } else if let Ok(chained) = sys_bound.extract::<PyChainedSystems>() {
        for inner in chained.systems.bind(py).iter() {
            hash_callable_code(&inner, hasher);
        }
    } else {
        hash_callable_code(sys_bound, hasher);
    }
}

/// Shared fingerprint builder for initial and reloaded Python definitions.
pub(crate) struct InitialDefsFingerprint {
    startup: DefaultHasher,
    observers: DefaultHasher,
    resource_names: Vec<String>,
    has_startup: bool,
    has_observers: bool,
}

impl Default for InitialDefsFingerprint {
    fn default() -> Self {
        Self {
            startup: DefaultHasher::new(),
            observers: DefaultHasher::new(),
            resource_names: Vec::new(),
            has_startup: false,
            has_observers: false,
        }
    }
}

impl InitialDefsFingerprint {
    /// Record the flattened systems from one successful registration call.
    pub(crate) fn record_systems(
        &mut self,
        py: Python<'_>,
        stage: PyStage,
        systems: &[Bound<'_, PyAny>],
    ) {
        if !stage.is_startup() || systems.is_empty() {
            return;
        }
        self.has_startup = true;
        format!("{stage:?}").hash(&mut self.startup);
        for system in systems {
            hash_system_code(py, system, &mut self.startup);
        }
    }

    pub(crate) fn record_resource(&mut self, resource_type: &Bound<'_, PyType>) {
        self.resource_names.push(
            resource_type
                .fully_qualified_name()
                .map(|name| name.to_string())
                .unwrap_or_else(|_| "<unknown>".to_string()),
        );
    }

    pub(crate) fn record_state(&mut self, state_type: &Bound<'_, PyType>) {
        self.resource_names.push(
            state_type
                .fully_qualified_name()
                .map(|name| format!("State[{name}]"))
                .unwrap_or_else(|_| "State[<unknown>]".to_string()),
        );
    }

    pub(crate) fn record_observer(&mut self, py: Python<'_>, observer: &Bound<'_, PyAny>) {
        self.has_observers = true;
        hash_system_code(py, observer, &mut self.observers);
    }

    /// Build the baseline the first reload's fingerprint is compared against.
    pub(crate) fn finish(&self) -> DefsFingerprint {
        let mut resource_names = self.resource_names.clone();
        resource_names.sort();
        let mut resources_hasher = DefaultHasher::new();
        resource_names.hash(&mut resources_hasher);

        DefsFingerprint {
            startup_code: self.startup.finish(),
            resource_types: resources_hasher.finish(),
            observer_code: self.observers.finish(),
            component_layout_changed: false,
            resource_layout_changed: false,
            has_startup: self.has_startup,
            has_resources: !self.resource_names.is_empty(),
            has_observers: self.has_observers,
        }
    }
}
