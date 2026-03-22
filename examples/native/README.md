# Native Plugin Examples

For existing Rust Bevy projects that want to add Python scripting — modding, rapid iteration, or using the Python ecosystem alongside Rust.

> **Security Warning:** `PyBevyPlugin` embeds a full CPython interpreter with unrestricted access to the host system. Never execute untrusted Python code. We advise against loading arbitrary user-provided scripts (e.g., for a modding/plugin system) without external sandboxing — Python has no built-in sandboxing and any loaded code runs with your application's full privileges.

## Files

- `native_plugin_example.rs` — Rust app that defines a `Health` component with `#[derive(PyComponent)]`, spawns entities, and reads values mutated by Python
- `example_systems.py` — Python systems that set up a 3D scene, rotate a cube, and apply damage to `Health` each frame

## Running

```bash
cargo run --example native_plugin_example --release
```

## Hot Reload

Hot reload is enabled in this example. While the app is running:

- **F5** — Full reload (re-runs Startup systems, re-creates the scene)
- **F6** — Toggle default reload mode (Full / Partial)
- **File watcher** — Automatically reloads when `.py` files change (requires `native-hot-reload` feature)

**Try it:** Open `example_systems.py`, change `ROTATION_SPEED = 1.0` to `5.0`, save, and press F5. The cube will spin faster.

## What it demonstrates

1. `Health` component defined in Rust with `#[derive(PyComponent)]`
2. Rust spawns entities with `Health(100, 100)`
3. Python queries `Health`, subtracts damage each frame
4. Rust reads the updated values and prints them every 60 frames
5. Hot reload: edit Python code and see changes without restarting

See `docs/native-plugin.md` for full documentation.
