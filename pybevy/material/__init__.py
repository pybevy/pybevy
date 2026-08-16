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
from threading import Lock
from typing import Any, TypeVar, cast, get_type_hints

from .. import _pybevy  # type: ignore
from ..color import LinearRgba
from ..image import Image
from ..math import Vec2, Vec3, Vec4
from ..pbr import MeshMaterial3dShader, ShaderMaterial, StandardMaterial
from ._layout import (
    _MAX_TEXTURE_SLOTS,
    _compute_layout,
    _generate_wgsl,
)

_Material = _pybevy.pbr.Material  # type: ignore
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
    data: list[float], float_index: int, value: Any, typ: type,  # noqa: ANN401
) -> None:
    """Pack a Python value into the flat float buffer at the given index."""
    if typ is float:
        data[float_index] = float(value)
    elif typ is int:
        # Pack int value into the float slot - stored as f32 in the uniform buffer
        data[float_index] = float(value)
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
    data: list[float], float_index: int, typ: type,
) -> Any:  # noqa: ANN401
    """Read a Python value from the flat float buffer."""
    if typ is float:
        return data[float_index]
    if typ is int:
        return int(data[float_index])
    if typ is Vec2:
        return Vec2(data[float_index], data[float_index + 1])
    if typ is Vec3:
        return Vec3(data[float_index], data[float_index + 1], data[float_index + 2])
    if typ is Vec4:
        return Vec4(data[float_index], data[float_index + 1],
                    data[float_index + 2], data[float_index + 3])
    if typ is LinearRgba:
        return LinearRgba(data[float_index], data[float_index + 1],
                          data[float_index + 2], data[float_index + 3])
    return None


_SENTINEL = object()



def _is_native_material(cls: type) -> bool:
    """A material class defined in Rust rather than by @material."""
    return cls.__module__ == _Material.__module__


