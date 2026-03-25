"""
Shared hot reload utilities for PyBevy.

This module provides reusable hot reload infrastructure for:
- PyBevy CLI (pybevy watch/dev/run commands)
- External tools and custom launchers
- Any application that needs hot reload with component caching

Key features:
- Component caching management (timing is critical!)
- Entrypoint detection (@entrypoint decorated functions)
- Module loading with selective reload
- Hot reload loader creation
- File watching with automatic reload triggering
"""

import inspect
import os
import runpy
import sys
import threading
import time
from collections.abc import Callable
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..app import App


def flush_user_modules(
    project_dir: str,
    verbose: bool = False,
    graph: object | None = None,
    changed_files: set[str] | None = None,
) -> list[str]:
    """
    Remove user project modules from sys.modules.

    When *graph* and *changed_files* are both provided, only modules whose
    source files are in the transitive dependent set (computed by the import
    graph) are flushed — this is the selective fast-path.

    Otherwise walks sys.modules and removes any module whose ``__file__`` is
    under *project_dir* (full flush).

    Modules belonging to pybevy, the stdlib, and site-packages are never
    removed.

    Args:
        project_dir: Absolute path to the project root directory.
        verbose: If True, print each flushed module name.
        graph: Optional ``ImportGraph`` instance for selective flushing.
        changed_files: Optional set of changed file paths (used with *graph*).

    Returns:
        List of module names that were flushed.
    """
    import importlib
    import sysconfig

    project_prefix = os.path.realpath(project_dir) + os.sep
    stdlib_prefix = os.path.realpath(sysconfig.get_paths()["stdlib"]) + os.sep

    # Selective flush: use import graph to compute affected files
    affected_files: set[str] | None = None
    if graph is not None and changed_files is not None and changed_files:
        from .import_graph import ImportGraph
        if isinstance(graph, ImportGraph):
            # Update the graph for changed files first
            for f in changed_files:
                graph.update_file(f)
            affected_files = graph.expand_changed_files(changed_files)
            if verbose:
                print(f"   → Import graph: {len(changed_files)} changed → {len(affected_files)} affected")

    to_remove: list[str] = []

    for name, mod in list(sys.modules.items()):
        if name.startswith(("pybevy.", "_pybevy", "pybevy")):
            continue
        fpath = getattr(mod, "__file__", None)
        if fpath is None:
            continue
        fpath = os.path.realpath(fpath)
        if not fpath.startswith(project_prefix):
            continue
        if "site-packages" in fpath or "dist-packages" in fpath:
            continue
        if fpath.startswith(stdlib_prefix):
            continue

        # If selective mode, only flush files in the affected set
        if affected_files is not None and fpath not in affected_files:
            continue

        to_remove.append(name)

    for name in to_remove:
        del sys.modules[name]
    importlib.invalidate_caches()

    if verbose:
        for name in to_remove:
            print(f"   → Flushed: {name}")

    return to_remove


def reload_module_from_source(module_path: str, module_name: str | None = None, project_dir: str | None = None, verbose: bool = False) -> dict:
    """
    Reload a Python module from source, bypassing .pyc bytecode cache.

    Uses ``runpy.run_path`` so the source file is always re-read from disk.
    When *project_dir* is given, all user modules under that directory are
    flushed from ``sys.modules`` first (transitive dependency reload).
    Otherwise only the single *module_name* entry is removed.

    This is the canonical reload mechanism shared by the CLI hot reload
    (``load_entrypoint_function``) and the native plugin hot reload
    (``PyBevyPlugin.with_hot_reload``).

    Args:
        module_path: Absolute filesystem path to the ``.py`` file.
        module_name: Optional module name for ``sys.modules`` cleanup and
            ``run_name``.  Defaults to *module_path* (the key that
            ``runpy.run_path`` uses internally).
        project_dir: If given, flush all user modules under this directory
            from ``sys.modules`` before reloading.  This ensures transitive
            dependencies are also reloaded.
        verbose: If True, print flushed module names.

    Returns:
        The module globals dict produced by ``runpy.run_path``.
    """
    import importlib
    import importlib.util

    key = module_name if module_name is not None else module_path

    if project_dir is not None:
        flush_user_modules(project_dir, verbose=verbose)
    else:
        if key in sys.modules:
            del sys.modules[key]
        importlib.invalidate_caches()

    return runpy.run_path(module_path, init_globals={}, run_name=key)


