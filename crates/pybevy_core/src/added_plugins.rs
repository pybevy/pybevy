use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
};

use bevy::ecs::resource::Resource;

/// Stable identity for one Python plugin instance across interpreter reloads.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PluginIdentity {
    qualified_name: String,
    instance_key: Option<String>,
}

impl PluginIdentity {
    pub fn new(qualified_name: impl Into<String>, instance_key: Option<String>) -> Self {
        Self {
            qualified_name: qualified_name.into(),
            instance_key,
        }
    }

    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }

    pub fn instance_key(&self) -> Option<&str> {
        self.instance_key.as_deref()
    }

    /// Backwards-compatible short name used by reload status diagnostics.
    pub fn report_name(&self) -> String {
        let name = self
            .qualified_name
            .rsplit('.')
            .next()
            .unwrap_or(&self.qualified_name);
        match &self.instance_key {
            Some(key) => format!("{name}[{key:?}]"),
            None => name.to_string(),
        }
    }
}

impl Display for PluginIdentity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match &self.instance_key {
            Some(key) => write!(formatter, "{}[{key:?}]", self.qualified_name),
            None => formatter.write_str(&self.qualified_name),
        }
    }
}

/// Interpreter-neutral record of Python plugin instances added to one app.
///
/// Type keys provide the fast path. Stable `(module.qualname, instance_key)`
/// identities preserve identity when hot reload creates a new class object,
/// while still allowing one class to install multiple named instances. The
/// qualified identity also verifies the pointer fast path because CPython can
/// immediately reuse the address of a dropped class. See "Addresses Are Not
/// Identities" in docs/safety.md.
#[derive(Resource, Default)]
pub struct AddedPythonPlugins {
    type_keys: HashMap<usize, HashSet<PluginIdentity>>,
    identities: HashSet<PluginIdentity>,
}

impl AddedPythonPlugins {
    fn key_matches(&self, type_key: usize, identity: &PluginIdentity) -> bool {
        self.type_keys
            .get(&type_key)
            .is_some_and(|recorded| recorded.contains(identity))
    }

    pub fn contains(&self, type_key: usize, identity: &PluginIdentity) -> bool {
        self.key_matches(type_key, identity) || self.identities.contains(identity)
    }

    pub fn contains_class(&self, type_key: usize, qualified_name: &str) -> bool {
        self.type_keys.get(&type_key).is_some_and(|identities| {
            identities
                .iter()
                .any(|identity| identity.qualified_name() == qualified_name)
        }) || self
            .identities
            .iter()
            .any(|identity| identity.qualified_name() == qualified_name)
    }

    pub fn insert(&mut self, type_key: usize, identity: PluginIdentity) {
        self.type_keys
            .entry(type_key)
            .or_default()
            .insert(identity.clone());
        self.identities.insert(identity);
    }

    /// Record a plugin and return whether its exact identity was present.
    ///
    /// A stable-identity match also records the new type key as an alias,
    /// restoring the fast path after a class is redefined during hot reload.
    pub fn check_and_insert(&mut self, type_key: usize, identity: PluginIdentity) -> bool {
        if self.key_matches(type_key, &identity) {
            return true;
        }
        if self.identities.contains(&identity) {
            self.type_keys.entry(type_key).or_default().insert(identity);
            return true;
        }

        self.insert(type_key, identity);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(name: &str, key: Option<&str>) -> PluginIdentity {
        PluginIdentity::new(name, key.map(str::to_owned))
    }

    #[test]
    fn type_key_is_the_fast_path() {
        let mut added = AddedPythonPlugins::default();

        assert!(!added.check_and_insert(1, identity("game.MyPlugin", None)));
        assert!(added.check_and_insert(1, identity("game.MyPlugin", None)));
    }

    #[test]
    fn qualified_identity_matches_redefined_class_and_adds_alias() {
        let mut added = AddedPythonPlugins::default();
        added.insert(1, identity("game.MyPlugin", Some("main")));

        let plugin = identity("game.MyPlugin", Some("main"));
        assert!(added.check_and_insert(2, plugin.clone()));
        assert!(added.contains(2, &plugin));
    }

    #[test]
    fn one_class_can_have_multiple_instance_keys() {
        let mut added = AddedPythonPlugins::default();

        assert!(!added.check_and_insert(1, identity("game.MyPlugin", Some("left"))));
        assert!(!added.check_and_insert(1, identity("game.MyPlugin", Some("right"))));
        assert_eq!(added.type_keys[&1].len(), 2);
    }

    #[test]
    fn different_names_do_not_collide() {
        let mut added = AddedPythonPlugins::default();
        added.insert(1, identity("game.PluginA", None));

        assert!(!added.contains(2, &identity("game.PluginB", None)));
    }

    /// A dropped plugin class frees its address for the next class allocated.
    /// Treating the address alone as identity reported the new plugin as
    /// already added, so its `build()` never ran.
    #[test]
    fn recycled_address_with_another_name_is_not_already_added() {
        let mut added = AddedPythonPlugins::default();
        added.insert(1, identity("game.PluginA", None));

        let replacement = identity("game.PluginB", None);
        assert!(!added.contains(1, &replacement));
        assert!(!added.check_and_insert(1, replacement));
    }
}
