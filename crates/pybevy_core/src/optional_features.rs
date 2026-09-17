//! Interpreter-neutral inventory of optional public PyBevy capabilities.
//!
//! A feature crate submits one registration only when its public capability is
//! compiled. Interpreter adapters materialize their native set type from this
//! shared list, so Python shims never infer build state from module attributes.

/// One optional public capability compiled into the current binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionalFeatureRegistration {
    name: &'static str,
}

impl OptionalFeatureRegistration {
    /// Construct a compile-time registration.
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    /// Cargo-compatible public name for the capability.
    pub const fn name(self) -> &'static str {
        self.name
    }
}

inventory::collect!(OptionalFeatureRegistration);

/// Return whether one optional public feature is compiled into this binary.
pub fn is_optional_feature_compiled(name: &str) -> bool {
    inventory::iter::<OptionalFeatureRegistration>
        .into_iter()
        .any(|registration| registration.name() == name)
}

/// Return the deterministic, duplicate-free set of compiled public features.
pub fn compiled_optional_features() -> Vec<&'static str> {
    let mut names = inventory::iter::<OptionalFeatureRegistration>
        .into_iter()
        .map(|registration| registration.name())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names
}

/// Register an optional public capability from a feature-gated crate or module.
#[macro_export]
macro_rules! register_optional_feature {
    ($name:literal) => {
        $crate::inventory::submit!($crate::optional_features::OptionalFeatureRegistration::new(
            $name
        ));
    };
}

#[cfg(test)]
mod tests {
    use super::{
        OptionalFeatureRegistration, compiled_optional_features, is_optional_feature_compiled,
    };

    inventory::submit!(OptionalFeatureRegistration::new("z-test-feature"));
    inventory::submit!(OptionalFeatureRegistration::new("a-test-feature"));
    inventory::submit!(OptionalFeatureRegistration::new("a-test-feature"));

    #[test]
    fn inventory_is_sorted_and_deduplicated() {
        let features = compiled_optional_features();
        assert_eq!(features, ["a-test-feature", "z-test-feature"]);
    }

    #[test]
    fn feature_lookup_uses_the_same_inventory() {
        assert!(is_optional_feature_compiled("a-test-feature"));
        assert!(!is_optional_feature_compiled("missing-test-feature"));
    }
}
