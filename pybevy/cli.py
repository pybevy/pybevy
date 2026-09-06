import argparse
import ctypes
import gc
import os
import signal
import sys
import threading
import traceback
from collections.abc import Sequence
from typing import TYPE_CHECKING, Protocol

from .util.hot_reload import (
    resolve_changed_files,
    setup_component_caching,
    watch_for_changes,
)

if TYPE_CHECKING:
    from .app import App
else:
    try:
        from .app import App
    except ImportError:
        App = object


class _CreateAppFunction(Protocol):
    __module__: str

    def __call__(self, app: App | None = None) -> App: ...


class CliUsageError(ValueError):
    """A stable usage error raised by command implementations."""


def _echo(message: object = "", *, err: bool = False) -> None:
    """Write user-facing CLI output without a third-party console dependency."""
    print(message, file=sys.stderr if err else sys.stdout, flush=True)


def _existing_path(value: str) -> str:
    if not os.path.exists(value):
        raise argparse.ArgumentTypeError(f"path does not exist: {value}")
    return value


def _add_reload_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--full",
        action="store_true",
        help="Use full reload mode (despawn entities, reload all systems including Startup)",
    )
    parser.add_argument(
        "--pause",
        action="store_true",
        help="Start paused: open the renderer but don't load the scene until Space is pressed.",
    )
    parser.add_argument(
        "--resolution",
        help="Window resolution as WIDTHxHEIGHT (e.g. 1920x1080).",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose debug output for hot reload operations.",
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="pybevy",
        description="A command-line interface for pybevy.",
        allow_abbrev=False,
    )
    commands = parser.add_subparsers(dest="command", required=True)

    run_parser = commands.add_parser(
        "run",
        help="Run a pybevy script in-process.",
        description="Run a pybevy script in-process.",
        allow_abbrev=False,
    )
    run_parser.add_argument("script_path", metavar="SCRIPT_PATH", type=_existing_path)
    run_parser.add_argument(
        "--hot-reload",
        action="store_true",
        help="Enable hot reloading by restarting the app on file changes.",
    )
    run_parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose debug output for hot reload operations.",
    )
    run_parser.set_defaults(handler=run, command_parser=run_parser)

    dev_parser = commands.add_parser(
        "dev",
        help="Run a pybevy script in development mode with hot reload.",
        description="Run a pybevy script in development mode with hot reload.",
        allow_abbrev=False,
    )
    dev_parser.add_argument("script_path", metavar="SCRIPT_PATH", type=_existing_path)
    _add_reload_options(dev_parser)
    dev_parser.set_defaults(handler=dev, command_parser=dev_parser)

    watch_parser = commands.add_parser(
        "watch",
        help="Watch and run a pybevy script with hot reload (alias for dev).",
        description="Watch and run a pybevy script with hot reload (alias for dev).",
        allow_abbrev=False,
    )
    watch_parser.add_argument("script_path", metavar="SCRIPT_PATH", type=_existing_path)
    _add_reload_options(watch_parser)
    watch_parser.set_defaults(handler=watch_command, command_parser=watch_parser)

    eject_parser = commands.add_parser(
        "mcp-eject",
        help="Eject MCP definitions to editable files.",
        description="Eject MCP definitions to editable files for customization.",
        allow_abbrev=False,
    )
    eject_parser.add_argument(
        "--output",
        "-o",
        default=".pybevy/mcp",
        help="Output directory for ejected definitions",
    )
    eject_parser.set_defaults(handler=mcp_eject, command_parser=eject_parser)

    mcp_parser = commands.add_parser(
        "mcp",
        help="Start MCP bridge for AI agent integration.",
        description="Start MCP bridge for AI agent integration.",
        allow_abbrev=False,
    )
    mcp_parser.add_argument(
        "script_path",
        metavar="SCRIPT_PATH",
        nargs="?",
        type=_existing_path,
    )
    mcp_parser.add_argument(
        "--transport",
        choices=("stdio", "http"),
        default="stdio",
        help="Transport mode: stdio (default) or http (Streamable HTTP).",
    )
    mcp_parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Host for HTTP transport (default 127.0.0.1).",
    )
    mcp_parser.add_argument(
        "--port",
        default=8080,
        type=int,
        help="Port for HTTP transport (default 8080).",
    )
    mcp_parser.add_argument(
        "--record",
        action="store_true",
        help="Record MCP calls to .pybevy/logs/ as JSONL + screenshots.",
    )
    mcp_parser.set_defaults(handler=mcp_command, command_parser=mcp_parser)

    return parser