def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
    *,
    alpha_mode: AlphaMode | None = None,
    double_sided: bool | None = None,
    cull_mode: object = _SENTINEL,
    unlit: bool | None = None,
    depth_bias: float | None = None,
) -> Callable[[type[MaterialT]], type[MaterialT]]:
    """Decorator to define a custom shader material.

    Generates WGSL struct + binding declaration and handles data packing.

    Args:
        fragment_shader: Asset path to custom fragment shader (relative to assets/ directory).
        vertex_shader: Asset path to custom vertex shader (relative to assets/ directory).
        alpha_mode: Default AlphaMode for the base StandardMaterial (e.g., AlphaMode.Blend()).
        double_sided: Whether the material renders on both sides.
        cull_mode: Face culling mode. None = no culling, Face.Back = back-face culling (default).
        unlit: Whether the material ignores lighting.
        depth_bias: Depth bias to prevent z-fighting.

    Example:
        @material(
            fragment_shader="shaders/hologram.wgsl",
            alpha_mode=AlphaMode.Blend(),
            double_sided=True,
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
    # Collect base material overrides from decorator params
    base_overrides: dict[str, Any] = {}
    if alpha_mode is not None:
        base_overrides["alpha_mode"] = alpha_mode
    if double_sided is not None:
        base_overrides["double_sided"] = double_sided
    if cull_mode is not _SENTINEL:
        base_overrides["cull_mode"] = cull_mode
    if unlit is not None:
        base_overrides["unlit"] = unlit
    if depth_bias is not None:
        base_overrides["depth_bias"] = depth_bias

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
        hints = get_type_hints(cls)
        defaults: dict[str, Any] = {}
        for name in hints:
            if hasattr(cls, name):
                defaults[name] = getattr(cls, name)

        # Compute layout (separates uniform, bool, and texture fields)
        layout, bool_fields, texture_fields = _compute_layout(hints, defaults)
        layout_signature: _MaterialLayoutSignature = (
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
                (field.name, field.bit_index, field.def_name)
                for field in bool_fields
            ),
            tuple(
                (field.name, field.slot_index)
                for field in texture_fields
            ),
        )
        material_type_id = _logical_material_type_id(
            qualified_name,
            layout_signature,
        )
        field_map = {f.name: f for f in layout}
        bool_field_map = {bf.name: bf for bf in bool_fields}
        texture_field_map = {tf.name: tf for tf in texture_fields}

        # Generate WGSL binding declarations (injected in-memory, not written to disk)
        wgsl = _generate_wgsl(cls.__name__, layout, texture_fields)

        # Compute default shader_defs bitmask from bool defaults
        default_shader_defs: int = 0
        for bf in bool_fields:
            if bf.default:
                default_shader_defs |= (1 << bf.bit_index)
        shader_def_names = [bf.def_name for bf in bool_fields]

        # Store metadata on the class
        _cls._material_layout = layout
        _cls._material_field_map = field_map
        _cls._material_bool_fields = bool_fields
        _cls._material_bool_field_map = bool_field_map
        _cls._material_texture_fields = texture_fields
        _cls._material_texture_field_map = texture_field_map
        _cls._material_fragment_shader = fragment_shader
        _cls._material_vertex_shader = vertex_shader
        _cls._material_wgsl = wgsl
        _cls._material_shader_def_names = shader_def_names

        # Capture base overrides for closure use
        _base_overrides = base_overrides

        # Create the __init__ that produces a ShaderMaterial
        def __init__(self: Any, base: StandardMaterial | None = None, **kwargs: Any) -> None:  # noqa: ANN401
            # Auto-construct base StandardMaterial with decorator defaults if not provided
            if base is None:
                base = StandardMaterial(**_base_overrides) if _base_overrides else StandardMaterial()

            # Start with defaults, override with kwargs
            data = [0.0] * 256
            self._shader_mat = None  # Not in borrowed mode

            for field in layout:
                value = kwargs.pop(field.name, field.default)
                if value is None:
                    continue
                float_idx = field.offset // 4
                _pack_value(data, float_idx, value, field.python_type)

            # Process bool fields -> shader defs bitmask
            shader_defs = default_shader_defs
            for bf in bool_fields:
                value = kwargs.pop(bf.name, None)
                if value is not None:
                    if value:
                        shader_defs |= (1 << bf.bit_index)
                    else:
                        shader_defs &= ~(1 << bf.bit_index)

            # Process texture fields -> list of handles (or None)
            textures: list[Image | None] = [None] * _MAX_TEXTURE_SLOTS
            for tf in texture_fields:
                value = kwargs.pop(tf.name, None)
                if value is not None:
                    textures[tf.slot_index] = value

            if kwargs:
                unknown = ", ".join(kwargs.keys())
                raise TypeError(f"Unknown material fields: {unknown}")

            self._data = data
            self._base = base
            self._shader_defs = shader_defs
            self._textures = textures

        # Capture shader paths and WGSL for closure use in to_shader_material
        _frag_shader = fragment_shader
        _vert_shader = vertex_shader
        _wgsl = wgsl

        def to_shader_material(self: Any) -> ShaderMaterial:  # noqa: ANN401
            """Convert to the underlying ShaderMaterial for adding to Assets."""
            tex_list = self._textures if any(t is not None for t in self._textures) else None
            shader_material = ShaderMaterial(
                base=self._base,
                fragment_shader=_frag_shader,
                vertex_shader=_vert_shader,
                data=self._data,
                shader_defs=self._shader_defs,
                shader_def_names=shader_def_names if shader_def_names else None,
                textures=tex_list,
                bindings_wgsl=_wgsl,
            )
            cast(Any, shader_material)._set_logical_type_id(material_type_id)
            return shader_material

        def _from_borrowed(
            klass: type, shader_mat: ShaderMaterial | None,
        ) -> Any:  # noqa: ANN401
            if shader_mat is None:
                return None
            inst: Any = _Material.__new__(klass)
            inst._shader_mat = shader_mat
            inst._data = None
            inst._base = None
            inst._shader_defs = None  # read/write through shader_mat
            return inst

        def from_ref(
            klass: type, shader_mat: ShaderMaterial | None,
        ) -> Any:  # noqa: ANN401
            """Wrap a read-only borrowed ShaderMaterial."""
            return _from_borrowed(klass, shader_mat)

        def from_mut(
            klass: type, shader_mat: ShaderMaterial | None,
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
                self: Any, _fi: int = _float_idx, _t: type = _typ, _n: int = _nf,  # noqa: ANN401
            ) -> Any:  # noqa: ANN401
                if self._shader_mat is not None:
                    floats = self._shader_mat.get_data_floats(_fi, _n)
                    return _read_value(floats, 0, _t)
                return _read_value(self._data, _fi, _t)

            def _setter(
                self: Any, value: Any, _fi: int = _float_idx, _t: type = _typ, _n: int = _nf,  # noqa: ANN401
            ) -> None:
                if self._shader_mat is not None:
                    temp = [0.0] * _n
                    _pack_value(temp, 0, value, _t)
                    self._shader_mat.set_data_floats(_fi, temp)
                else:
                    _pack_value(self._data, _fi, value, _t)

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
                        defs |= (1 << bit)
                    else:
                        defs &= ~(1 << bit)
                    self._shader_mat.set_shader_defs(defs)
                else:
                    if value:
                        self._shader_defs |= (1 << bit)
                    else:
                        self._shader_defs &= ~(1 << bit)

            setattr(cls, bf.name, property(_bool_getter, _bool_setter))

        # Add property accessors for texture fields
        for tf in texture_fields:
            _slot = tf.slot_index

            def _tex_getter(self: Any, slot: int = _slot) -> Image | None:  # noqa: ANN401
                if self._shader_mat is not None:
                    # Texture reads not supported in borrowed mode
                    return None
                return self._textures[slot]  # type: ignore[no-any-return]

            def _tex_setter(self: Any, value: Image | None, slot: int = _slot) -> None:  # noqa: ANN401
                if self._shader_mat is not None:
                    if value is not None:
                        self._shader_mat.set_texture(slot, value)
                    else:
                        self._shader_mat.clear_texture(slot)
                else:
                    self._textures[slot] = value

            setattr(cls, tf.name, property(_tex_getter, _tex_setter))

        _cls.__init__ = __init__
        _cls.to_shader_material = to_shader_material
        _cls.from_ref = classmethod(from_ref)
        _cls.from_mut = classmethod(from_mut)

        # Phase 5 metadata: enables Assets[HologramMaterial], MeshMaterial3d[HologramMaterial]
        _cls.__pybevy_asset_type__ = ShaderMaterial
        _cls.__pybevy_component_type__ = MeshMaterial3dShader
        _cls.__pybevy_logical_type_id__ = material_type_id

        return cls

    return decorator
