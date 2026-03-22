# PyBevy MCP Server

The PyBevy MCP server allows AI agents (Claude Code, Cursor, Codex, etc.) to inspect and manipulate running Bevy scenes through the [Model Context Protocol](https://modelcontextprotocol.io/).

## Setup

### Claude Code

Add to your project's `.mcp.json`:

```json
{
  "mcpServers": {
    "pybevy": {
      "command": "pybevy",
      "args": ["mcp"]
    }
  }
}
```

Or using the CLI:

```bash
claude mcp add pybevy -- pybevy mcp
```

If `pybevy` isn't on your PATH (e.g. installed via Poetry), use the full path:

```bash
claude mcp add pybevy -- /path/to/virtualenv/bin/pybevy mcp
```

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "pybevy": {
      "command": "pybevy",
      "args": ["mcp"]
    }
  }
}
```

### Auto-load a scene on startup

Pass a script path to start with a scene already running:

```json
{
  "mcpServers": {
    "pybevy": {
      "command": "pybevy",
      "args": ["mcp", "examples/mcp/mcp_scene.py"]
    }
  }
}
```

## HTTP Transport

When you add `McpPlugin()` directly to your app and run it (not via `pybevy mcp`), an HTTP+SSE server starts automatically inside the Bevy process:

```python
@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins, McpPlugin())
```

By default it listens on `http://127.0.0.1:8420/mcp/v1`. You can customize the port and host:

```python
McpPlugin(port=9000, host="0.0.0.0")
```

This is useful for connecting non-stdio MCP clients or custom tooling directly to a running scene.

The stdio setup (described above) uses a separate Python bridge process that manages the Bevy app as a subprocess. The bridge sets `PYBEVY_MCP_PIPE=1` internally, which switches `McpPlugin` to pipe transport instead of HTTP.

## MCP Content (`pybevy/mcp/`)

The MCP server serves contextual documentation to AI agents from the `pybevy/mcp/` folder:

- **`instructions.md`** - Default instructions sent to the AI agent when the MCP server connects. Contains the core PyBevy API patterns, conventions, and usage guidance.
- **`guides/`** - Topic-specific guides that the MCP server can serve on demand (e.g., `camera.md`, `lighting.md`, `queries.md`, `performance.md`). These provide deeper coverage of individual features.

## Customizing MCP Definitions (`mcp-eject`)

To customize the tool descriptions and prompts served to AI agents:

```bash
pybevy mcp-eject              # Ejects to .pybevy/mcp/
pybevy mcp-eject -o my-mcp/   # Custom output directory
```

This creates a `pack.toml` with all tool descriptions commented out for patching, plus editable Markdown prompt files. Uncomment and modify entries in `pack.toml` to override the default tool descriptions.

## Troubleshooting

### No window appears (Linux)

The Bevy subprocess needs access to the display server (X11 or Wayland). The bridge tries to auto-detect display environment variables (`DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`) when they're missing, but if that fails, you can pass them explicitly in `.mcp.json`:

```json
{
  "mcpServers": {
    "pybevy": {
      "command": "pybevy",
      "args": ["mcp"],
      "env": {
        "DISPLAY": ":0",
        "WAYLAND_DISPLAY": "wayland-0",
        "XDG_RUNTIME_DIR": "/run/user/1000"
      }
    }
  }
}
```

Check your values with `echo $DISPLAY $WAYLAND_DISPLAY $XDG_RUNTIME_DIR`.

### Subprocess crashes on startup

If `run_scene` returns an error with "subprocess crashed", check the error message — it includes the subprocess stderr output. Common causes:
- Missing Python dependencies
- Scene file syntax errors
- GPU/driver issues (try running `pybevy watch <script>` directly to diagnose)