def main(argv: Sequence[str] | None = None) -> None:
    """Run the PyBevy command-line interface."""
    parser = _build_parser()
    namespace = parser.parse_args(argv)
    values = vars(namespace)
    handler = values.pop("handler")
    command_parser = values.pop("command_parser")
    values.pop("command")
    try:
        handler(**values)
    except CliUsageError as error:
        command_parser.error(str(error))


def run(script_path: str, hot_reload: bool, verbose: bool) -> None:
    """
    Run a pybevy script in-process.

    SCRIPT_PATH: The path to the Python script to run.
    """
    _run_script(script_path, hot_reload, verbose=verbose)


def dev(
    script_path: str, full: bool, pause: bool, resolution: str | None, verbose: bool
) -> None:
    """
    Run a pybevy script in development mode with hot-reload enabled.

    This command automatically enables hot-reloading, which watches for file changes
    and reloads your app without restarting the entire process. By default, uses
    fast partial reload mode that only updates Update/Last systems.

    SCRIPT_PATH: The path to the Python script to run.
    """
    partial_reload = not full  # Default to partial reload
    if partial_reload:
        _echo("Running in development mode with fast PARTIAL hot-reload (default)...")
    else:
        _echo("Running in development mode with FULL hot-reload...")
    _run_script(
        script_path,
        hot_reload=True,
        partial_reload=partial_reload,
        pause=pause,
        resolution=resolution,
        verbose=verbose,
    )


def watch_command(
    script_path: str, full: bool, pause: bool, resolution: str | None, verbose: bool
) -> None:
    """
    Watch and run a pybevy script with hot-reload (alias for dev command).

    This command is an alias for 'dev' and automatically enables hot-reloading.
    By default, uses fast partial reload mode that only updates Update/Last systems.

    SCRIPT_PATH: The path to the Python script to run.
    """
    partial_reload = not full  # Default to partial reload
    if partial_reload:
        _echo("Running in watch mode with fast PARTIAL hot-reload (default)...")
    else:
        _echo("Running in watch mode with FULL hot-reload...")
    _run_script(
        script_path,
        hot_reload=True,
        partial_reload=partial_reload,
        pause=pause,
        resolution=resolution,
        verbose=verbose,
    )


