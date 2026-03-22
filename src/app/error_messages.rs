/// Centralized error messages for plugin validation
///
/// This module contains all user-facing error messages for plugin-related errors,
/// making them easy to maintain and update in one place.
/// Error message when a non-Plugin type is passed to add_plugins()
///
/// # Arguments
/// * `plugin_name` - The name of the invalid class
/// * `mro` - Method Resolution Order (inheritance chain)
pub fn plugin_not_a_plugin_error(plugin_name: &str, mro: &str) -> String {
    format!(
        "Expected a Plugin instance or type, but got '{plugin_name}'\n\
         \n\
         Inheritance chain: {mro}\n\
         \n\
         Possible causes:\n\
         • The class does not inherit from Plugin (ensure 'class {plugin_name}(Plugin):')\n\
         • Missing '@plugin' decorator (add @plugin above the class)\n\
         • The Plugin class was not imported correctly (check 'from pybevy.app import Plugin')\n\
         \n\
         Example:\n\
         from pybevy.app import Plugin\n\
         from pybevy.decorators import plugin\n\
         \n\
         @plugin\n\
         class MyPlugin(Plugin):\n\
             def build(self, app):\n\
                 pass"
    )
}

/// Error message when a Plugin class is missing the @plugin decorator
///
/// # Arguments
/// * `plugin_name` - The name of the plugin class missing the decorator
pub fn plugin_missing_decorator_error(plugin_name: &str) -> String {
    format!(
        "Plugin class '{plugin_name}' must be decorated with @plugin decorator\n\
         \n\
         Add the @plugin decorator above your plugin class:\n\
         \n\
         from pybevy.app import Plugin\n\
         from pybevy.decorators import plugin\n\
         \n\
         @plugin  # <- Add this!\n\
         class {plugin_name}(Plugin):\n\
             def build(self, app):\n\
                 pass"
    )
}
