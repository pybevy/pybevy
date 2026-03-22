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
from collections import defaultdict


class ImportGraph:
    """Dependency graph built from AST import analysis.

    Tracks forward edges (file → its imports) and reverse edges
    (file → files that import it) for selective flushing.
    """

    def __init__(self, watch_root: str, entry_file: str | None = None) -> None:
        self._watch_root = os.path.realpath(watch_root)
        self._entry_file = os.path.realpath(entry_file) if entry_file else None
        # forward: file → set of files it imports
        self._forward: dict[str, set[str]] = defaultdict(set)
        # reverse: file → set of files that import it
        self._reverse: dict[str, set[str]] = defaultdict(set)
        # all known .py files under watch_root
        self._all_files: set[str] = set()

    def build(self) -> None:
        """Scan all .py files under watch_root and build the dependency graph."""
        self._forward.clear()
        self._reverse.clear()
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
        filepath = os.path.realpath(filepath)
        if filepath not in self._all_files:
            self._all_files.add(filepath)

        # Remove old forward edges from this file
        old_imports = self._forward.pop(filepath, set())
        for imp in old_imports:
            self._reverse.get(imp, set()).discard(filepath)

        # Re-parse
        self._parse_file(filepath)

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
                elif node.level and node.level > 0:
                    # Relative import like "from . import foo"
                    for alias in node.names:
                        resolved = self._resolve_import_from(
                            alias.name, node.level, file_dir, filepath
                        )
                        if resolved:
                            imports.add(resolved)

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
        return len(self._all_files)

    @property
    def edge_count(self) -> int:
        """Total number of forward dependency edges."""
        return sum(len(deps) for deps in self._forward.values())
