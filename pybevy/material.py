"""@material decorator for defining custom shader materials from Python.

Generates WGSL struct declarations and handles std140 packing automatically.

Example:
    @material(fragment_shader="shaders/lava.wgsl")
    class LavaMaterial:
        color: LinearRgba = LinearRgba(1.0, 0.3, 0.0, 1.0)
        crack_scale: float = 5.0
        speed: float = 2.0
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from typing import TYPE_CHECKING, Any, get_type_hints

if TYPE_CHECKING:
    from .render import AlphaMode

from .color import LinearRgba
from .image import Image
from .math import Vec2, Vec3, Vec4
from .pbr import MeshMaterial3dShader, ShaderMaterial, StandardMaterial

_MAX_TEXTURE_SLOTS = 4

# ── WGSL type mapping ────────────────────────────────────────────────
#
# Each entry: (wgsl_type, size_bytes, align_bytes, num_floats)

_TYPE_MAP: dict[type, tuple[str, int, int, int]] = {
    float: ("f32", 4, 4, 1),
    int: ("f32", 4, 4, 1),
    Vec2: ("vec2<f32>", 8, 8, 2),
    Vec3: ("vec3<f32>", 12, 16, 3),
    Vec4: ("vec4<f32>", 16, 16, 4),
    LinearRgba: ("vec4<f32>", 16, 16, 4),
}


def _align_up(offset: int, alignment: int) -> int:
    return (offset + alignment - 1) & ~(alignment - 1)


# ── Layout computation ───────────────────────────────────────────────

class _FieldLayout:
    __slots__ = ("default", "name", "num_floats", "offset", "python_type", "size", "wgsl_type")

    def __init__(
        self, name: str, python_type: type, wgsl_type: str,
        offset: int, size: int, num_floats: int, default: Any,  # noqa: ANN401
    ) -> None:
        self.name = name
        self.python_type = python_type
        self.wgsl_type = wgsl_type
        self.offset = offset
        self.size = size
        self.num_floats = num_floats
        self.default = default


class _BoolFieldInfo:
    """Tracks a bool field that maps to a shader def bit."""
    __slots__ = ("bit_index", "def_name", "default", "name")

    def __init__(self, name: str, bit_index: int, def_name: str, default: bool) -> None:
        self.name = name
        self.bit_index = bit_index
        self.def_name = def_name
        self.default = default


class _TextureFieldInfo:
    """Tracks an Image field that maps to a texture slot."""
    __slots__ = ("binding_sampler", "binding_texture", "name", "slot_index")

    def __init__(self, name: str, slot_index: int) -> None:
        self.name = name
        self.slot_index = slot_index
        # Binding numbers must match the Rust AsBindGroup attributes
        self.binding_texture = 101 + slot_index * 2
        self.binding_sampler = 102 + slot_index * 2


def _compute_layout(
    hints: Mapping[str, type], defaults: Mapping[str, Any],
) -> tuple[list[_FieldLayout], list[_BoolFieldInfo], list[_TextureFieldInfo]]:
    """Compute std140-compatible layout for material fields.

    Returns (uniform_fields, bool_fields, texture_fields).
    Bool fields become shader defs, Image fields become texture slots.
    """
    fields: list[_FieldLayout] = []
    bool_fields: list[_BoolFieldInfo] = []
    texture_fields: list[_TextureFieldInfo] = []
    offset = 0

    for name, typ in hints.items():
        if name.startswith("_"):
            continue

        # Bool fields → shader defs, not uniforms
        if typ is bool:
            if len(bool_fields) >= 32:
                raise TypeError(
                    f"Too many bool fields (max 32): '{name}' is the 33rd"
                )
            default = defaults.get(name, False)
            bool_fields.append(_BoolFieldInfo(
                name=name,
                bit_index=len(bool_fields),
                def_name=name.upper(),
                default=bool(default),
            ))
            continue

        # Image fields → texture slots
        if typ is Image:
            if len(texture_fields) >= _MAX_TEXTURE_SLOTS:
                raise TypeError(
                    f"Too many Image fields (max {_MAX_TEXTURE_SLOTS}): '{name}'"
                )
            texture_fields.append(_TextureFieldInfo(
                name=name,
                slot_index=len(texture_fields),
            ))
            continue

        if typ not in _TYPE_MAP:
            raise TypeError(
                f"Unsupported material field type '{typ.__name__}' for '{name}'. "
                f"Supported: float, int, bool, Image, Vec2, Vec3, Vec4, LinearRgba"
            )

        wgsl_type, size, align, num_floats = _TYPE_MAP[typ]
        offset = _align_up(offset, align)

        default = defaults.get(name)

        fields.append(_FieldLayout(
            name=name,
            python_type=typ,
            wgsl_type=wgsl_type,
            offset=offset,
            size=size,
            num_floats=num_floats,
            default=default,
        ))
        offset += size

    total = _align_up(offset, 16)  # struct alignment = max member alignment
    if total > 1024:
        raise TypeError(f"Material struct too large: {total} bytes (max 1024)")

    return fields, bool_fields, texture_fields


def _pack_value(
    data: list[float], float_index: int, value: Any, typ: type,  # noqa: ANN401
) -> None:
    """Pack a Python value into the flat float buffer at the given index."""
    if typ is float:
        data[float_index] = float(value)
    elif typ is int:
        # Pack int value into the float slot — stored as f32 in the uniform buffer
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


# ── WGSL generation ──────────────────────────────────────────────────

def _generate_wgsl(
    class_name: str,
    fields: list[_FieldLayout],
    texture_fields: list[_TextureFieldInfo] | None = None,
) -> str:
    """Generate WGSL struct + binding declarations.

    Includes ``#define_import_path`` so user shaders can::

        #import pybevy::gen::MyMaterial

    to get the struct and binding declarations without duplicating them.
    """
    import_path = f"pybevy::gen::{class_name}"
    lines = [
        "// Auto-generated by @material — do not edit",
        f"#define_import_path {import_path}",
    ]

    # Uniform struct (only if there are uniform fields)
    if fields:
        lines.append(f"struct {class_name} {{")
        for f in fields:
            lines.append(f"    {f.name}: {f.wgsl_type},")
        lines.append("}")
        lines.append("")
        # Bevy 0.18: material bind group is at group 3 (MATERIAL_BIND_GROUP_INDEX)
        lines.append(f"@group(3) @binding(100) var<uniform> material: {class_name};")
        lines.append("")

    # Texture/sampler declarations
    if texture_fields:
        for tf in texture_fields:
            lines.append(
                f"@group(3) @binding({tf.binding_texture}) "
                f"var {tf.name}: texture_2d<f32>;"
            )
            lines.append(
                f"@group(3) @binding({tf.binding_sampler}) "
                f"var {tf.name}_sampler: sampler;"
            )
        lines.append("")

    return "\n".join(lines)


# ── Decorator ────────────────────────────────────────────────────────

_SENTINEL = object()


def material(
    fragment_shader: str | None = None,
    vertex_shader: str | None = None,
    *,
    alpha_mode: AlphaMode | None = None,
    double_sided: bool | None = None,
    cull_mode: object = _SENTINEL,
    unlit: bool | None = None,
    depth_bias: float | None = None,
) -> Callable[[type], type]:
    """Decorator to define a custom shader material.

    Generates WGSL struct + binding declaration and handles data packing.

    Args:
        fragment_shader: Asset path to custom fragment shader (relative to assets/ directory).
        vertex_shader: Asset path to custom vertex shader (relative to assets/ directory).
        alpha_mode: Default AlphaMode for the base StandardMaterial (e.g., AlphaMode.BLEND).
        double_sided: Whether the material renders on both sides.
        cull_mode: Face culling mode. None = no culling, Face.Back = back-face culling (default).
        unlit: Whether the material ignores lighting.
        depth_bias: Depth bias to prevent z-fighting.

    Example:
        @material(
            fragment_shader="shaders/hologram.wgsl",
            alpha_mode=AlphaMode.BLEND,
            double_sided=True,
        )
        class HologramMaterial:
            opacity: float = 0.5
            scan_speed: float = 2.0

        # base StandardMaterial is auto-constructed with decorator defaults:
        mat = HologramMaterial(opacity=0.8)

        # Or override base explicitly:
        mat = HologramMaterial(
            base=StandardMaterial(alpha_mode=AlphaMode.ADD),
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

    def decorator(cls: type) -> type:
        # Use Any-typed alias to allow dynamic attribute assignment on cls
        _cls: Any = cls

        # Collect type hints and defaults
        hints = get_type_hints(cls)
        defaults: dict[str, Any] = {}
        for name in hints:
            if hasattr(cls, name):
                defaults[name] = getattr(cls, name)

        # Compute layout (separates uniform, bool, and texture fields)
        layout, bool_fields, texture_fields = _compute_layout(hints, defaults)
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
            return ShaderMaterial(
                base=self._base,
                fragment_shader=_frag_shader,
                vertex_shader=_vert_shader,
                data=self._data,
                shader_defs=self._shader_defs,
                shader_def_names=shader_def_names if shader_def_names else None,
                textures=tex_list,
                bindings_wgsl=_wgsl,
            )

        def from_mut(
            klass: type, shader_mat: ShaderMaterial | None,
        ) -> Any:  # noqa: ANN401
            """Wrap a mutable ShaderMaterial for field-level access at runtime.

            Use with materials.get_mut(handle) to modify uniforms at 60fps:

                mat = MyMaterial.from_mut(materials.get_mut(handle))
                mat.speed = 5.0  # writes directly to GPU buffer
            """
            if shader_mat is None:
                return None
            inst: Any = object.__new__(klass)
            inst._shader_mat = shader_mat
            inst._data = None
            inst._base = None
            inst._shader_defs = None  # read/write through shader_mat
            return inst

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
        _cls.from_mut = classmethod(from_mut)

        # Phase 5 metadata: enables Assets[HologramMaterial], MeshMaterial3d[HologramMaterial]
        _cls.__pybevy_asset_type__ = ShaderMaterial
        _cls.__pybevy_material_component__ = MeshMaterial3dShader

        return cls

    return decorator
