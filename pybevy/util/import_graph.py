"""
AST-based import graph for selective module flushing during hot reload.

Builds a dependency graph by parsing Python source files and extracting
import statements. On file change, expands the changed set to include
all transitive dependents (reverse dependencies) so only affected modules
are flushed from sys.modules.
"""

from __future__ import annotations

import ast
import os
import threading
from collections import defaultdict


class ImportGraph:
    """Dependency graph built from AST import analysis.

    Tracks forward edges (file → its imports) and reverse edges
    (file → files that import it) for selective flushing.
    """

    def __init__(self, watch_root: str, entry_file: str | None = None) -> None:
        self._watch_root = os.path.realpath(watch_root)
        self._entry_file = os.path.realpath(entry_file) if entry_file else None
        self._lock = threading.RLock()
        # forward: file → set of files it imports
        self._forward: dict[str, set[str]] = defaultdict(set)
        # reverse: file → set of files that import it
        self._reverse: dict[str, set[str]] = defaultdict(set)
        self._dynamic_importers: set[str] = set()
        # all known .py files under watch_root
        self._all_files: set[str] = set()

    def build(self) -> None:
        """Scan all .py files under watch_root and build the dependency graph."""
        with self._lock:
            self._build_locked()

    def _build_locked(self) -> None:
        self._forward.clear()
        self._reverse.clear()
        self._dynamic_importers.clear()
        self._all_files.clear()

        for dirpath, _dirnames, filenames in os.walk(self._watch_root):
            # Skip common non-project directories
            basename = os.path.basename(dirpath)
            if basename in (".git", "__pycache__", ".pytest_cache", ".venv",
                            "venv", "node_modules", "target", ".mypy_cache",
                            ".ruff_cache"):
                _dirnames.clear()  # Don't descend
                continue

            for filename in filenames:
                if filename.endswith(".py"):
                    filepath = os.path.realpath(os.path.join(dirpath, filename))
                    self._all_files.add(filepath)

        for filepath in self._all_files:
            self._parse_file(filepath)

    def update_file(self, filepath: str) -> None:
        """Re-parse a single file after it changes (incremental update)."""
        self.update_files({filepath})

    def update_files(self, filepaths: set[str]) -> None:
        """Re-parse one native-watcher batch against a consistent file set."""
        with self._lock:
            self._update_files_locked(filepaths)

    def _update_files_locked(self, filepaths: set[str]) -> None:
        normalized = {os.path.realpath(filepath) for filepath in filepaths}
        self._all_files.update(
            filepath for filepath in normalized if os.path.exists(filepath)
        )

        for filepath in normalized:
            old_imports = self._forward.pop(filepath, set())
            for imported in old_imports:
                self._reverse.get(imported, set()).discard(filepath)
            self._dynamic_importers.discard(filepath)

            if not os.path.exists(filepath):
                # Keep inbound edges so importers of a deleted file are flushed.
                self._all_files.discard(filepath)

        for filepath in normalized:
            if os.path.exists(filepath):
                self._parse_file(filepath)

    def updated_copy(self, filepaths: set[str]) -> ImportGraph:
        """Build a candidate graph without publishing it to concurrent readers."""
        with self._lock:
            candidate = ImportGraph(self._watch_root, self._entry_file)
            candidate._forward = defaultdict(
                set, {path: set(edges) for path, edges in self._forward.items()}
            )
            candidate._reverse = defaultdict(
                set, {path: set(edges) for path, edges in self._reverse.items()}
            )
            candidate._dynamic_importers = set(self._dynamic_importers)
            candidate._all_files = set(self._all_files)
        candidate.update_files(filepaths)
        return candidate

    def publish(self, candidate: ImportGraph) -> None:
        """Atomically replace this graph with a successfully loaded candidate."""
        if (
            candidate._watch_root != self._watch_root
            or candidate._entry_file != self._entry_file
        ):
            raise ValueError("cannot publish an unrelated import graph")
        with candidate._lock:
            with self._lock:
                self._forward = candidate._forward
                self._reverse = candidate._reverse
                self._dynamic_importers = candidate._dynamic_importers
                self._all_files = candidate._all_files

    def relevant_changed_files(self, changed_files: set[str]) -> set[str]:
        """Return the batch only when it can affect the configured entry file."""
        with self._lock:
            return self._relevant_changed_files_locked(changed_files)

    def _relevant_changed_files_locked(self, changed_files: set[str]) -> set[str]:
        normalized = {os.path.realpath(filepath) for filepath in changed_files}
        if self._entry_file is None:
            return normalized

        if self._entry_file in self.expand_changed_files(normalized):
            return normalized

        reachable = {self._entry_file}
        queue = [self._entry_file]
        while queue:
            current = queue.pop()
            for dependency in self._forward.get(current, set()):
                if dependency not in reachable:
                    reachable.add(dependency)
                    queue.append(dependency)
        if reachable & self._dynamic_importers:
            root_prefix = self._watch_root + os.sep
            if any(
                filepath == self._watch_root or filepath.startswith(root_prefix)
                for filepath in normalized
            ):
                return normalized
        return set()

    def expand_changed_files(self, changed_files: set[str]) -> set[str]:
        """Expand a set of changed files to include all transitive dependents.

        Uses BFS over reverse dependency edges. If file A imports file B,
        and B changes, then A must also be flushed.

        Args:
            changed_files: Set of absolute file paths that changed on disk.

        Returns:
            Expanded set including all files that transitively depend on
            any changed file.
        """
        with self._lock:
            return self._expand_changed_files_locked(changed_files)

    def _expand_changed_files_locked(self, changed_files: set[str]) -> set[str]:
        expanded: set[str] = set()
        queue = list(changed_files)
        visited: set[str] = set()

        while queue:
            current = queue.pop()
            current = os.path.realpath(current)
            if current in visited:
                continue
            visited.add(current)
            expanded.add(current)

            # Add all files that import this one
            for dependent in self._reverse.get(current, set()):
                if dependent not in visited:
                    queue.append(dependent)

        return expanded

    def _parse_file(self, filepath: str) -> None:
        """Parse a single Python file and extract import edges."""
        try:
            with open(filepath, encoding="utf-8") as f:
                source = f.read()
        except (OSError, UnicodeDecodeError):
            return

        try:
            tree = ast.parse(source, filename=filepath)
        except SyntaxError:
            return

        file_dir = os.path.dirname(filepath)
        imports: set[str] = set()

        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    resolved = self._resolve_module(alias.name, file_dir)
                    if resolved:
                        imports.add(resolved)

            elif isinstance(node, ast.ImportFrom):
                if node.module is not None:
                    level = node.level or 0
                    resolved = self._resolve_import_from(
                        node.module, level, file_dir, filepath
                    )
                    if resolved:
                        imports.add(resolved)
                    # "from pkg import helper" may name a submodule, not an
                    # attribute of pkg/__init__.py. Both edges are real.
                    for alias in node.names:
                        resolved = self._resolve_import_from(
                            f"{node.module}.{alias.name}", level, file_dir, filepath
                        )
                        if resolved:
                            imports.add(resolved)
                elif node.level and node.level > 0:
                    # Relative import like "from . import foo"
                    for alias in node.names:
                        resolved = self._resolve_import_from(
                            alias.name, node.level, file_dir, filepath
                        )
                        if resolved:
                            imports.add(resolved)

            elif isinstance(node, ast.Call):
                function = node.func
                if (
                    isinstance(function, ast.Name)
                    and function.id == "__import__"
                ) or (
                    isinstance(function, ast.Attribute)
                    and function.attr == "import_module"
                ):
                    self._dynamic_importers.add(filepath)

        self._forward[filepath] = imports
        for imp in imports:
            self._reverse[imp].add(filepath)

    def _resolve_module(self, module_name: str, from_dir: str) -> str | None:
        """Resolve a dotted module name to a file path under watch_root."""
        parts = module_name.split(".")
        # Try resolving as a path relative to watch_root
        candidate = os.path.join(self._watch_root, *parts) + ".py"
        if os.path.realpath(candidate) in self._all_files:
            return os.path.realpath(candidate)

        # Try as package __init__.py
        candidate = os.path.join(self._watch_root, *parts, "__init__.py")
        if os.path.realpath(candidate) in self._all_files:
            return os.path.realpath(candidate)

        # Try relative to the importing file's directory
        candidate = os.path.join(from_dir, *parts) + ".py"
        if os.path.realpath(candidate) in self._all_files:
            return os.path.realpath(candidate)

        return None  # External module (stdlib, site-packages)

    def _resolve_import_from(
        self,
        module_name: str,
        level: int,
        file_dir: str,
        filepath: str,
    ) -> str | None:
        """Resolve a 'from X import Y' statement."""
        if level > 0:
            # Relative import: walk up `level` directories from the file
            base_dir = file_dir
            for _ in range(level - 1):
                base_dir = os.path.dirname(base_dir)

            parts = module_name.split(".") if module_name else []
            candidate = os.path.join(base_dir, *parts) + ".py"
            if os.path.realpath(candidate) in self._all_files:
                return os.path.realpath(candidate)

            # Package __init__.py
            candidate = os.path.join(base_dir, *parts, "__init__.py")
            if os.path.realpath(candidate) in self._all_files:
                return os.path.realpath(candidate)

            return None
        # Absolute import
        return self._resolve_module(module_name, file_dir)

    @property
    def file_count(self) -> int:
        """Number of tracked files."""
        with self._lock:
            return len(self._all_files)

    @property
    def edge_count(self) -> int:
        """Total number of forward dependency edges."""
        with self._lock:
            return sum(len(deps) for deps in self._forward.values())
