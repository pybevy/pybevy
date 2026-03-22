from . import (
    a11y,
    animation,
    app,
    assets,
    audio,
    camera,
    color,
    core_pipeline,
    ecs,
    gltf,
    image,
    input,
    light,
    material,
    math,
    mesh,
    pbr,
    prelude,
    render,
    scene,
    shader,
    sprite,
    text,
    time,
    transform,
    ui,
    wgpu,
    window,
    winit,
)
from ._constants import _apply as _apply_constant_descriptors
from .decorators import (
    clear_component_cache,
    component,
    enable_component_caching,
    is_component_decorated,
    plugin,
    resource,
)

_apply_constant_descriptors()

# CLI import is optional (requires watchfiles)
try:
    from .cli import main
except ImportError:
    main = None  # type: ignore[assignment]  # CLI not available

__all__ = [
    "main",
    # decorators
    "component",
    "resource",
    # "event",
    "plugin",
    # hot reload utilities
    "clear_component_cache",
    "enable_component_caching",
    "is_component_decorated",
    # modules
    "a11y",
    "animation",
    "app",
    "assets",
    "audio",
    "camera",
    "color",
    "core_pipeline",
    "ecs",
    "gltf",
    "image",
    "input",
    "light",
    "material",
    "math",
    "mesh",
    "pbr",
    "prelude",
    "render",
    "scene",
    "shader",
    "sprite",
    "text",
    "time",
    "transform",
    "ui",
    "wgpu",
    "window",
    "winit",
]
