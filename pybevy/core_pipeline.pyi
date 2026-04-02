"""Core rendering pipeline plugin."""

from typing import ClassVar

from pybevy.app import App, Plugin
from pybevy.ecs import Component

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

class Tonemapping(Component):
    """Tonemapping algorithm for HDR to LDR conversion."""

    def __init__(self) -> None: ...

    NONE: ClassVar[Tonemapping]
    REINHARD: ClassVar[Tonemapping]
    REINHARD_LUMINANCE: ClassVar[Tonemapping]
    ACES_FITTED: ClassVar[Tonemapping]
    AGX: ClassVar[Tonemapping]
    SOMEWHAT_BORING_DISPLAY_TRANSFORM: ClassVar[Tonemapping]
    TONY_MC_MAPFACE: ClassVar[Tonemapping]
