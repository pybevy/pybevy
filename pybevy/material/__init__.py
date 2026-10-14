"""Native material types and the @material decorator.

Native classes mirror ``bevy::material``. The @material decorator defines
custom shader materials from Python, generating WGSL struct declarations and
handling std140 packing automatically.

Example:
    @material(fragment_shader="shaders/lava.wgsl")
    class LavaMaterial(Material):
        color: LinearRgba = LinearRgba(1.0, 0.3, 0.0, 1.0)
        crack_scale: float = 5.0
        speed: float = 2.0
"""

from __future__ import annotations

import sys
from collections.abc import Callable
from copy import copy
from dataclasses import InitVar
from inspect import signature as _signature
from threading import Lock
from typing import Any, Protocol, TypeVar, cast, dataclass_transform, get_type_hints

from .. import _pybevy  # type: ignore
from ..assets import Handle
from ..color import LinearRgba
from ..image import Image, ImageSamplerDescriptor
from ..math import Vec2, Vec3, Vec4
from ..pbr import (
    MeshMaterial3dShader,
    MeshMaterial3dStorageShader,
    ShaderMaterial,
    StandardMaterial,
    StorageShaderMaterial,
)
from ..render import ShaderBuffer
from ._bindings import SamplerBinding as SamplerBinding
from ._bindings import StorageBinding as StorageBinding
from ._bindings import TextureBinding as TextureBinding
from ._bindings import (
    _copy_sampler,
    _partition_sampler_fields,
    _partition_storage_fields,
    _partition_texture_fields,
)
from ._declarations import _lower_declarations
from ._layout import (
    _MAX_TEXTURE_SLOTS,
    _compute_layout,
    _generate_wgsl,
    _validate_binding_names,
)
from ._scalars import U32 as U32
from ._structs import (
    ShaderTypeField as ShaderTypeField,
)
from ._structs import (
    ShaderTypeLayout as ShaderTypeLayout,
)
from ._structs import (
    pack_shader_data as pack_shader_data,
)
from ._structs import (
    shader_type as shader_type,
)
from ._structs import (
    shader_type_layout as shader_type_layout,
)

_Material = _pybevy.pbr.Material  # type: ignore
_PACKED_VECTOR_TYPES = (Vec2, Vec3, Vec4, LinearRgba)
MaterialT = TypeVar("MaterialT", bound=_Material)  # type: ignore[valid-type]
AlphaMode = _pybevy.material.AlphaMode  # type: ignore
OpaqueRendererMethod = _pybevy.material.OpaqueRendererMethod  # type: ignore


_MaterialLayoutSignature = tuple[object, ...]

_material_type_cache: dict[str, tuple[int, _MaterialLayoutSignature]] = {}
_next_material_type_id = 1
_material_type_lock = Lock()
_material_type_cache_enabled = bool(
    getattr(sys.modules.get("pybevy.decorators"), "_component_cache_enabled", False)
)


def _clear_material_type_cache() -> None:
    """Drop full-reload aliases while never reusing an old process-local ID."""
    with _material_type_lock:
        _material_type_cache.clear()


def _set_material_type_caching(enabled: bool) -> None:
    """Mirror the component cache mode used by the hot-reload loader."""
    global _material_type_cache_enabled
    with _material_type_lock:
        _material_type_cache_enabled = enabled


def _logical_material_type_id(
    qualified_name: str,
    signature: _MaterialLayoutSignature,
) -> int:
    """Allocate or reuse a logical identity according to the reload mode."""
    global _next_material_type_id
    with _material_type_lock:
        cached = _material_type_cache.get(qualified_name)
        if (
            _material_type_cache_enabled
            and cached is not None
            and cached[1] == signature
        ):
            return cached[0]

        logical_type_id = _next_material_type_id
        _next_material_type_id += 1
        _material_type_cache[qualified_name] = (logical_type_id, signature)
        return logical_type_id


