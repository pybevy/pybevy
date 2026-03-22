"""Core rendering pipeline plugin."""

from pybevy.app import App, Plugin

class CorePipelinePlugin(Plugin):
    """Core rendering pipeline plugin.

    Provides the core rendering pipeline including:
    - Prepass rendering (depth, normals, motion vectors)
    - Main pass rendering
    - Post-processing setup

    Required when using rendering features with materials and meshes.
    Must be added after RenderPlugin in the plugin chain.

    Example:
        >>> from pybevy.app import App
        >>> from pybevy.render import RenderPlugin
        >>> from pybevy.core_pipeline import CorePipelinePlugin
        >>> app = App()
        >>> app.add_plugins(RenderPlugin, CorePipelinePlugin)
    """

    def __init__(self) -> None:
        """Create a new CorePipelinePlugin."""
    def build(self, app: App) -> None: ...