def reload_module_by_name(module_name: str) -> dict:
    """
    Reload a module by its importable name, bypassing .pyc cache.

    Resolves the module name to a source path via ``importlib.util.find_spec``
    and delegates to :func:`reload_module_from_source`.

    Used by the native plugin hot reload (``PyBevyPlugin.with_hot_reload``).

    Args:
        module_name: Dotted module name (e.g. ``"game_logic"``).

    Returns:
        The module globals dict.

    Raises:
        ImportError: If the module source cannot be located.
    """
    import importlib.util

    if module_name in sys.modules:
        del sys.modules[module_name]

    spec = importlib.util.find_spec(module_name)
    if spec is None or spec.origin is None:
        raise ImportError(f"Cannot find source for module '{module_name}'")

    return reload_module_from_source(spec.origin, module_name=module_name)


def setup_component_caching(enabled: bool, verbose: bool = False) -> None:
    """
    Setup component caching for hot reload.

    CRITICAL: Must be called BEFORE loading the initial app to ensure
    components are cached from the start. This preserves component identity
    across hot reloads.

    Args:
        enabled: True to enable caching (for hot reload), False to disable
        verbose: If True, print debug information

    Example:
        # Before loading your app:
        setup_component_caching(enabled=True, verbose=True)

        # Then load and run your app:
        app = load_app(script_path)
        app.run()
    """
    import pybevy

    if enabled:
        pybevy.enable_component_caching(True, verbose=verbose)
        if verbose:
            print("🔧 Component caching ENABLED for hot reload")
    else:
        pybevy.enable_component_caching(False, verbose=verbose)
        if verbose:
            print("🔧 Component caching DISABLED")


def find_entrypoint(module_globals: dict) -> Callable:
    """
    Find the @entrypoint decorated function in module globals.

    Searches for common function names first (main, create_app, app, run),
    then falls back to scanning all callables for @entrypoint decorator.

    Args:
        module_globals: Dictionary of module globals (from runpy.run_path or similar)

    Returns:
        The @entrypoint decorated function

    Raises:
        RuntimeError: If no @entrypoint function is found

    Example:
        module_globals = runpy.run_path(script_path)
        entrypoint_func = find_entrypoint(module_globals)
        app = entrypoint_func()  # Call with no args to create App
    """

    def is_entrypoint_decorated(obj: object) -> bool:
        """Check if a callable is decorated with @entrypoint."""
        if not callable(obj):
            return False
        # Check if it has __wrapped__ (decorated with @wraps)
        if not hasattr(obj, '__wrapped__'):
            return False
        try:
            # Check the wrapped function's signature
            wrapped_sig = inspect.signature(obj.__wrapped__)  # type: ignore[attr-defined]
            params = list(wrapped_sig.parameters.values())
            # Should have single parameter named 'app'
            if len(params) == 1 and params[0].name == 'app':
                # Verify wrapper has optional parameter
                wrapper_sig = inspect.signature(obj, follow_wrapped=False)
                wrapper_params = list(wrapper_sig.parameters.values())
                # Wrapper should have optional parameter
                if len(wrapper_params) == 1 and wrapper_params[0].default is not inspect.Parameter.empty:
                    return True
            return False
        except (ValueError, TypeError):
            return False

    # Try common names first
    app_function_names = ["main", "create_app", "app", "run"]
    for name in app_function_names:
        if name in module_globals:
            obj = module_globals[name]
            if is_entrypoint_decorated(obj):
                return obj  # type: ignore

    # Fallback: scan all callables for @entrypoint pattern
    for name, obj in module_globals.items():
        if name.startswith('_'):
            continue  # Skip private functions
        if is_entrypoint_decorated(obj):
            return obj  # type: ignore

    raise RuntimeError(
        "Script must use the @entrypoint decorator.\n"
        "Example: @entrypoint\\ndef main(app: App) -> App: ..."
    )