def _pack_value(
    data: list[float | int],
    float_index: int,
    value: Any,  # noqa: ANN401
    typ: type,
) -> None:
    """Pack a Python value into the flat uniform values at the given index."""
    if typ is float:
        data[float_index] = float(value)
    elif typ is int:
        if type(value) is not int:
            raise TypeError(f"must be an int; got {type(value).__name__}")
        if not -(2**31) <= value < 2**31:
            raise ValueError("must fit i32")
        data[float_index] = value
    elif typ is Vec2:
        data[float_index] = value.x
        data[float_index + 1] = value.y
    elif typ is Vec3:
        data[float_index] = value.x
        data[float_index + 1] = value.y
        data[float_index + 2] = value.z
    elif typ is Vec4:
        data[float_index] = value.x
        data[float_index + 1] = value.y
        data[float_index + 2] = value.z
        data[float_index + 3] = value.w
    elif typ is LinearRgba:
        data[float_index] = value.red
        data[float_index + 1] = value.green
        data[float_index + 2] = value.blue
        data[float_index + 3] = value.alpha


def _read_value(
    data: list[float | int],
    float_index: int,
    typ: type,
) -> Any:  # noqa: ANN401
    """Read a Python value from the flat uniform values."""
    if typ is float:
        return data[float_index]
    if typ is int:
        return int(data[float_index])
    if typ is Vec2:
        return Vec2(data[float_index], data[float_index + 1])
    if typ is Vec3:
        return Vec3(data[float_index], data[float_index + 1], data[float_index + 2])
    if typ is Vec4:
        return Vec4(
            data[float_index],
            data[float_index + 1],
            data[float_index + 2],
            data[float_index + 3],
        )
    if typ is LinearRgba:
        return LinearRgba(
            data[float_index],
            data[float_index + 1],
            data[float_index + 2],
            data[float_index + 3],
        )
    return None


class _BaseInstance(Protocol):
    _shader_mat: ShaderMaterial | StorageShaderMaterial | None
    _base: StandardMaterial


class _StorageInstance(Protocol):
    _shader_mat: StorageShaderMaterial | None
    _storage_buffers: list[Handle[ShaderBuffer] | None]


class _SamplerInstance(Protocol):
    _shader_mat: StorageShaderMaterial | None
    _samplers: list[ImageSamplerDescriptor]


_SENTINEL = object()


def _is_native_material(cls: type) -> bool:
    """A material class defined in Rust rather than by @material."""
    return cls.__module__ == _Material.__module__


def _inherited_material_default(cls: type, name: str, typ: type) -> object:
    for base in cls.__mro__[1:]:
        if name not in base.__dict__:
            continue
        declarations = base.__dict__.get("_material_binding_declarations", {})
        if name in declarations:
            annotation, default = declarations[name]
            return default if annotation == typ else _SENTINEL
        fields = base.__dict__.get("_material_field_map", {})
        if name in fields and fields[name].python_type is typ:
            return fields[name].default
        sampler_defaults = base.__dict__.get("_material_sampler_defaults", {})
        if name in sampler_defaults:
            return sampler_defaults[name]
        bool_fields = base.__dict__.get("_material_bool_field_map", {})
        if typ is bool and name in bool_fields:
            return bool_fields[name].default
        break
    return _SENTINEL


