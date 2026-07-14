use std::collections::HashSet;

use bevy::ecs::resource::Resource;

/// Interpreter-neutral record of Python plugin classes added to one app.
///
/// Type keys provide the fast path. Qualified names preserve identity when hot
/// reload creates a new class object for the same `module.qualname`.
#[derive(Resource, Default)]
pub struct AddedPythonPlugins {
    type_keys: HashSet<usize>,
    qualified_names: HashSet<String>,
}

impl AddedPythonPlugins {
    pub fn contains(&self, type_key: usize, qualified_name: Option<&str>) -> bool {
        self.type_keys.contains(&type_key)
            || qualified_name.is_some_and(|name| self.qualified_names.contains(name))
    }

    pub fn insert(&mut self, type_key: usize, qualified_name: Option<&str>) {
        self.type_keys.insert(type_key);
        if let Some(name) = qualified_name {
            self.qualified_names.insert(name.to_owned());
        }
    }

    /// Record a plugin and return whether it was already present.
    ///
    /// A name match also records the new type key as an alias, restoring the
    /// fast path after a class is redefined during hot reload.
    pub fn check_and_insert(&mut self, type_key: usize, qualified_name: Option<&str>) -> bool {
        if self.type_keys.contains(&type_key) {
            return true;
        }
        if qualified_name.is_some_and(|name| self.qualified_names.contains(name)) {
            self.type_keys.insert(type_key);
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
        assert!(added.contains(2, None));
    }

    #[test]
    fn different_names_do_not_collide() {
        let mut added = AddedPythonPlugins::default();
        added.insert(1, Some("game.PluginA"));

        assert!(!added.contains(2, Some("game.PluginB")));
    }
}