def load_entrypoint_function(
    script_path: str,
    changed_files: set[str] | None = None,
    verbose: bool = False,
) -> Callable:
    """
    Load the entrypoint function from a script, optionally with selective reload.

    This function handles:
    - Component caching setup (based on changed_files)
    - Module reloading (selective for partial, full for changed_files=None)
    - Package vs standalone script detection
    - Entrypoint detection

    Args:
        script_path: Absolute path to the Python script
        changed_files: Set of changed file paths for selective reload.
                      None = full reload (clear component cache)
                      set() = partial reload (enable component caching)
        verbose: If True, print debug information

    Returns:
        The @entrypoint decorated function (NOT called yet)

    Example:
        # Full reload:
        entrypoint = load_entrypoint_function(script_path, changed_files=None)

        # Partial reload with changed files:
        changed = {'/path/to/changed.py'}
        entrypoint = load_entrypoint_function(script_path, changed_files=changed)
    """
    abs_path = os.path.abspath(script_path)
    parent_dir = os.path.dirname(abs_path)

    # Handle component caching
    import pybevy
    from pybevy.decorators import _component_cache_enabled  # type: ignore[attr-defined]

    if changed_files is not None:
        # Partial reload: enable component caching if not already enabled
        if not _component_cache_enabled:
            pybevy.enable_component_caching(True, verbose=verbose)
        if verbose:
            print("   → Component caching ENABLED (partial reload)")
    else:
        # Full reload: clear component cache
        pybevy.clear_component_cache(verbose=verbose)
        if verbose:
            print("   → Component cache CLEARED (full reload)")

    # Add script directory to path
    if parent_dir not in sys.path:
        sys.path.insert(0, parent_dir)

    # Ensure sys.argv only contains the script path so user argparse works correctly
    sys.argv = [script_path]

    # Reload module from source (shared with native plugin hot reload)
    # Pass project_dir to flush all user modules for transitive dependency reload
    module_globals = reload_module_from_source(
        script_path, project_dir=parent_dir, verbose=verbose,
    )

    # Find and return entrypoint
    return find_entrypoint(module_globals)


def create_hot_reload_loader(
    script_path: str,
    changed_files_cache: dict[str, object],
    reload_state: object,
    verbose: bool = False,
    entrypoint_wrapper: Callable[[Callable], Callable] | None = None,
) -> Callable[[], Callable[[], "App"]]:
    """
    Create a hot reload loader function for use with app._set_hot_reload_loader().

    The returned loader function will be called by PyBevy's hot reload system
    when a reload is triggered. It loads the fresh entrypoint function and
    returns it to PyBevy.

    Args:
        script_path: Absolute path to the script to reload
        changed_files_cache: Dict with 'files' key containing set of changed paths
        reload_state: The app._state object with is_partial_reload() method
        verbose: If True, print debug information
        entrypoint_wrapper: Optional function to wrap the loaded entrypoint.
                           Takes entrypoint function, returns wrapped function.
                           Useful for re-adding plugins or modifying app setup.

    Returns:
        A loader function that returns the fresh entrypoint function

    Example (basic):
        changed_files_cache = {"files": set()}
        reload_state = app._state

        loader = create_hot_reload_loader(
            script_path,
            changed_files_cache,
            reload_state,
            verbose=True
        )

        app._set_hot_reload_loader(loader)

    Example (with wrapper to re-add plugin):
        def wrap_with_plugin(entrypoint_func):
            def wrapped(app):
                configured = entrypoint_func(app)
                configured.add_plugins(MyPlugin(config))
                return configured
            return wrapped

        loader = create_hot_reload_loader(
            script_path,
            changed_files_cache,
            reload_state,
            entrypoint_wrapper=wrap_with_plugin,
        )
    """

    def loader() -> Callable[[], "App"]:
        """Loader function called by PyBevy during hot reload."""
        try:
            if verbose:
                print("🔄 Hot reload loader called")

            files = changed_files_cache.get("files")
            assert files is None or isinstance(files, set)

            # Check the actual reload mode from the reload state
            is_partial = False
            if reload_state is not None and hasattr(reload_state, "is_partial_reload"):
                is_partial = reload_state.is_partial_reload()  # type: ignore

            if verbose:
                print(f"🔍 Reload mode: partial={is_partial}, changed_files={len(files) if isinstance(files, set) else 0}")

            # In partial mode, use selective reload with changed files
            # In full mode, pass None to force full module reload
            entrypoint_func = load_entrypoint_function(
                script_path,
                files if (is_partial and isinstance(files, set)) else None,
                verbose=verbose,
            )

            # Clear the file cache after loading
            changed_files_cache["files"] = set()

            # Apply wrapper if provided (e.g., to re-add plugins)
            if entrypoint_wrapper is not None:
                entrypoint_func = entrypoint_wrapper(entrypoint_func)
                if verbose:
                    print("🔧 Applied entrypoint wrapper")

            if verbose:
                print("✅ Hot reload loader complete")

            return entrypoint_func

        except Exception as e:
            # CRITICAL: Print and flush immediately so error reaches pipe
            # Python's stderr is block-buffered when piped, so explicit flush is needed
            import sys
            import traceback
            print(f"\n❌ Hot reload failed: {type(e).__name__}: {e}", file=sys.stderr)
            traceback.print_exc(file=sys.stderr)
            sys.stderr.flush()  # Force flush to pipe!
            raise  # Re-raise for pybevy Rust to handle

    return loader


