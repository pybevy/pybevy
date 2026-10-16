"""Typed material declarations lowered to native binding metadata."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import InitVar
from typing import Annotated, get_args, get_origin

from ..assets import Handle
from ..image import Image, ImageSamplerDescriptor
from ..pbr import StandardMaterial
from ..render import ShaderBuffer
from ._bindings import SamplerBinding, StorageBinding, TextureBinding


def _lower_declarations(
    hints: Mapping[str, object], defaults: dict[str, object]
) -> tuple[dict[str, object], frozenset[str], tuple[str, ...]]:
    lowered: dict[str, object] = {}
    declaration_names: set[str] = set()
    required: list[str] = []
    for name, annotation in hints.items():
        if isinstance(annotation, InitVar):
            if name != "base" or annotation.type != StandardMaterial | None:
                raise TypeError(
                    "Only base: InitVar[StandardMaterial | None] is supported"
                )
            if name not in defaults or defaults.pop(name) is not None:
                raise TypeError("base must default to None")
            continue
        if name == "base":
            raise TypeError("Declare base as InitVar[StandardMaterial | None] = None")
        if get_origin(annotation) is StorageBinding and not name.startswith("_"):
            declaration_names.add(name)
            marker = defaults.get(name)
            if name in defaults:
                if not isinstance(marker, StorageBinding):
                    raise TypeError(
                        f"StorageBinding field '{name}' must use StorageBinding(default=None) or have no default"
                    )
                defaults[name] = None
            else:
                required.append(name)
            element = get_args(annotation)[0]
            binding = StorageBinding(element=element)
            if isinstance(marker, StorageBinding):
                if marker._explicit_element and (
                    marker.wgsl_type != binding.wgsl_type
                    or marker.declarations != binding.declarations
                ):
                    raise TypeError(
                        f"StorageBinding field '{name}' has conflicting element types"
                    )
                if marker.declarations and not marker._explicit_element:
                    binding = StorageBinding(
                        element=element, declarations=marker.declarations
                    )
            lowered[name] = Annotated[Handle[ShaderBuffer] | None, binding]
        elif annotation is StorageBinding:
            raise TypeError(
                f"StorageBinding field '{name}' needs an element type, such as StorageBinding[Vec4]"
            )
        elif annotation in (TextureBinding, SamplerBinding) and not name.startswith(
            "_"
        ):
            assert isinstance(annotation, type)
            declaration_names.add(name)
            if name in defaults:
                marker = defaults[name]
                if not isinstance(marker, annotation):
                    raise TypeError(
                        f"{annotation.__name__} field '{name}' must use {annotation.__name__}(...) or have no default"
                    )
            else:
                required.append(name)
                marker = annotation()
            if isinstance(marker, TextureBinding):
                lowered[name] = Annotated[Handle[Image] | None, marker]
                defaults[name] = None
            else:
                assert isinstance(marker, SamplerBinding)
                lowered[name] = Annotated[ImageSamplerDescriptor, marker]
                defaults[name] = marker._default
        else:
            lowered[name] = annotation
    return lowered, frozenset(declaration_names), tuple(required)
