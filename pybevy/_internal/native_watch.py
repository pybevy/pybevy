"""File-change filtering for the embedded `PyBevyPlugin` watcher.

The plugin reloads its module after flushing the user modules beside it, so
only edits that can reach that module matter. This reuses the `pybevy run`
import graph to tell them apart.
"""

from __future__ import annotations

import importlib.util
import os

from ..util.import_graph import ImportGraph


def module_import_graph(module_name: str) -> ImportGraph | None:
    """Import graph rooted at the module's directory, or None when it has no file."""
    spec = importlib.util.find_spec(module_name)
    if spec is None or spec.origin is None or not os.path.isfile(spec.origin):
        return None
    origin = os.path.realpath(spec.origin)
    graph = ImportGraph(watch_root=os.path.dirname(origin), entry_file=origin)
    graph.build()
    return graph


def relevant_changes(graph: ImportGraph, changed: list[str]) -> list[str]:
    """The batch's paths that can affect the module, after updating the graph."""
    relevant = graph.relevant_changed_files({os.path.abspath(path) for path in changed})
    if relevant:
        graph.publish(graph.updated_copy(relevant))
    return sorted(relevant)