def watch_for_changes(
    watch_path: str,
    reload_state: object,
    stop_event: threading.Event,
    changed_files_cache: dict[str, set[str]],
    partial_mode: bool = True,
    ignore_patterns: list[str] | None = None,
    verbose: bool = False,
    log_prefix: str = "[PyBevy]",
) -> None:
    """
    Background thread that watches for file changes and triggers hot reload.

    This function encapsulates the file watching logic used by PyBevy CLI
    and external tools. It watches a directory for Python file changes
    and triggers reload via the reload_state object.

    Args:
        watch_path: Directory path to watch for changes
        reload_state: App reload state object with trigger methods
        stop_event: Threading event to signal watcher should stop
        changed_files_cache: Dict with 'files' key to store changed file paths
        partial_mode: Whether to request partial reload (True) or full reload (False)
        ignore_patterns: List of path patterns to ignore (defaults to common patterns)
        verbose: If True, print debug information
        log_prefix: Prefix for log messages (e.g., "[PyBevy]" or "[MyApp]")

    Example:
        stop_event = threading.Event()
        changed_files_cache = {"files": set()}

        watcher_thread = threading.Thread(
            target=watch_for_changes,
            args=(script_dir, app._state, stop_event, changed_files_cache),
            kwargs={"verbose": True, "log_prefix": "[MyApp]"},
            daemon=True,
        )
        watcher_thread.start()
    """
    # Check if watchfiles is available
    try:
        from watchfiles import watch
    except ImportError:
        print(f"{log_prefix} Warning: watchfiles not installed, hot reload unavailable")
        return

    # Default ignore patterns
    if ignore_patterns is None:
        ignore_patterns = [".git", "__pycache__", ".pytest_cache", ".venv", "target"]

    if verbose:
        print(f"{log_prefix} File watcher started, watching: {watch_path}")
        print(f"{log_prefix} Ignoring patterns: {ignore_patterns}")

    # Determine reload trigger method (prefers new API)
    trigger_reload: Callable[[], None]
    if hasattr(reload_state, "trigger_reload_if_needed"):
        # New method: honors any reload mode already set (e.g., by F5)
        trigger_reload = lambda: reload_state.trigger_reload_if_needed(partial_mode)  # type: ignore
        if verbose:
            print(f"{log_prefix} Using trigger_reload_if_needed(partial={partial_mode})")
    elif partial_mode and hasattr(reload_state, "set_pending_partial_reload"):
        # Fallback: partial reload
        trigger_reload = reload_state.set_pending_partial_reload  # type: ignore
        if verbose:
            print(f"{log_prefix} Using set_pending_partial_reload()")
    elif not partial_mode and hasattr(reload_state, "set_pending_reload"):
        # Fallback: full reload
        trigger_reload = reload_state.set_pending_reload  # type: ignore
        if verbose:
            print(f"{log_prefix} Using set_pending_reload()")
    else:
        print(f"{log_prefix} ERROR: No reload method available on reload_state")
        return

    # Watch loop
    for changes in watch(watch_path, stop_event=stop_event):
        if stop_event.is_set():
            if verbose:
                print(f"{log_prefix} File watcher received stop signal")
            break

        try:
            if verbose:
                print(f"{log_prefix} Detected {len(changes)} file changes")

            # Filter for Python files, excluding ignored patterns
            python_changes = []
            for change in changes:
                path_str = str(change[1])

                # Skip ignored patterns
                if any(skip in path_str for skip in ignore_patterns):
                    if verbose:
                        print(f"{log_prefix}   Skipping: {path_str} (ignored pattern)")
                    continue

                # Accept Python files
                if path_str.endswith(".py"):
                    python_changes.append(change)
                    if verbose:
                        print(f"{log_prefix}   Accepted: {path_str}")

            if python_changes:
                # Small delay for file write completion
                time.sleep(0.01)

                # Store changed files for selective reload
                changed_paths = {os.path.abspath(str(c[1])) for c in python_changes}
                changed_files_cache["files"] = changed_paths

                # Log changes
                if verbose:
                    print(f"{log_prefix} Triggering reload for {len(changed_paths)} files:")
                    for path in changed_paths:
                        try:
                            rel_path = os.path.relpath(path, watch_path)
                        except ValueError:
                            rel_path = path
                        print(f"{log_prefix}   - {rel_path}")

                # Trigger reload
                trigger_reload()

                if verbose:
                    print(f"{log_prefix} Reload request sent")

        except Exception as e:
            print(f"{log_prefix} Error in file watcher: {e}")
            import traceback
            traceback.print_exc()

    if verbose:
        print(f"{log_prefix} File watcher stopped")
