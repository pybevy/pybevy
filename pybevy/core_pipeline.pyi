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
    def __hash__(self) -> int: ...

    def is_enabled(self) -> bool:
        """True unless this is Tonemapping.None_."""

    None_: ClassVar[Tonemapping]
    Linear: ClassVar[Tonemapping]
    Reinhard: ClassVar[Tonemapping]
    ReinhardLuminance: ClassVar[Tonemapping]
    AcesFitted: ClassVar[Tonemapping]
    AgX: ClassVar[Tonemapping]
    SomewhatBoringDisplayTransform: ClassVar[Tonemapping]
    TonyMcMapface: ClassVar[Tonemapping]
    BlenderFilmic: ClassVar[Tonemapping]
    KhronosPbrNeutral: ClassVar[Tonemapping]

class DepthPrepass(Component):
    """If added to Camera3d, depth values are copied to a separate texture."""
    def __init__(self) -> None: ...

class NormalPrepass(Component):
    """If added to Camera3d, vertex world normals are copied to a separate texture."""
    def __init__(self) -> None: ...

class MotionVectorPrepass(Component):
    """If added to Camera3d, screen space motion vectors are copied to a separate texture."""
    def __init__(self) -> None: ...

class DeferredPrepass(Component):
    """If added to Camera3d, deferred materials are rendered to the deferred gbuffer texture.
    Deferred cameras also require Msaa.Off and DepthPrepass. With DepthPrepass
    but no DeferredPrepass, forced-deferred meshes can panic in Bevy's prepass
    queue and prevent frame readback.
    """
    def __init__(self) -> None: ...
