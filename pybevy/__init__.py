from dataclasses import dataclass
from typing import Final

from . import _pybevy as _pybevy  # type: ignore
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
    curve,
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
    picking,
    post_process,
    prelude,
    render,
    shader,
    shape,
    sprite,
    state,
    text,
    time,
    transform,
    ui,
    ui_widgets,
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

# Optional public capabilities compiled into this PyBevy build. The immutable
# set comes from Rust inventory registrations submitted by feature-gated crates;
# names match their Cargo features, such as ``mcp``. Fixed per compiled binary.
FEATURES: Final[frozenset[str]] = _pybevy.features()


# Report the loaded backend's version, including under editable installs.
__version__: Final[str] = _pybevy.__version__


@dataclass(frozen=True, slots=True)
class BuildInfo:
    """Identity of the loaded PyBevy binary, fixed when it was compiled.

    ``git_sha`` and ``git_sha_short`` are hexadecimal commit hashes, or
    ``"unknown"`` when the build had no git checkout (for example an sdist).
    ``git_dirty`` is true when tracked Rust sources or Cargo manifests differed
    from that commit. ``bevy_version`` is the targeted Bevy release series, such
    as ``"0.20"``. ``str()`` gives a one-line form for bug reports.
    """

    version: str
    bevy_version: str
    git_sha: str
    git_sha_short: str
    git_dirty: bool
    profile: str

    def __str__(self) -> str:
        suffix = "-dirty" if self.git_dirty else ""
        return (
            f"{self.version} ({self.git_sha_short}{suffix}, {self.profile}, "
            f"Bevy {self.bevy_version})"
        )


def _load_build_info() -> BuildInfo:
    raw: dict[str, str | bool] = _pybevy.build_info()
    return BuildInfo(
        version=str(raw["version"]),
        bevy_version=str(raw["bevy_version"]),
        git_sha=str(raw["git_sha"]),
        git_sha_short=str(raw["git_sha_short"]),
        git_dirty=raw["git_dirty"] is True,
        profile=str(raw["profile"]),
    )


# Commit, dirty flag, and profile of the loaded backend, for bug reports.
BUILD_INFO: Final[BuildInfo] = _load_build_info()

# Keep top-level CLI import optional for constrained embedding environments.
try:
    from .cli import main
except ImportError:
    main = None  # type: ignore[assignment]  # CLI not available

__all__ = [
    "FEATURES",
    "__version__",
    "BUILD_INFO",
    "BuildInfo",
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
    "curve",
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
    "picking",
    "post_process",
    "prelude",
    "render",
    "shader",
    "shape",
    "sprite",
    "state",
    "text",
    "time",
    "transform",
    "ui",
    "ui_widgets",
    "window",
    "winit",
    "world_serialization",
]