@dataclass_transform(kw_only_default=True, eq_default=False)
def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
    *,
    prepass_vertex_shader: str | None = None,
    prepass_fragment_shader: str | None = None,
    deferred_vertex_shader: str | None = None,
    deferred_fragment_shader: str | None = None,
    base: StandardMaterial | None = None,
    **removed_settings: object,
) -> Callable[[type[MaterialT]], type[MaterialT]]:
    """Decorator to define a custom shader material.

    Generates WGSL struct + binding declaration and handles data packing.

    Args:
        fragment_shader: Asset path to custom fragment shader (relative to assets/ directory).
        vertex_shader: Asset path to custom vertex shader (relative to assets/ directory).
        prepass_vertex_shader: WGSL vertex shader for depth, normals, motion and shadows.
        prepass_fragment_shader: WGSL fragment shader for those prepasses, including discard.
        deferred_vertex_shader: WGSL vertex shader for deferred rendering.
        deferred_fragment_shader: WGSL fragment shader writing the deferred G-buffer.
        base: StandardMaterial template, copied at decoration and per instance.

    Example:
        @material(
            fragment_shader="shaders/hologram.wgsl",
            base=StandardMaterial(alpha_mode=AlphaMode.Blend(), double_sided=True),
        )
        class HologramMaterial(Material):
            opacity: float = 0.5
            scan_speed: float = 2.0

        # base StandardMaterial is auto-constructed with decorator defaults:
        mat = HologramMaterial(opacity=0.8)

        # Or override base explicitly:
        mat = HologramMaterial(
            base=StandardMaterial(alpha_mode=AlphaMode.Add()),
            opacity=0.8,
        )
    """
    for name, value in removed_settings.items():
        if name in {"alpha_mode", "double_sided", "cull_mode", "unlit", "depth_bias"}:
            raise TypeError(f"@material({name}=...) was removed; use base=StandardMaterial({name}={value!r})")
        raise TypeError(f"material() got an unexpected keyword argument '{name}'")
    if base is not None and not isinstance(base, StandardMaterial):
        raise TypeError("Material base template must be a StandardMaterial or None")
    base_template = copy(base) if base is not None else StandardMaterial()

    def decorator(cls: type[MaterialT]) -> type[MaterialT]:
        # Rejected before any mutation: the decorator rewrites the class in
        # place, and rewriting a native wrapper leaves its constructor calling
        # itself.
        if cls is _Material or _is_native_material(cls):
            raise TypeError(
                f"@material cannot be applied to '{cls.__name__}': it is a native "
                "material provided by PyBevy. Define your own class inheriting "
                "Material instead."
            )
        if not issubclass(cls, _Material):
            raise TypeError(
                f"Material class '{cls.__name__}' must inherit from Material: "
                f"write `class {cls.__name__}(Material)` so Assets[{cls.__name__}] "
                f"and MeshMaterial3d[{cls.__name__}] accept it"
            )

        # Use Any-typed alias to allow dynamic attribute assignment on cls
        _cls: Any = cls
        qualified_name = f"{cls.__module__}.{cls.__qualname__}"

        # Collect type hints and defaults
        hints = get_type_hints(cls, include_extras=True)
        if "base" in hints and not isinstance(hints["base"], InitVar):
            raise TypeError(f"Material {cls.__name__}.base is reserved for the StandardMaterial constructor argument")
        defaults: dict[str, Any] = {}
        for name, typ in hints.items():
            if hasattr(cls, name):
                value = getattr(cls, name)
                if isinstance(value, property | StorageBinding | TextureBinding | SamplerBinding) and name not in cls.__dict__:
                    value = _inherited_material_default(cls, name, typ)
                if value is not _SENTINEL:
                    defaults[name] = value

        declared_types = dict(hints)
        declared_defaults = dict(defaults)
        hints, concise_bindings, required_bindings = _lower_declarations(hints, defaults)

        # Compute layout (separates uniform, bool, and texture fields)
        sampler_hints, sampler_fields = _partition_sampler_fields(hints)
        sampler_defaults = {
            field.name: _copy_sampler(defaults.get(field.name, field.layout._default))
            for field in sampler_fields
        }
        texture_hints, texture_layouts = _partition_texture_fields(sampler_hints)
        try:
            ordinary_hints, storage_fields = _partition_storage_fields(texture_hints)
        except ValueError as error:
            raise TypeError(f"Material {qualified_name}, fields [{', '.join(texture_hints)}]: {error}") from error
        layout, bool_fields, texture_fields = _compute_layout(ordinary_hints, defaults)
        for texture in texture_fields:
            texture.layout = texture_layouts.get(texture.name)
        try:
            _pybevy.pbr.ShaderMaterial._validate_shader_def_names([field.def_name for field in bool_fields])
        except TypeError as error:
            raise TypeError(f"Material {cls.__name__}: {error}") from error
        extended_bindings = bool(storage_fields or texture_layouts or sampler_fields)
        for field in storage_fields:
            if field.name in {
                f"{texture.name}_sampler"
                for texture in texture_fields
                if texture.layout is None or texture.layout.sampler_type is not None
            }:
                raise TypeError(
                    f"StorageBinding field '{field.name}' conflicts with a generated sampler"
                )
            if defaults.get(field.name) is not None:
                raise TypeError(
                    f"StorageBinding field '{field.name}' must default to None; pass its handle to the material instance"
                )
        _validate_binding_names(
            cls.__name__, layout, texture_fields,
            (field.name for field in storage_fields),
            (field.name for field in sampler_fields),
        )
        native_type = StorageShaderMaterial if extended_bindings else ShaderMaterial
        layout_signature: _MaterialLayoutSignature = (
            tuple((field.name, field.layout.sampler_type) for field in sampler_fields),
            tuple(
                (
                    field.name,
                    field.python_type,
                    field.offset,
                    field.size,
                    field.num_floats,
                )
                for field in layout
            ),
            tuple(
                (field.name, field.bit_index, field.def_name) for field in bool_fields
            ),
            tuple(
                (field.name, field.slot_index, field.layout) for field in texture_fields
            ),
            tuple(
                (
                    field.name,
                    field.binding,
                    field.layout.wgsl_type,
                    field.layout.declarations,
                    field.layout.stride,
                )
                for field in storage_fields
            ),
        )
        field_map = {f.name: f for f in layout}
        bool_field_map = {bf.name: bf for bf in bool_fields}
        texture_field_map = {tf.name: tf for tf in texture_fields}

        # Generate WGSL binding declarations (injected in-memory, not written to disk)
        wgsl = _generate_wgsl(cls.__name__, layout, texture_fields)
        if storage_fields:
            wgsl += "\n" + "\n".join(
                dict.fromkeys(field.layout.declarations for field in storage_fields)
            )
        for field in storage_fields:
            wgsl += (
                f"\n@group(#{{MATERIAL_BIND_GROUP}}) @binding({field.binding}) "
                f"var<storage, read> {field.name}: array<{field.layout.wgsl_type}>;\n"
            )

        for slot, sampler_field in enumerate(sampler_fields):
            wgsl += f"\n@group(#{{MATERIAL_BIND_GROUP}}) @binding({109 + len(storage_fields) + slot}) var {sampler_field.name}: {sampler_field.layout.wgsl_type};\n"

        try:
            _pybevy.pbr.ShaderMaterial._validate_bindings(wgsl)
        except TypeError as error:
            resources = ", ".join(field.name for field in storage_fields)
            raise TypeError(f"Material {qualified_name}, storage fields [{resources}]: {error}") from error

        # Compute default shader_defs bitmask from bool defaults
        default_shader_defs: int = 0
        for bf in bool_fields:
            if bf.default:
                default_shader_defs |= 1 << bf.bit_index
        shader_def_names = [bf.def_name for bf in bool_fields]

        material_type_id = _logical_material_type_id(
            qualified_name,
            layout_signature,
        )
        # Store metadata on the class
        _cls._material_binding_declarations = {
            name: (declared_types[name], declared_defaults.get(name, _SENTINEL))
            for name in concise_bindings
        }
        _cls._material_layout = layout
        _cls._material_field_map = field_map
        _cls._material_bool_fields = bool_fields
        _cls._material_bool_field_map = bool_field_map
        _cls._material_texture_fields = texture_fields
        _cls._material_texture_field_map = texture_field_map
        _cls._material_sampler_defaults = sampler_defaults
        _cls._material_storage_fields = storage_fields
        _cls._material_fragment_shader = fragment_shader
        _cls._material_vertex_shader = vertex_shader
        _cls._material_wgsl = wgsl
        _cls._material_shader_def_names = shader_def_names

        # Create the __init__ that produces a ShaderMaterial
        def __init__(
            self: Any,  # noqa: ANN401
            *,
            base: StandardMaterial | None = None,
            **kwargs: Any,  # noqa: ANN401
        ) -> None:
            actual = type(self)
            if actual is not cls and "__pybevy_logical_type_id__" not in actual.__dict__:
                raise TypeError(f"Material subclass '{actual.__name__}' must be decorated with @material")
            if base is not None and not isinstance(base, StandardMaterial):
                raise TypeError("Material base must be a StandardMaterial or None")
            missing = [name for name in required_bindings if name not in kwargs]
            if missing:
                kind = "material" if any(declared_types[name] in (TextureBinding, SamplerBinding) for name in missing) else "storage"
                raise TypeError(f"Missing required {kind} fields: {', '.join(missing)}")
            if base is None:
                base = copy(base_template)

            # Start with defaults, override with kwargs
            data: list[float | int] = [0.0] * 256
            self._shader_mat = None  # Not in borrowed mode

            for field in layout:
                fallback = 0 if field.python_type is int and field.default is None else field.default
                value = kwargs.pop(field.name, fallback)
                if value is None and field.python_type is not int:
                    continue
                float_idx = field.offset // 4
                try:
                    _pack_value(data, float_idx, value, field.python_type)
                except (TypeError, ValueError, AttributeError, OverflowError) as error:
                    raise type(error)(f"Material {cls.__name__}.{field.name}: {error}") from error

            # Process bool fields -> shader defs bitmask
            shader_defs = default_shader_defs
            for bf in bool_fields:
                value = kwargs.pop(bf.name, None)
                if value is not None:
                    if value:
                        shader_defs |= 1 << bf.bit_index
                    else:
                        shader_defs &= ~(1 << bf.bit_index)

            # Process texture fields -> list of handles (or None)
            textures: list[Handle[Image] | None] = [None] * _MAX_TEXTURE_SLOTS
            for tf in texture_fields:
                value = kwargs.pop(tf.name, None)
                if extended_bindings:
                    _pybevy.pbr.StorageShaderMaterial._validate_texture_handle(value)
                if value is not None:
                    textures[tf.slot_index] = value

            self._samplers = [
                _copy_sampler(kwargs.pop(field.name, sampler_defaults[field.name]))
                for field in sampler_fields
            ]
            self._storage_buffers = []
            for field in storage_fields:
                value = kwargs.pop(field.name, None)
                _pybevy.pbr.StorageShaderMaterial._validate_storage_handle(value)
                self._storage_buffers.append(value)

            if kwargs:
                unknown = sorted(kwargs)
                valid = [f.name for f in layout]
                valid += [bf.name for bf in bool_fields]
                valid += [tf.name for tf in texture_fields]
                valid += [sf.name for sf in storage_fields]
                valid += [sf.name for sf in sampler_fields]
                message = f"Unknown material fields: {', '.join(unknown)}"
                hint = _pybevy._keyword_hint(cls.__name__, unknown[0], valid)
                if hint:
                    message = f"{message}. {hint}"
                raise TypeError(message)

            self._data = data
            self._base = base
            self._shader_defs = shader_defs
            self._textures = textures

        def _base_getter(self: _BaseInstance) -> StandardMaterial:
            native = self._shader_mat
            if native is not None:
                return native.base
            return cast(StandardMaterial, _pybevy.pbr.ShaderMaterial._read_only_base(self._base))

        def _base_setter(self: _BaseInstance, value: StandardMaterial) -> None:
            if not isinstance(value, StandardMaterial):
                raise TypeError("Material base must be a StandardMaterial")
            native = self._shader_mat
            if native is not None:
                native.base = value
            else:
                object.__setattr__(self, "_base", copy(value))

        public_fields = frozenset(
            field.name for field in (*layout, *bool_fields, *texture_fields, *storage_fields, *sampler_fields)
        ) | {"base"}
        internal_fields = frozenset({
            "_shader_mat", "_base", "_data", "_shader_defs", "_textures", "_storage_buffers", "_samplers",
        })
        base_fields = frozenset(_signature(StandardMaterial).parameters)

        def __setattr__(self: object, name: str, value: object) -> None:
            if name in public_fields or name in internal_fields:
                object.__setattr__(self, name, value)
                return
            message = f"Material {type(self).__name__} has no field '{name}'"
            if name in base_fields:
                message += f"; '{name}' is a base setting, use mat.base.{name}"
            else:
                hint = _pybevy._keyword_hint(type(self).__name__, name, sorted(public_fields))
                if hint:
                    message += f". {hint}"
            raise AttributeError(message)

        _cls.base = property(_base_getter, _base_setter)
        _cls.__setattr__ = __setattr__

        # Capture shader paths and WGSL for closure use in to_shader_material
        _frag_shader = fragment_shader
        _vert_shader = vertex_shader
        _wgsl = wgsl

        def to_shader_material(self: Any) -> ShaderMaterial | StorageShaderMaterial:  # noqa: ANN401
            """Convert to the underlying ShaderMaterial for adding to Assets."""
            if self._shader_mat is not None:
                return copy(self._shader_mat)
            tex_list = (
                self._textures if any(t is not None for t in self._textures) else None
            )
            integer_slots = {field.offset // 4 for field in layout if field.python_type is int}
            factory = StorageShaderMaterial if extended_bindings else ShaderMaterial
            shader_material = factory(
                base=self._base,
                fragment_shader=_frag_shader,
                vertex_shader=_vert_shader,
                data=[0.0 if index in integer_slots else value for index, value in enumerate(self._data)],
                shader_defs=self._shader_defs,
                shader_def_names=shader_def_names if shader_def_names else None,
                textures=tex_list,
                bindings_wgsl=_wgsl,
                prepass_vertex_shader=prepass_vertex_shader,
                prepass_fragment_shader=prepass_fragment_shader,
                deferred_vertex_shader=deferred_vertex_shader,
                deferred_fragment_shader=deferred_fragment_shader,
                **(
                    {
                        "storage_buffers": self._storage_buffers,
                        "samplers": [
                            (field.layout.sampler_type, self._samplers[slot])
                            for slot, field in enumerate(sampler_fields)
                        ],
                        "texture_layouts": [
                            None
                            if field.layout is None
                            else (
                                field.layout.dimension,
                                field.layout.sample_type,
                                field.layout.filterable,
                                field.layout.sampler_type,
                            )
                            for field in texture_fields
                        ],
                        "storage_layouts": [
                            (field.layout.wgsl_type, field.layout.declarations)
                            for field in storage_fields
                        ],
                    }
                    if extended_bindings
                    else {}
                ),
            )
            for index in integer_slots:
                cast(Any, shader_material)._set_data_int(index, self._data[index])
            cast(Any, shader_material)._set_logical_type_id(material_type_id)
            return shader_material

        def _from_borrowed(
            klass: type,
            shader_mat: ShaderMaterial | StorageShaderMaterial | None,
        ) -> Any:  # noqa: ANN401
            if shader_mat is None:
                return None
            inst: Any = _Material.__new__(klass)
            inst._shader_mat = shader_mat
            inst._data = None
            inst._base = None
            inst._shader_defs = None  # read/write through shader_mat
            return inst

        def __copy__(self: Any) -> Any:  # noqa: ANN401
            if self._shader_mat is not None:
                return _from_borrowed(type(self), copy(self._shader_mat))
            values = {
                field.name: getattr(self, field.name)
                for field in (*layout, *bool_fields, *texture_fields, *storage_fields, *sampler_fields)
            }
            return type(self)(base=copy(self._base), **values)

        def __deepcopy__(self: Any, memo: dict[int, object]) -> Any:  # noqa: ANN401
            result = __copy__(self)
            memo[id(self)] = result
            return result

        def from_ref(
            klass: type,
            shader_mat: ShaderMaterial | StorageShaderMaterial | None,
        ) -> Any:  # noqa: ANN401
            """Wrap a read-only borrowed ShaderMaterial."""
            return _from_borrowed(klass, shader_mat)

        def from_mut(
            klass: type,
            shader_mat: ShaderMaterial | StorageShaderMaterial | None,
        ) -> Any:  # noqa: ANN401
            """Wrap a mutable borrowed ShaderMaterial."""
            return _from_borrowed(klass, shader_mat)

        # Add property accessors for uniform fields
        for field in layout:
            _name = field.name
            _float_idx = field.offset // 4
            _typ = field.python_type
            _nf = field.num_floats

            def _getter(
                self: Any,  # noqa: ANN401
                _fi: int = _float_idx,
                _t: type = _typ,
                _n: int = _nf,
            ) -> Any:  # noqa: ANN401
                if _t in _PACKED_VECTOR_TYPES:
                    if self._shader_mat is not None:
                        return self._shader_mat._uniform_field(_fi, _t)
                    value = _read_value(self._data, _fi, _t)
                    return _pybevy.pbr.ShaderMaterial._read_only_uniform(value)
                if self._shader_mat is not None:
                    if _t is int:
                        return self._shader_mat._get_data_int(_fi)
                    floats = self._shader_mat.get_data_floats(_fi, _n)
                    return _read_value(floats, 0, _t)
                return _read_value(self._data, _fi, _t)

            def _setter(
                self: Any,  # noqa: ANN401
                value: Any,  # noqa: ANN401
                _fi: int = _float_idx,
                _t: type = _typ,
                _n: int = _nf,
                _label: str = f"{cls.__name__}.{field.name}",
            ) -> None:
                temp: list[float | int] = [0.0] * _n
                try:
                    _pack_value(temp, 0, value, _t)
                except (TypeError, ValueError, AttributeError, OverflowError) as error:
                    raise type(error)(f"Material {_label}: {error}") from error
                if self._shader_mat is not None:
                    if _t is int:
                        self._shader_mat._set_data_int(_fi, temp[0])
                    else:
                        self._shader_mat.set_data_floats(_fi, temp)
                else:
                    self._data[_fi:_fi + _n] = temp

            setattr(cls, _name, property(_getter, _setter))

        # Add property accessors for bool fields (shader defs)
        for bf in bool_fields:
            _bit = bf.bit_index

            def _bool_getter(self: Any, bit: int = _bit) -> bool:  # noqa: ANN401
                if self._shader_mat is not None:
                    return bool(self._shader_mat.get_shader_defs() & (1 << bit))
                return bool(self._shader_defs & (1 << bit))

            def _bool_setter(self: Any, value: bool, bit: int = _bit) -> None:  # noqa: ANN401
                if self._shader_mat is not None:
                    defs = self._shader_mat.get_shader_defs()
                    if value:
                        defs |= 1 << bit
                    else:
                        defs &= ~(1 << bit)
                    self._shader_mat.set_shader_defs(defs)
                else:
                    if value:
                        self._shader_defs |= 1 << bit
                    else:
                        self._shader_defs &= ~(1 << bit)

            setattr(cls, bf.name, property(_bool_getter, _bool_setter))

        # Add property accessors for texture fields
        for tf in texture_fields:
            _slot = tf.slot_index
            if tf.name in concise_bindings:
                texture_descriptor = copy(tf.layout)
                object.__setattr__(texture_descriptor, "_slot", _slot)
                setattr(cls, tf.name, texture_descriptor)
                continue

            def _tex_getter(self: Any, slot: int = _slot) -> Handle[Image] | None:  # noqa: ANN401
                if self._shader_mat is not None:
                    return self._shader_mat.get_texture(slot)  # type: ignore[no-any-return]
                return self._textures[slot]  # type: ignore[no-any-return]

            def _tex_setter(
                self: Any,  # noqa: ANN401
                value: Handle[Image] | None,
                slot: int = _slot,
            ) -> None:
                if self._shader_mat is not None:
                    if value is not None:
                        self._shader_mat.set_texture(slot, value)
                    else:
                        self._shader_mat.clear_texture(slot)
                else:
                    if extended_bindings:
                        _pybevy.pbr.StorageShaderMaterial._validate_texture_handle(value)
                    self._textures[slot] = value

            setattr(cls, tf.name, property(_tex_getter, _tex_setter))

        for slot, storage_field in enumerate(storage_fields):
            if storage_field.name in concise_bindings:
                descriptor = copy(storage_field.layout)
                object.__setattr__(descriptor, "_slot", slot)
                setattr(cls, storage_field.name, descriptor)
                continue

            def _storage_getter(
                self: _StorageInstance, _slot: int = slot
            ) -> Handle[ShaderBuffer] | None:
                if self._shader_mat is not None:
                    return self._shader_mat.get_storage_buffer_at(_slot)
                return self._storage_buffers[_slot]

            def _storage_setter(
                self: _StorageInstance,
                value: Handle[ShaderBuffer] | None,
                _slot: int = slot,
            ) -> None:
                if self._shader_mat is not None:
                    self._shader_mat.set_storage_buffer_at(_slot, value)
                else:
                    _pybevy.pbr.StorageShaderMaterial._validate_storage_handle(value)
                    self._storage_buffers[_slot] = value

            setattr(cls, storage_field.name, property(_storage_getter, _storage_setter))

        for slot, sampler_field in enumerate(sampler_fields):
            if sampler_field.name in concise_bindings:
                sampler_descriptor = copy(sampler_field.layout)
                object.__setattr__(sampler_descriptor, "_slot", slot)
                setattr(cls, sampler_field.name, sampler_descriptor)
                continue
            def _sampler_getter(
                self: _SamplerInstance, _slot: int = slot
            ) -> ImageSamplerDescriptor:
                if self._shader_mat is not None:
                    return self._shader_mat.get_sampler(_slot)
                return self._samplers[_slot]

            def _sampler_setter(
                self: _SamplerInstance, value: ImageSamplerDescriptor, _slot: int = slot
            ) -> None:
                if self._shader_mat is not None:
                    self._shader_mat.set_sampler(_slot, value)
                else:
                    self._samplers[_slot] = _copy_sampler(value)

            setattr(cls, sampler_field.name, property(_sampler_getter, _sampler_setter))

        _cls.__copy__ = __copy__
        _cls.__deepcopy__ = __deepcopy__
        __init__.__qualname__ = f"{cls.__qualname__}.__init__"
        __init__.__module__ = cls.__module__
        _cls.__init__ = __init__
        _cls.to_shader_material = to_shader_material
        _cls.from_ref = classmethod(from_ref)
        _cls.from_mut = classmethod(from_mut)

        # Asset metadata: enables Assets[HologramMaterial], MeshMaterial3d[HologramMaterial]
        _cls.__pybevy_asset_type__ = native_type
        _cls.__pybevy_component_type__ = (
            MeshMaterial3dStorageShader if extended_bindings else MeshMaterial3dShader
        )
        _cls.__pybevy_logical_type_id__ = material_type_id

        return cls

    return decorator


# Rejected legacy keywords are diagnostics, not accepted API parameters.
material.__signature__ = _signature(material).replace(parameters=[parameter for name, parameter in _signature(material).parameters.items() if name != "removed_settings"])
