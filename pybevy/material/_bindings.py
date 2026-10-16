"""Immutable declaration metadata for the opt-in material storage layout."""

from collections.abc import Mapping
from typing import Annotated, NamedTuple, get_args, get_origin

from ..math import Vec4
from ._layout import _is_texture_annotation, _validate_wgsl_identifier


class StorageBuffer:
    """Mark a Handle[ShaderBuffer] annotation as a read-only vec4 storage binding."""

    __module__ = "pybevy.material"
    __slots__ = ()

    def __init__(self, *, element: type[Vec4] = Vec4) -> None:
        if element is not Vec4:
            raise TypeError(
                "Material storage buffers currently support only element=Vec4"
            )

    @property
    def element(self) -> type[Vec4]:
        return Vec4


class _StorageField(NamedTuple):
    name: str
    binding: int = 109


def _partition_storage_fields(
    hints: Mapping[str, object],
) -> tuple[dict[str, object], tuple[_StorageField, ...]]:
    ordinary: dict[str, object] = {}
    storage: list[_StorageField] = []
    for name, annotation in hints.items():
        if get_origin(annotation) is Annotated:
            field_type, *metadata = get_args(annotation)
            markers = [value for value in metadata if isinstance(value, StorageBuffer)]
            if markers and not name.startswith("_"):
                if len(markers) != 1 or not _is_texture_annotation(field_type):
                    raise TypeError(
                        f"Storage field '{name}' needs Annotated[Handle[ShaderBuffer] | None, StorageBuffer()]"
                    )
                _validate_wgsl_identifier(name, "storage field")
                if name == "material":
                    raise TypeError(
                        "Storage field 'material' conflicts with the generated uniform binding"
                    )
                if storage:
                    raise TypeError(
                        "A material supports one storage buffer field (binding 109)"
                    )
                storage.append(_StorageField(name))
                continue
            annotation = field_type
        ordinary[name] = annotation
    return ordinary, tuple(storage)