def mcp_eject(output: str) -> None:
    """Eject MCP definitions to editable files for customization.

    Creates a pack.toml with all tool descriptions commented out for patching,
    and prompts as editable Markdown files. Edit pack.toml to steer LLM behavior
    by uncommenting and modifying tool description overrides.
    """
    from pathlib import Path

    from .mcp import ApiIndex
    from .mcp.definitions import builtin_prompts, builtin_tools

    output_dir = Path(output)
    if output_dir.exists():
        _echo(
            f"Error: Output directory '{output}' already exists. Remove it first or use --output."
        )
        raise SystemExit(1)

    # Get built-in definitions from Python definitions + Rust ApiIndex
    tools = builtin_tools()
    api_index = ApiIndex()
    instructions = api_index.get_instructions() or ""
    prompts_list = builtin_prompts(instructions)

    # Create output directory
    output_dir.mkdir(parents=True, exist_ok=True)

    # Build [overrides.tools] section with all tools commented out showing current descriptions
    override_lines = [
        "# Patch tool descriptions to steer LLM behavior.",
        "# Text is appended as [PACK INSTRUCTIONS] to the built-in description.",
        "# Uncomment and edit to customize.",
        "[overrides.tools]",
    ]
    for tool in tools:
        name = tool["name"]
        desc = tool.get("description", "").replace('"', '\\"')
        override_lines.append(f'# {name} = "{desc}"')

    pack_toml = (
        '[pack]\nname = "my-mcp-pack"\nversion = "0.1.0"\n'
        'description = "Custom MCP capability pack"\n\n'
        '[compatibility]\n# pybevy = ">=0.1.0"\n\n'
        "[tools]\nexclude = []\n\n" + "\n".join(override_lines) + "\n"
    )
    (output_dir / "pack.toml").write_text(pack_toml)

    # Write instructions.md (auto-injected into LLM context on MCP connect)
    pack_customization_note = f"""

## MCP Pack Customization

This project has an editable MCP pack at `{output}/`.

- `pack.toml`: uncomment `[overrides.tools]` lines to patch tool descriptions
- `instructions.md`: edit these instructions (auto-injected into LLM context on connect)
- `prompts.md`: on-demand prompts (loaded via prompts/get)
- `guides/*.md`: add or override guides (`*.default.md` are read-only reference copies)

When a tool description is misleading or missing context for this project,
suggest editing `{output}/pack.toml` to the user.
"""
    (output_dir / "instructions.md").write_text(instructions + pack_customization_note)

    # Write prompts as a single Markdown file (on-demand, loaded via prompts/get)
    if prompts_list:
        parts = []
        for prompt in prompts_list:
            name = prompt["name"]
            description = prompt.get("description", "")
            content = prompt.get("content", "")
            parts.append(
                f"---\nname: {name}\ndescription: {description}\n---\n{content}"
            )
        (output_dir / "prompts.md").write_text("\n".join(parts) + "\n")

    # Copy built-in guides as *.default.md (reference only, not loaded)
    guide_count = 0
    try:
        import pybevy as _pybevy_pkg

        pybevy_pkg_dir = Path(_pybevy_pkg.__path__[0])
        guides_src = pybevy_pkg_dir / "mcp" / "guides"
        if guides_src.is_dir():
            guides_dir = output_dir / "guides"
            guides_dir.mkdir(exist_ok=True)
            for md_file in sorted(guides_src.rglob("*.md")):
                rel = md_file.relative_to(guides_src)
                # Preserve subdirectory structure: recipes/indoor.md -> recipes/indoor.default.md
                dest = guides_dir / rel.parent / f"{rel.stem}.default.md"
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_text(md_file.read_text())
                guide_count += 1
    except Exception as e:
        _echo(f"  (could not copy guides: {e})")

    _echo(f"Ejected MCP definitions to '{output}/':")
    _echo("  instructions.md (auto-injected into LLM context on connect)")
    _echo(f"  pack.toml with {len(tools)} tool overrides (commented out)")
    if prompts_list:
        _echo(f"  prompts.md with {len(prompts_list)} prompt(s)")
    if guide_count:
        _echo(f"  {guide_count} guide(s) as *.default.md reference files")
    _echo("")
    _echo("To customize:")
    _echo("  - Edit instructions.md to change what the LLM sees on connect")
    _echo("  - Edit pack.toml to patch tool descriptions")
    _echo("  - Copy guides/foo.default.md -> guides/foo.md to override a guide")
    _echo("  - Add new guides/foo.md or guides/subdir/foo.md for custom guides")


def mcp_command(
    script_path: str | None, transport: str, host: str, port: int, record: bool
) -> None:
    """Start MCP bridge for AI agent integration.

    Launches an MCP server that AI agents (Codex, Claude Code, Cursor, etc.)
    can connect to. The bridge handles API discovery locally and manages a Bevy
    subprocess for scene interaction.

    Optionally provide SCRIPT_PATH to auto-load a scene on startup (stdio mode only).
    """
    if transport == "http":
        if script_path:
            _echo(
                "Warning: scene_path is ignored with HTTP transport; "
                "each session creates its own bridge.",
                err=True,
            )
        from .mcp.http_transport import run_mcp_http

        run_mcp_http(host=host, port=port, record=record)
    else:
        from .mcp.bridge import McpBridge

        bridge = McpBridge(scene_path=script_path, record=record)
        bridge.run()


