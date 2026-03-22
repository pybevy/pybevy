# Cargo Features

PyBevy exposes several Cargo features to control optional functionality. Features are defined in the root `Cargo.toml` and proxied through the `pybevy-python` cdylib crate.

## Feature Summary

| Feature | Default | Description |
|---------|---------|-------------|
| `mcp` | Yes | Model Context Protocol server for AI agents |
| `linux-display` | No | X11 and Wayland windowing on Linux |
| `native-plugin` | No | Embed Python systems in native Rust Bevy apps |
| `native-hot-reload` | No | File-watcher auto-reload for native plugin mode |

## Feature Details

### `mcp`

**Default: enabled**

Enables the Model Context Protocol (MCP) server, which lets AI agents (Claude, Cursor, etc.) interact with a running PyBevy scene. The server exposes tools for querying entities, inspecting components, capturing screenshots, and more.

- Pulls in the `pybevy_mcp` crate, which bundles Tokio, Axum, and an HTTP/SSE transport
- Registers the `pybevy.mcp` Python submodule with server start/stop functions
- No runtime cost if the server is not explicitly started from Python

```toml
# Disable to reduce compile time and binary size
pybevy = { version = "0.18", default-features = false }
```

### `linux-display`

**Default: disabled**

Enables windowing support on Linux by activating Bevy's `x11` and `wayland` features. Without this, PyBevy compiles but cannot open a window on Linux.

- Required for any graphical application on Linux
- Not needed on macOS or Windows (those use platform-native windowing by default)
- The standard development build command includes it: `maturin develop --features "linux-display"`

```toml
# For Linux development
pybevy = { version = "0.18", features = ["linux-display"] }
```

### `native-plugin`

**Default: disabled**

Enables `PyBevyPlugin`, which lets you embed Python systems into a native Rust Bevy application. This is the entry point for using PyBevy as a scripting/modding layer rather than as a standalone Python game engine.

- Compiles `src/plugin.rs` and exports `PyBevyPlugin` from the crate root
- Generates a `_pybevy` PyO3 module inside the rlib (for `append_to_inittab!` embedding)
- No additional dependencies — this is a pure compile-gate feature

```toml
[dependencies]
pybevy = { version = "0.18", features = ["native-plugin"] }
```

See [Native Plugin Usage](native-plugin.md) for the full guide.

### `native-hot-reload`

**Default: disabled**

Adds automatic file-watching to the native plugin's hot reload system. When a `.py` file changes in a watched directory, a reload is triggered without requiring a manual F5 keypress.

- Implies `native-plugin` (automatically enables it)
- Pulls in the `notify` crate for cross-platform filesystem event watching
- Watches directories registered via `PyBevyPlugin::with_python_path()`

```toml
[dependencies]
pybevy = { version = "0.18", features = ["native-hot-reload"] }
```

## Feature Propagation

The `pybevy-python` cdylib crate (which produces the `_pybevy.so`/`.pyd` Python extension module) proxies features to the root `pybevy` crate:

```
pybevy-python feature    →    pybevy feature
─────────────────────         ──────────────
linux-display            →    linux-display   →  bevy/x11 + bevy/wayland
mcp                      →    mcp             →  dep:pybevy_mcp
```

The `native-plugin` and `native-hot-reload` features are only relevant when using PyBevy as a Rust library dependency, so they are not proxied through `pybevy-python`.

## Common Configurations

```toml
# Python-first development on Linux (default + linux windowing)
maturin develop --features "linux-display"

# Minimal build (CI, no GPU overlay, no MCP)
maturin develop --features "linux-display" --no-default-features

# Native Rust app with Python scripting + auto-reload
[dependencies]
pybevy = { version = "0.18", features = ["native-hot-reload"] }

# Native Rust app, manual reload only (no file watcher)
[dependencies]
pybevy = { version = "0.18", features = ["native-plugin"] }
```
