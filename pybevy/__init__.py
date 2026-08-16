from . import (
    a11y,
    animation,
    app,
    array,
    assets,
    audio,
    camera,
    collections,
    color,
    core_pipeline,
    ecs,
    gizmos,
    gltf,
    image,
    input,
    light,
    material,
    math,
    mesh,
    pbr,
    post_process,
    prelude,
    render,
    shader,
    sprite,
    text,
    time,
    transform,
    ui,
    window,
    winit,
    world_serialization,
)
from ._constants import _apply as _apply_constant_descriptors
from .decorators import (
    clear_component_cache,
    component,
    enable_component_caching,
    event,
    is_component_decorated,
    message,
    plugin,
    resource,
)
from .ecs import system_set

_apply_constant_descriptors()

# Keep top-level CLI import optional for constrained embedding environments.
try:
    from .cli import main
except ImportError:
    main = None  # type: ignore[assignment]  # CLI not available

__all__ = [
    "main",
    # decorators
    "component",
    "event",
    "message",
    "resource",
    "plugin",
    "system_set",
    # hot reload utilities
    "clear_component_cache",
    "enable_component_caching",
    "is_component_decorated",
    # modules
    "a11y",
    "animation",
    "app",
    "array",
    "assets",
    "audio",
    "camera",
    "collections",
    "color",
    "core_pipeline",
    "ecs",
    "gizmos",
    "gltf",
    "image",
    "input",
    "light",
    "material",
    "math",
    "mesh",
    "pbr",
    "post_process",
    "prelude",
    "render",
    "shader",
    "sprite",
    "text",
    "time",
    "transform",
    "ui",
    "window",
    "winit",
    "world_serialization",
]
