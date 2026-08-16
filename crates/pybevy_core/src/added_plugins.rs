use std::collections::{HashMap, HashSet};

use bevy::ecs::resource::Resource;

/// Interpreter-neutral record of Python plugin classes added to one app.
///
/// Type keys provide the fast path. Qualified names preserve identity when hot
/// reload creates a new class object for the same `module.qualname`, and also
/// verify the fast path: a class address is only an identity while that class
/// is alive, and CPython reuses the address of a dropped plugin class straight
/// away. See "Addresses Are Not Identities" in docs/safety.md.
#[derive(Resource, Default)]
pub struct AddedPythonPlugins {
    type_keys: HashMap<usize, Option<String>>,
    qualified_names: HashSet<String>,
}

impl AddedPythonPlugins {
    fn key_matches(&self, type_key: usize, qualified_name: Option<&str>) -> bool {
        self.type_keys
            .get(&type_key)
            .is_some_and(|recorded| recorded.as_deref() == qualified_name)
    }

    pub fn contains(&self, type_key: usize, qualified_name: Option<&str>) -> bool {
        self.key_matches(type_key, qualified_name)
            || qualified_name.is_some_and(|name| self.qualified_names.contains(name))
    }

    pub fn insert(&mut self, type_key: usize, qualified_name: Option<&str>) {
        self.type_keys
            .insert(type_key, qualified_name.map(str::to_owned));
        if let Some(name) = qualified_name {
            self.qualified_names.insert(name.to_owned());
        }
    }

    /// Record a plugin and return whether it was already present.
    ///
    /// A name match also records the new type key as an alias, restoring the
    /// fast path after a class is redefined during hot reload.
    pub fn check_and_insert(&mut self, type_key: usize, qualified_name: Option<&str>) -> bool {
        if self.key_matches(type_key, qualified_name) {
            return true;
        }
        if qualified_name.is_some_and(|name| self.qualified_names.contains(name)) {
            self.type_keys
                .insert(type_key, qualified_name.map(str::to_owned));
            return true;
        }

        self.insert(type_key, qualified_name);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_key_is_the_fast_path() {
        let mut added = AddedPythonPlugins::default();

        assert!(!added.check_and_insert(1, Some("game.MyPlugin")));
        assert!(added.check_and_insert(1, Some("game.MyPlugin")));
    }

    #[test]
    fn qualified_name_matches_redefined_class_and_adds_alias() {
        let mut added = AddedPythonPlugins::default();
        added.insert(1, Some("game.MyPlugin"));

        assert!(added.check_and_insert(2, Some("game.MyPlugin")));
        assert!(added.contains(2, Some("game.MyPlugin")));
    }

    #[test]
    fn different_names_do_not_collide() {
        let mut added = AddedPythonPlugins::default();
        added.insert(1, Some("game.PluginA"));

        assert!(!added.contains(2, Some("game.PluginB")));
    }

    /// A dropped plugin class frees its address for the next class allocated.
    /// Treating the address alone as identity reported the new plugin as
    /// already added, so its `build()` never ran.
    #[test]
    fn recycled_address_with_another_name_is_not_already_added() {
        let mut added = AddedPythonPlugins::default();
        added.insert(1, Some("game.PluginA"));

        assert!(!added.contains(1, Some("game.PluginB")));
        assert!(!added.check_and_insert(1, Some("game.PluginB")));
    }
}