def _bind_lifetime_to_parent() -> None:
    """Die with the parent process when running under the test suite.

    pytest-timeout's thread method ends the runner with os._exit, which skips
    every finally block: a spawned scene would outlive the run and spin at
    full CPU. PR_SET_PDEATHSIG delivers SIGTERM to this process when its
    parent dies, whatever the cause.
    """
    if os.environ.get("PYBEVY_TESTING") != "1" or sys.platform != "linux":
        return
    pr_set_pdeathsig = 1
    try:
        libc = ctypes.CDLL(None, use_errno=True)
        libc.prctl(pr_set_pdeathsig, signal.SIGTERM, 0, 0, 0)
    except (OSError, AttributeError):
        return
    # The parent may have died before prctl took effect; reparenting to init
    # (or a subreaper) means the signal will never come.
    if os.getppid() == 1:
        os.kill(os.getpid(), signal.SIGTERM)


def _run_script(
    script_path: str,
    hot_reload: bool,
    partial_reload: bool = True,
    pause: bool = False,
    resolution: str | None = None,
    verbose: bool = False,
) -> None:
    """
    Internal function to run a pybevy script with optional hot-reload.

    Args:
        script_path: Path to the Python script to run.
        hot_reload: Whether to enable hot reloading.
        partial_reload: If True, use partial reload mode (Update/Last systems only). Defaults to True for faster reloads.
        pause: If True, start paused (the renderer opens but the scene is not
            loaded until Space is pressed).
        resolution: Window resolution as "WIDTHxHEIGHT" (e.g. "1920x1080").
        verbose: If True, enable verbose debug output for hot reload operations.
    """
    _bind_lifetime_to_parent()

    # Resolve while os.getcwd() is still the real one: loading the app can
    # change it, and a later abspath() would then rebuild a relative script
    # path against the wrong root, where the reload registers the scene module
    # under a foreign path and silently serves stale first-generation classes.
    script_path = os.path.abspath(script_path)

    # Set environment variable for Rust code to check
    if verbose:
        os.environ["PYBEVY_VERBOSE"] = "1"
    else:
        os.environ.pop("PYBEVY_VERBOSE", None)

    # Set pause flag for Rust code
    if pause:
        os.environ["PYBEVY_START_PAUSED"] = "1"
    else:
        os.environ.pop("PYBEVY_START_PAUSED", None)

    # Lets AssetPlugin turn on bevy's asset watcher, so shader and texture edits
    # land like Python edits do.
    if hot_reload:
        os.environ["PYBEVY_HOT_RELOAD"] = "1"
    else:
        os.environ.pop("PYBEVY_HOT_RELOAD", None)

    # Set window resolution for Rust code
    if resolution:
        parts = resolution.lower().split("x")
        if len(parts) != 2:
            raise CliUsageError(
                f"Invalid resolution format: {resolution!r}. Use WIDTHxHEIGHT (e.g. 1920x1080)."
            )
        try:
            int(parts[0])
            int(parts[1])
        except ValueError as err:
            raise CliUsageError(
                f"Invalid resolution format: {resolution!r}. Use WIDTHxHEIGHT (e.g. 1920x1080)."
            ) from err
        os.environ["PYBEVY_WINDOW_RESOLUTION"] = resolution
    else:
        os.environ.pop("PYBEVY_WINDOW_RESOLUTION", None)

    # Strip CLI arguments so user scripts see only their own script path in sys.argv.
    # Without this, scripts using argparse would fail on unrecognized args like "watch".
    sys.argv = [script_path]

    def load_create_app_function(
        script_path: str, changed_files: set | None = None
    ) -> _CreateAppFunction:
        """Load just the create_app function without executing it

        Args:
            script_path: Path to the main script
            changed_files: Set of absolute paths that changed (for selective reload)
        """
        try:
            # Try to convert file path to module name for relative imports
            abs_path = os.path.abspath(script_path)
            cwd = (
                os.getcwd()
            )  # Check if script is in a package (has __init__.py in parent dir)
            parent_dir = os.path.dirname(abs_path)
            parent_name = os.path.basename(parent_dir)

            module_globals: dict[str, object]
            if os.path.exists(os.path.join(parent_dir, "__init__.py")):
                # Add project root to sys.path if not already there
                if cwd not in sys.path:
                    sys.path.insert(0, cwd)

                # Convert path to module name: examples/hot_reload.py -> examples.hot_reload
                module_name = (
                    parent_name + "." + os.path.splitext(os.path.basename(abs_path))[0]
                )

                # Clear module cache based on what changed
                # Use 'is not None' check because changed_files can be an empty set
                if changed_files is not None:
                    # Selective reload (partial mode): enable component caching
                    # This keeps component class identity stable across reloads
                    setup_component_caching(enabled=True, verbose=verbose)
                    if verbose:
                        _echo("   → Component caching ENABLED (partial reload)")
                else:
                    # Full reload: disable component caching and clear cache
                    # This allows component structure changes to take effect
                    setup_component_caching(enabled=False, verbose=verbose)
                    if verbose:
                        _echo("   → Component caching disabled (full reload)")

                # Flush user modules (selective via import graph when available)
                from .util.hot_reload import flush_user_modules

                flushed = flush_user_modules(
                    parent_dir,
                    verbose=verbose,
                    graph=import_graph_holder.get("graph"),
                    changed_files=changed_files,
                )
                if verbose:
                    _echo(f"   → Flushed {len(flushed)} user modules from sys.modules")

                # Execute a fresh copy and register it in sys.modules so that
                # `import <module_name>` (run_code, numba cache loading,
                # pickling) resolves to the live scene classes. __name__ is the
                # module name, so `if __name__ == "__main__"` blocks don't run.
                from .util.hot_reload import exec_scene_module

                module_globals = exec_scene_module(module_name, verbose=verbose)
            else:
                # Standalone script without package - use run_path
                # Handle caching BEFORE reloading module
                from .decorators import (  # type: ignore[attr-defined]
                    _component_cache_enabled,  # pyright: ignore[reportAttributeAccessIssue]
                )

                if changed_files is not None:
                    # Selective reload (partial mode): ensure component caching is enabled
                    if not _component_cache_enabled:
                        setup_component_caching(enabled=True, verbose=verbose)
                    if verbose:
                        _echo(
                            "   → Component caching ENABLED (partial reload, standalone)"
                        )
                else:
                    # Full reload (F5): clear component cache but keep caching enabled if it was on
                    # This invalidates stale components while maintaining workflow preference
                    import pybevy

                    pybevy.clear_component_cache(verbose=verbose)
                    if verbose:
                        if _component_cache_enabled:
                            _echo(
                                "   → Component cache CLEARED (full reload, caching still enabled)"
                            )
                        else:
                            _echo(
                                "   → Component caching DISABLED (full reload, standalone)"
                            )

                # Flush user modules (selective via import graph when available)
                from .util.hot_reload import flush_user_modules

                flush_user_modules(
                    parent_dir,
                    verbose=verbose,
                    graph=import_graph_holder.get("graph"),
                    changed_files=changed_files,
                )

                # Match `python script.py` semantics: ensure the script's
                # directory is on sys.path so sibling imports resolve.
                if parent_dir not in sys.path:
                    sys.path.insert(0, parent_dir)

                # Execute under the file-stem module name and register in
                # sys.modules (runpy.run_path named the module "<run_path>"
                # and left it unregistered, so `import <scene>` from run_code
                # duplicated every class and numba's cache pickled an
                # unimportable module name).
                from .util.hot_reload import exec_scene_module

                module_name = os.path.splitext(os.path.basename(abs_path))[0]
                module_globals = exec_scene_module(
                    module_name, file_path=abs_path, verbose=verbose
                )

            # Look for create_app (legacy) or any @entrypoint decorated function
            if "create_app" in module_globals:
                return module_globals["create_app"]  # type: ignore

            # Look for functions decorated with @entrypoint
            # Common names for @entrypoint decorated functions
            import inspect

            app_function_names = ["main", "create_app", "app", "run"]

            def is_entrypoint_decorated(obj: object) -> bool:
                """Check if a callable is decorated with @entrypoint.

                The @entrypoint decorator uses @wraps which sets __wrapped__, and the
                wrapped function accepts a single 'app' parameter.
                """
                if not callable(obj):
                    return False
                # Check if it has __wrapped__ (decorated with @wraps)
                if not hasattr(obj, "__wrapped__"):
                    return False
                try:
                    # Check the wrapped function's signature
                    wrapped_sig = inspect.signature(obj.__wrapped__)  # type: ignore[attr-defined]
                    params = list(wrapped_sig.parameters.values())
                    # Should have single parameter named 'app'
                    if len(params) == 1 and params[0].name == "app":
                        # Try calling with no args - if it works, it's @entrypoint decorated
                        try:
                            # Don't actually call it, just verify it can be called
                            wrapper_sig = inspect.signature(obj, follow_wrapped=False)
                            wrapper_params = list(wrapper_sig.parameters.values())
                            # Wrapper should have optional parameter
                            if (
                                len(wrapper_params) == 1
                                and wrapper_params[0].default
                                is not inspect.Parameter.empty
                            ):
                                return True
                        except Exception:
                            pass
                    return False
                except (ValueError, TypeError):
                    return False

            for name in app_function_names:
                if name in module_globals:
                    obj = module_globals[name]
                    if is_entrypoint_decorated(obj):
                        return obj  # type: ignore

            # Fallback: scan all callables for @entrypoint pattern
            for name, obj in module_globals.items():
                if name.startswith("_"):
                    continue  # Skip private functions
                if is_entrypoint_decorated(obj):
                    _echo(f"   → Found @entrypoint decorated function: {name}")
                    return obj  # type: ignore

            raise RuntimeError(
                "Script must define either a create_app() function or use the @entrypoint decorator.\n"
                "Example: @entrypoint\\ndef main(app: App) -> App: ..."
            )
        except Exception as e:
            _echo(f"Failed to load create_app function: {e}")
            raise

    def load_full_app(script_path: str) -> App:
        """Load and create the initial app instance"""
        create_app_func = load_create_app_function(script_path)
        app = create_app_func()
        return app._set_scene_module(create_app_func.__module__)

    # Store changed files and reload mode for selective reload
    changed_files_cache: dict[str, object] = {"files": set(), "partial_mode": False}

    # Import graph for selective module flushing (built lazily after first load)
    import_graph_holder: dict[str, object] = {"graph": None}

    def watcher_thread(
        script_path: str,
        reload_state: object,
        stop_event: threading.Event,
        partial_mode: bool = False,
    ) -> None:
        """Background thread that watches for file changes and *requests* a reload"""
        _echo("Watching for changes in current directory...")

        # Use shared file watcher from PyBevy utilities
        watch_for_changes(
            watch_path=".",
            reload_state=reload_state,
            stop_event=stop_event,
            changed_files_cache=changed_files_cache,  # type: ignore[arg-type]
            partial_mode=partial_mode,
            verbose=False,  # CLI already has its own output formatting
            log_prefix="",  # CLI writes watcher output directly
        )

        _echo("Watcher thread exiting.")

    def run_app_with_error_handling(app: App, reload_state: object | None) -> None:
        """Run the app with proper error handling and reporting"""
        try:
            # Set up hot reload loader if hot reload is enabled
            # Note: We don't use HotReloadPlugin here because it would try to add
            # systems to the Last schedule after DefaultPlugins has already set it up.
            # Instead, _set_hot_reload_loader() initializes hot reload internally.
            if hasattr(app, "_set_hot_reload_loader") and hot_reload:
                # Create a loader that checks the reload mode from the state
                def loader() -> _CreateAppFunction:
                    files = changed_files_cache.get("files")
                    assert files is None or isinstance(files, set)
                    # Consume this batch before loading. A newer edit that arrives
                    # during the attempt must remain queued for the next reload.
                    changed_files_cache["files"] = set()

                    # Check reload mode from state
                    is_partial = False
                    if reload_state is not None and hasattr(
                        reload_state, "is_partial_reload"
                    ):
                        is_partial = reload_state.is_partial_reload()  # type: ignore

                    if verbose:
                        _echo(
                            f"🔄 Hot reload: partial={is_partial}, changed_files={len(files) if isinstance(files, set) else 0}"
                        )

                    # Load entrypoint with selective reload based on mode
                    return load_create_app_function(
                        script_path, resolve_changed_files(is_partial, files)
                    )

                app._set_hot_reload_loader(loader)

            app.run()
        except KeyboardInterrupt:
            # This should not happen, since pybevy catches the keyboard interrupts
            _echo("\nApplication interrupted by user")
            sys.exit(0)
        except Exception as e:
            _echo(f"\nError running application: {e}")
            _echo("Traceback:")
            traceback.print_exc()
            sys.exit(1)

    watcher: threading.Thread | None = None
    stop_event: threading.Event | None = None

    if hot_reload:
        _echo("Starting with hot-reload enabled...")
        if pause:
            _echo("Start paused: press Space in the window to load the scene.")
        _echo(f"Loading initial app from {script_path}...")

        # Store the partial_reload mode for use in loader
        changed_files_cache["partial_mode"] = partial_reload

        # Initialize component caching based on mode BEFORE first load
        # This ensures components are cached from the start in partial mode
        if partial_reload:
            setup_component_caching(enabled=True, verbose=verbose)
            if verbose:
                _echo("🔧 Component caching initialized for partial reload mode")

        app = load_full_app(script_path)

        # Build import graph for selective module flushing
        try:
            from .util.import_graph import ImportGraph

            graph = ImportGraph(
                watch_root=os.path.dirname(os.path.abspath(script_path)),
                entry_file=os.path.abspath(script_path),
            )
            graph.build()
            import_graph_holder["graph"] = graph
            if verbose:
                _echo(
                    f"📊 Import graph built: {graph.file_count} files, "
                    f"{graph.edge_count} edges"
                )
        except Exception as e:
            if verbose:
                _echo(f"⚠️ Import graph build failed (full flush will be used): {e}")

        reload_state: object | None = getattr(app, "_state", None)

        if reload_state is None:
            _echo("App does not have _state attribute. Hot-reload not available.")
            run_app_with_error_handling(app, None)
            return

        stop_event = threading.Event()

        watcher = threading.Thread(
            target=watcher_thread,
            args=(script_path, reload_state, stop_event, partial_reload),
            daemon=True,
        )
        watcher.start()

        _echo("Hot-reload ready! App running in main thread...")

        try:
            run_app_with_error_handling(app, reload_state)
        finally:
            if stop_event and watcher:
                _echo("\nApplication exited. Stopping watcher thread...")
                stop_event.set()
                watcher.join(timeout=2.0)
                if watcher.is_alive():
                    _echo("Watcher thread did not stop in time.")
                else:
                    _echo("Watcher thread stopped.")

    else:
        _echo(f"Loading and running {script_path}...")
        try:
            app = load_full_app(script_path)
            run_app_with_error_handling(app, None)
            del app
            gc.collect()

            _echo("Application exited normally")
        except Exception as e:
            _echo(f"Failed to load app: {e}")
            traceback.print_exc()
            sys.exit(1)


if __name__ == "__main__":
    main()
