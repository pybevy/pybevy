# Python Plugins in Rust Applications

A Rust host compiled with PyBevy's `native-plugin` feature can load a Python
plugin using `PyBevyPlugin::new("dialog").with_plugin("DialogPlugin")`.
Use `with_python_path("scripts")` when the module is in a separate directory.
It adds an import-search directory, not a `.py` file: `scripts/dialog.py` maps
to `new("dialog").with_python_path("scripts")`. Package modules use dotted names.

The module exports a `@plugin` class inheriting `pybevy.app.Plugin`, or an
instance of that class. Classes are constructed without arguments. Its
`build(self, app: App) -> None` can insert custom Python resources, register
messages, add nested plugins and register systems on the Rust host's actual
World. Resource values and message types need no corresponding Rust definition.
The host supplies the native Bevy plugins required by those systems.

Repeat `with_plugin` for ordered configuration. Normal `App.add_plugins`
deduplication applies. Explicitly selected Rust-builder systems can coexist.
There is no implicit discovery: select plugins, select systems, or use
`with_auto_discovery()` to opt into discovering `startup`, `update` and `last`
(missing functions are skipped). An empty selection panics before configuration,
including when a Python path or hot reload is configured.

The configuration App expires when plugin loading finishes. Do not retain it
for future callbacks. Its lifecycle and immediate App execution methods are
unavailable during configuration; use scheduled systems for runtime work.
`app.world(callback)` remains available for immediate World configuration.

Import, constructor and build failures print the Python exception and panic at
native plugin installation, restoring ownership of the host App while retaining
any already completed mutations. Plugin-system failures are reported through
stderr and `LastSystemError` and release pending exception references each frame.

Combine `with_plugin()` with `with_hot_reload()` for Python plugin reloads.
Each reload re-imports the selected classes or instances and collects their
builds, including nested plugins and explicit function selections. Partial
reload preserves resources and skips Startup; full reload reconstructs custom
resources and reruns Startup. Definition changes can promote partial requests
to full reload under the normal reload rules.

Keep reloadable builds declarative. `app.world(callback)` is unavailable during
reload collection, so put World mutations in Startup systems. Native Bevy
plugins are not rebuilt. Python `build` methods run even during partial reload;
external side effects are not rolled back. Import, construction or build errors
leave the previous generation and module registrations available for recovery.
F5/F6 and optional `native-hot-reload` file watching remain supported.
