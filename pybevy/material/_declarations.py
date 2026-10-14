"""Typed material declarations lowered to native binding metadata."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import InitVar
from typing import Annotated, Protocol, cast, get_args, get_origin, overload

from .. import _pybevy  # pyright: ignore[reportAttributeAccessIssue]
from ..assets import Handle
from ..pbr import StandardMaterial, StorageShaderMaterial
from ..render import ShaderBuffer
from ._bindings import StorageBinding


class _StorageInstance(Protocol):
    _shader_mat: StorageShaderMaterial | None
    _storage_buffers: list[Handle[ShaderBuffer] | None]


class Storage[Element]:
    """A read-only GPU array field whose Python value is a ShaderBuffer handle."""

    __module__ = "pybevy.material"

    def __init__(self, *, default: None) -> None:
        if default is not None:
            raise TypeError("Storage default must be None")
        self._slot: int | None = None

    @overload
    def __get__(self, instance: None, owner: type[object]) -> Storage[Element]: ...
    @overload
    def __get__(
        self, instance: object, owner: type[object]
    ) -> Handle[ShaderBuffer] | None: ...

    def __get__(
        self, instance: object | None, owner: type[object]
    ) -> Storage[Element] | Handle[ShaderBuffer] | None:
        if instance is None:
            return self
        slot = self._bound_slot()
        material = cast(_StorageInstance, instance)
        if material._shader_mat is not None:
            return material._shader_mat.get_storage_buffer_at(slot)
        return material._storage_buffers[slot]

    def __set__(self, instance: object, value: Handle[ShaderBuffer] | None) -> None:
        slot = self._bound_slot()
        material = cast(_StorageInstance, instance)
        if material._shader_mat is not None:
            material._shader_mat.set_storage_buffer_at(slot, value)
        else:
            # The same validator runs before constructor values are accepted.
            _pybevy.pbr.StorageShaderMaterial._validate_storage_handle(value)
            material._storage_buffers[slot] = value

    def _bound_slot(self) -> int:
        if self._slot is None:
            raise TypeError("Storage fields require @material")
        return self._slot


def _lower_declarations(
    hints: Mapping[str, object], defaults: dict[str, object]
) -> tuple[dict[str, object], frozenset[str], tuple[str, ...]]:
    lowered: dict[str, object] = {}
    storage_names: set[str] = set()
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
        if get_origin(annotation) is Storage and not name.startswith("_"):
            storage_names.add(name)
            if name in defaults:
                if not isinstance(defaults[name], Storage):
                    raise TypeError(
                        f"Storage field '{name}' must use Storage(default=None) or have no default"
                    )
                defaults[name] = None
            else:
                required.append(name)
            element = get_args(annotation)[0]
            lowered[name] = Annotated[
                Handle[ShaderBuffer] | None, StorageBinding(element=element)
            ]
        elif annotation is Storage:
            raise TypeError(
                f"Storage field '{name}' needs an element type, such as Storage[Vec4]"
            )
        else:
            lowered[name] = annotation
    return lowered, frozenset(storage_names), tuple(required)
