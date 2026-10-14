# Custom Shaders with `@material`

`@material` generates WGSL bindings, packs GPU data, and integrates Python materials with Bevy PBR.
Uniform and texture field names and the material class name become WGSL
identifiers. WGSL keywords and reserved words such as `loop`, `module`, and
`static` are rejected when the class is decorated; use a trailing underscore
such as `loop_`. Bool fields become shader definitions rather than WGSL fields.
Texture and storage names must also avoid generated global names: a texture
`image` creates `image_sampler`, and uniform fields create a struct named after
the material class plus a global `material`. Collisions raise `TypeError` at
decoration time. Uniform struct members have their own scope and may share
those names.

## Quick Start

```python
from pybevy.prelude import *
from pybevy.color import LinearRgba
from pybevy.pbr import ShaderMaterialPlugin

@material(fragment_shader="shaders/glow.wgsl")  # relative to assets/ directory
class GlowMaterial(Material):
    color: LinearRgba = LinearRgba(0.0, 1.0, 0.5, 1.0)
    intensity: float = 1.5

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[GlowMaterial]],
) -> None:
    commands.spawn(
        Mesh3d(meshes.add(Sphere(1.0))),
        MeshMaterial3d[GlowMaterial](materials.add(GlowMaterial(
            base=StandardMaterial(base_color=Color.BLACK),
            color=LinearRgba(0.0, 1.0, 0.5, 1.0),
            intensity=2.0,
        ))),
        Transform.from_xyz(0, 1, 0),
    )
    commands.spawn(Camera3d(), Transform.from_xyz(0, 2, 5).looking_at(Vec3.ZERO, Vec3.Y))

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(ShaderMaterialPlugin())  # required
        .add_systems(Startup, setup)
    )

if __name__ == "__main__":
    main().run()
```

**Shader paths** are relative to the `assets/` directory (Bevy's default asset root).
For a file at `assets/shaders/glow.wgsl`, use `fragment_shader="shaders/glow.wgsl"`.
Malformed paths, such as a trailing `#` with no label, raise `ValueError` when
the material is converted to `ShaderMaterial`, before consuming its base.
All custom material shader paths must name `.wgsl` assets. The separate
`Shader`/`Source` GLSL and WESL constructors do not establish support for those
languages in this material API.

## Base Material Properties

The `@material` decorator accepts optional parameters to configure the underlying `StandardMaterial`:

```python
@material(
    fragment_shader="shaders/glass.wgsl",
    alpha_mode=AlphaMode.Blend(),  # transparency blending
    double_sided=True,              # render both faces
    cull_mode=None,                 # disable face culling
    unlit=True,                     # ignore lighting
    depth_bias=1.0,                 # prevent z-fighting
)
class GlassMaterial(Material):
    opacity: float = 0.3
    tint: LinearRgba = LinearRgba(0.8, 0.9, 1.0, 1.0)

# No need to pass base= explicitly:
mat = GlassMaterial(opacity=0.5)
```

These parameters provide defaults for the base material. Passing `base=` replaces
those defaults, so include its desired `alpha_mode` and other settings explicitly.
Assigning `mat.alpha_mode` or `mat.cull_mode` creates an ordinary Python attribute;
it does not edit the native base or change rendering.

`AlphaMode.Mask(cutoff)` selects the alpha-mask pass, but custom fragment WGSL
must perform the discard. In the PBR fragment pattern below, call
`alpha_discard(pbr_input.material, output_color)` before returning the color.
The cutoff is a per-material float uniform; materials can share a shader file
with different thresholds. A fragment that simply returns `vec4(...)` does not
apply the mask threshold. Match discards in custom prepass/deferred hooks too.

Converting an `@material` instance to `ShaderMaterial` consumes its base; use a fresh `StandardMaterial` for each conversion:

```python
mat = GlowMaterial(
    base=StandardMaterial(base_color=Color.BLACK),
    color=LinearRgba(0.0, 1.0, 0.5, 1.0),
)
```

## Supported Field Types

Bindings use the same declaration pattern:

<!-- pybevy-snippet: typecheck -->
```python
from pybevy.image import ImageSamplerDescriptor
from pybevy.material import SamplerBinding, StorageBinding, TextureBinding, material
from pybevy.math import Vec4
from pybevy.pbr import Material

@material(fragment_shader="shaders/surface.wgsl")
class Surface(Material):
    colors: StorageBinding[Vec4]                 # required ShaderBuffer handle
    image: TextureBinding                       # required Image handle
    sharp: SamplerBinding = SamplerBinding("non_filtering")
    soft: SamplerBinding = SamplerBinding(default=ImageSamplerDescriptor.linear())
```

Without a default, each binding requires a constructor argument. `StorageBinding()` and
`TextureBinding()` default to `None`; `SamplerBinding()` supplies a fresh nearest-filtered
descriptor. Use `TextureBinding(dimension="cube")`, `TextureBinding(sampler=None)`, or other
options to configure a texture declaration. Class annotations describe bindings;
instance reads, assignments and constructor arguments use `Handle[ShaderBuffer]
| None`, `Handle[Image] | None`, or `ImageSamplerDescriptor`. Both mypy and
Pyright/Pylance understand these types, including inherited fields.

These declarations reuse the explicit `Annotated` forms below. Keep those for
advanced metadata such as custom WGSL storage structs. Binding numbers and
shader syntax do not change. Explicit texture bindings, storage and independent
samplers use the same `ShaderMaterialPlugin()`.

| Python type    | WGSL type      | Notes                          |
|----------------|----------------|--------------------------------|
| `float`        | `f32`          | 4 bytes                        |
| `int`          | `f32`          | 4 bytes, cast to float         |
| `Vec2`         | `vec2<f32>`    | 8 bytes, 8-byte aligned        |
| `Vec3`         | `vec3<f32>`    | 12 bytes, 16-byte aligned      |
| `Vec4`         | `vec4<f32>`    | 16 bytes, 16-byte aligned      |
| `LinearRgba`   | `vec4<f32>`    | 16 bytes, 16-byte aligned      |
| `bool`         | shader def     | `#ifdef FIELD_NAME` in WGSL    |
| `Handle[Image] \| None` | texture slot | `texture_2d<f32>` + `sampler` |
| `Annotated[Handle[Image] \| None, TextureBinding(...)]` | sampled texture | cube/3D/array dimensions and sample controls |
| `StorageBinding[Vec4]` | read-only storage | concise typed array declaration |
| `TextureBinding` | sampled texture | concise Image-handle declaration |
| `SamplerBinding` | independent sampler | concise ImageSamplerDescriptor declaration |
| `Annotated[Handle[ShaderBuffer] \| None, StorageBinding()]` | read-only storage | typed read-only arrays |

Use `StorageBinding[T]` or explicit `StorageBinding` metadata for buffers. A bare
`Handle[ShaderBuffer]` cannot declare storage: `Handle[T]` currently erases `T`
at runtime, so an unmarked handle annotation follows the texture-field path.

WGSL integer-vector spellings are `vec2i` / `vec2<i32>` and `vec2u` /
`vec2<u32>`, with the analogous forms for three and four components. GLSL names
such as `ivec2` and `uvec2` are not WGSL aliases. WGSL struct members use commas,
including inside `StorageBinding(declarations=...)`.

## How It Works

The decorator:
1. Inspects type hints and computes std140 layout
2. Builds the matching material binding layout for Bevy
3. Creates `__init__` that packs field values into a 256-float buffer
4. Creates property accessors for runtime mutation
5. Registers metadata for `Assets[YourMaterial]` and `MeshMaterial3d[YourMaterial]`

Each decorated class keeps a distinct logical identity even though the engine stores
them as `ShaderMaterial` or, for classes with storage or explicit `TextureBinding` metadata,
`StorageShaderMaterial`. `Assets[GlowMaterial]` rejects handles from another
custom material class, and `Query[MeshMaterial3d[GlowMaterial]]` only matches entities
using `GlowMaterial`. In optional query data, a different custom material class is
treated as absent and yields `None`; it does not remove the entity from the query.

A decorated material may inherit from another decorated material. Its fields
keep the parent's defaults unless the child explicitly overrides them. Each
decorated class still has its own shader paths and logical identity.

Partial reload preserves a material's identity when its qualified name and field layout
are unchanged. Changing field names, types, or ordering allocates a fresh identity, as
does a full reload, so old handles cannot be interpreted through a different layout.

`Assets[T]` retains a decorated material class while its parameter or runtime
collection is alive. Released holders allow cyclic GC to reclaim discarded
scenes.

`Assets[T]` for a decorated material yields the decorated class from `get()`,
`get_mut()`, iteration, and `remove()`. Iterated materials are read-only and
expire with the system; removed materials are independent owned values.

Custom material collections with the same native material type share one underlying
resource: `Assets[ShaderMaterial]` for ordinary materials and
`Assets[StorageShaderMaterial]` for storage/explicit-texture materials. A system cannot request
two mutable collections backed by the same resource, such as two storage
classes; split those mutations across ordered systems. An ordinary collection
and a storage collection have distinct native resource identities.

## Raw Shader Buffers

Construct `ShaderBuffer` from `bytes` or a bounded `uint8` `Array`, or reserve
space with `ShaderBuffer.with_size()`. Mutate it through `Assets[ShaderBuffer]`;
`resize_in_place()` preserves GPU data when no CPU copy is available.
Use `with buffer.data() as data` for read-only CPU bytes, `data_mut()` for
scoped writes, and `data_copy()` for an independent copy. Views block conflicting
asset mutations and expire on context exit or system completion. `data_len()`
returns the current CPU byte count or `None`, not GPU allocation size.
Render extraction may consume CPU upload data; its retention depends on the
asset path. These accessors never read back GPU writes. `set_data()` takes already encoded bytes (including shader
layout padding), not Bevy's typed shader serialization; `None` clears CPU data.

## Read-only Material Storage

Use `StorageBinding[Vec4]` to bind an existing `ShaderBuffer` asset as a read-only
array of vec4 elements. The field value remains a typed buffer handle; PyBevy
assigns binding numbers and generates matching WGSL declarations. `StorageBinding`
is a declaration, not a buffer allocation or CPU data serializer.
`ShaderMaterialPlugin()` enables all custom material layouts. Add it once after
`DefaultPlugins`. The existing `StorageShaderMaterialPlugin()` remains supported;
there is no need to add both.

<!-- pybevy-snippet: typecheck -->
```python
import struct

from pybevy.assets import Assets, Handle
from pybevy.ecs import ResMut
from pybevy.material import StorageBinding, material
from pybevy.math import Vec4
from pybevy.pbr import Material, ShaderMaterialPlugin
from pybevy.render import ShaderBuffer

@material(fragment_shader="shaders/palette.wgsl")
class Palette(Material):
    colors: StorageBinding[Vec4]

def make_palette(
    buffers: ResMut[Assets[ShaderBuffer]],
    materials: ResMut[Assets[Palette]],
) -> Handle[Palette]:
    values = struct.pack("<8f", 1, 0, 0, 1, 0, 1, 0, 1)
    surface = Palette(colors=buffers.add(ShaderBuffer(values)))
    return materials.add(surface)
```

Bare `StorageBinding[T]` fields require a constructor argument. The argument and
attribute value are `Handle[ShaderBuffer] | None`, while `T` describes the WGSL
element (`float`, `int`, `Vec2`, `Vec3` or `Vec4`). Passing `None` explicitly
keeps the material pending until a valid handle is assigned. For an optional
field, declare `colors: StorageBinding[Vec4] = StorageBinding()` (`StorageBinding(default=None)` is equivalent).

Pyright/Pylance and mypy infer keyword constructor arguments and handle-valued
reads/writes through descriptor typing. Constructor checking is based on
explicit field defaults: provide arguments for annotations without defaults.
Ordinary material fields keep their existing runtime defaults; concise storage
fields enforce requiredness at runtime too. Decorated classes are not standard
library dataclasses; use the material API rather than `dataclasses.replace`.

Usually `@material` creates its own StandardMaterial. To supply a custom `base=`
with a typed constructor, declare it as initialization-only input:

<!-- pybevy-snippet: typecheck -->
```python
from dataclasses import InitVar
from pybevy.material import StorageBinding, material
from pybevy.math import Vec4
from pybevy.pbr import Material, StandardMaterial

@material(fragment_shader="shaders/palette.wgsl")
class BasedPalette(Material):
    base: InitVar[StandardMaterial | None] = None
    colors: StorageBinding[Vec4]

surface = BasedPalette(base=StandardMaterial(unlit=True), colors=None)
```

The base declaration adds no shader field. It is only needed for the explicit
`base=` argument; ordinary StandardMaterial assets require no decorator or
InitVar. Existing runtime base arguments remain accepted without the declaration,
but type checkers need the declared InitVar to include it in the constructor.

`assets/shaders/palette.wgsl` can read the array directly in a vertex or
fragment stage, including custom prepass/deferred shaders:

```wgsl
#import pybevy::gen::Palette as params

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return params::colors[0];
}
```

The generated declaration is `@group(3) @binding(109) var<storage, read>
colors: array<vec4<f32>>`. `arrayLength(&params::colors)` gives its current row
count. Existing uniform, texture, bool-definition and pass-hook syntax works
alongside the storage field. For per-instance lookup, use `MeshTag` and Bevy's
`mesh_functions::get_tag(instance_index)`, as in
[the storage-buffer example](../../../examples/bevy/shaders/storage_buffer.py).

Update bytes through `buffers.get_mut(handle).set_data(...)`, or assign a new
handle through a mutable material. Allocation changes, including resize,
refresh the material binding automatically. A field set to `None`, a removed
buffer, or an upload still awaiting preparation leaves the material pending
and suppresses its draw. Restoring a valid handle/upload makes it drawable.
There is no fallback storage buffer. Material and nested uniform accesses keep
the usual asset authority, change tracking and system lifetime rules.

Each field gets its own binding starting at 109, in declaration order. For
custom WGSL types, keep the explicit `Annotated[Handle[ShaderBuffer] | None,
StorageBinding(...)]` form below. Its `element` accepts `float` (`f32`), `int` (`i32`), `Vec2`, `Vec3`, `Vec4`, or WGSL type text
such as `"u32"`, `"mat3x3<f32>"`, and `"array<vec2<u32>, 3>"`. For manually declared structs,
provide their WGSL in `declarations`; typed dataclasses can use `@shader_struct` below. These storage types are separate from
uniform `int`, which retains its existing `f32` encoding.

<!-- pybevy-snippet: typecheck -->
```python
from typing import Annotated
from pybevy.assets import Handle
from pybevy.material import StorageBinding, material
from pybevy.pbr import Material
from pybevy.render import ShaderBuffer

paint = StorageBinding(
    element="Paint",
    declarations="struct Paint { color: vec3<f32>, intensity: f32, }",
)
assert (paint.stride, paint.alignment) == (16, 16)

@material()
class IndexedPaint(Material):
    indices: Annotated[Handle[ShaderBuffer] | None, StorageBinding(element="u32")] = None
    paints: Annotated[Handle[ShaderBuffer] | None, paint] = None
```

WGSL can read `params::paints[params::indices[0]].color`. Metadata validates
host-shareable types with Naga and reports `stride` and `alignment`; upload
bytes with that padding yourself. There is no automatic Python-object packing.
For example, a `vec3<f32>` array has stride 16, not 12. A `Paint` row above is
`struct.pack("<4f", red, green, blue, intensity)`. Nested fixed-size arrays and
structs work; booleans, atomics, pointers, nested runtime arrays and optional
16/64-bit scalar features are unsupported. Declarations contain types/constants,
not functions or entry points. Reuse identical declaration text for shared types.

The renderer rejects an empty buffer, a byte count not divisible by its element
stride, missing `STORAGE` usage, or a range over the device binding limit; it
logs the reason and keeps the material pending. All declared buffers must be
ready. Handle assignment rejects other asset types immediately. The raw
`StorageShaderMaterial` accepts `storage_buffers` and matching `storage_layouts`
(WGSL element/declaration pairs), with `get_storage_buffer_at` and
`set_storage_buffer_at` for indexed access. The original `storage_buffer`
constructor argument/property still addresses the first Vec4 slot by default.

Storage buffers require vertex and fragment storage-buffer support and adequate
combined material binding limits. Each material binds only its declared storage
slots; the device determines the maximum count. No new Cargo feature flag is
needed. This path is tested on native Vulkan; WebGL and other backends/devices
are not qualified by that result. Ordinary materials retain their existing
layout. Writable storage and general user-selected binding numbers remain
outside this API. Bevy 0.20's changing
buffer/shader APIs still require a separate port.

## Typed Shader Structs

Use `@shader_struct` above `@dataclass` to generate WGSL for a storage record.
`StorageBinding[Tile]` uses that definition; `pack_shader_structs` produces bytes
for `ShaderBuffer` or its `set_data` method.

<!-- pybevy-snippet: typecheck -->
```python
from dataclasses import dataclass
from pybevy.material import (
    StorageBinding, material, shader_struct, shader_struct_layout, pack_shader_structs,
)
from pybevy.math import Vec3, Vec4
from pybevy.pbr import Material
from pybevy.render import ShaderBuffer

@shader_struct
@dataclass
class Tile:
    offset: Vec3
    scale: float
    color: Vec4

@material(fragment_shader="shaders/tiles.wgsl")
class Tiles(Material):
    records: StorageBinding[Tile]

rows = [Tile(Vec3.ZERO, 1.0, Vec4.ONE)]
buffer = ShaderBuffer(pack_shader_structs(rows))
layout = shader_struct_layout(Tile)
assert layout.stride == 32
assert tuple(field.offset for field in layout.fields) == (0, 12, 16)
rows[0].scale = 2.0
buffer.set_data(pack_shader_structs(rows))
```

Add `buffer` to `Assets[ShaderBuffer]`, then pass its handle as `Tiles(records=handle)`.
In WGSL, import the material bindings and access `records[index].offset`, `.scale`
and `.color`. `ShaderMaterialPlugin()` is the same plugin used for other materials.

The first version accepts `float` (`f32`), `int` (`i32`), and `Vec2/Vec3/Vec4`.
Packing validates value types, finite f32 ranges and signed 32-bit integer ranges.
It writes little-endian bytes, zeroes padding and uses Naga's field offsets,
alignment and array stride. All records must have exactly the same decorated
class. An empty iterable produces empty bytes; `struct_type=Tile` can explicitly
select and validate the schema. ClassVar attributes are excluded; InitVar, nested
structs, matrices, arrays and bool fields are unsupported in this decorator.
Explicit WGSL `StorageBinding(element=..., declarations=...)` remains available.

The immutable `ShaderStructLayout` exposes declarations, size, stride, alignment
and per-field name, WGSL type, offset, size and alignment. It does not allocate or
upload GPU data. The packing helper does not update existing buffers or retain
records; call `set_data` explicitly after edits. Reading borrowed vector fields
still requires a live borrow. Layout metadata is separate from CPU packing so
future GPU producers can validate the same contract without CPU staging.
Binding metadata normalizes a decorated record type to its WGSL name and
declarations, avoiding global references to reloadable Python classes.

## Texture Dimensions and Sampling

Plain `Handle[Image] | None` fields retain the four ordinary 2D float slots and
white fallbacks. Use `Annotated[..., TextureBinding(...)]` to declare a different
sampled-image contract. These classes use `ShaderMaterialPlugin()` even
if they have no storage fields. TextureBinding-only layouts do not require storage
buffer support. There are still four texture slots at bindings 101–108 (with optional paired samplers);
this API does not expose binding arrays, storage textures or multisampled images.

<!-- pybevy-snippet: typecheck -->
```python
from typing import Annotated
from pybevy.assets import Handle
from pybevy.image import Image, ImageSampler
from pybevy.material import TextureBinding, material
from pybevy.pbr import Material, ShaderMaterialPlugin
from pybevy.render import Extent3d, TextureViewDescriptor, TextureViewDimension

@material(fragment_shader="shaders/environment.wgsl")
class Environment(Material):
    sky: Annotated[Handle[Image] | None, TextureBinding(dimension="cube")] = None

# Six square faces in +X, -X, +Y, -Y, +Z, -Z order.
image = Image(
    Extent3d(width=1, height=1, depth_or_array_layers=6),
    data=bytes((255, 255, 255, 255)) * 6,
)
image.texture_view_descriptor = TextureViewDescriptor(dimension=TextureViewDimension.Cube)
image.sampler = ImageSampler.linear()
# Add image to Assets[Image] and assign its handle to Environment.sky.
```

```wgsl
#import pybevy::gen::Environment as params

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return textureSample(params::sky, params::sky_sampler, vec3(1.0, 0.0, 0.0));
}
```

| `dimension` | Image configuration | Sampling coordinates |
|---|---|---|
| `"2d"` | `TextureDimension.D2`, one layer | `vec2` UV |
| `"2d_array"` | D2, multiple layers; `TextureViewDimension.D2Array` | `vec2` UV plus integer layer |
| `"cube"` | D2, six square faces; `TextureViewDimension.Cube` | `vec3` direction |
| `"cube_array"` | D2, six faces per cube; `TextureViewDimension.CubeArray` | `vec3` direction plus integer cube index |
| `"3d"` | `TextureDimension.D3`; depth is `depth_or_array_layers` | normalized `vec3` UVW |

Array sampling takes a separate fourth argument in standard WGSL:
`textureSample(tex, samp, uv, layer)` for 2D arrays, and
`textureSample(tex, samp, direction, cube_index)` for cube arrays. A 12-face cube
array contains cubes 0 and 1; the cube index is not a face-layer offset. Do not
pack the array index into a `vec4` coordinate.

The annotation describes the binding; it does not reinterpret or reshape the
image. Prepare the matching view with `image.texture_view_descriptor`, and use
`reinterpret_stacked_2d_as_array(layers)` when converting stacked source images.
Assigning a D3 view to a D2 image raises `ValueError`; create a volume using
`Image(..., dimension=TextureDimension.D3)`. This setter checks native dimension
compatibility; matching face counts, square cube faces and other descriptor
constraints still need valid image data and are subject to GPU validation.
Explicitly annotated fields are required: `None`, missing images or incompatible
views/formats/samplers leave the material pending with no fallback. Invalid
contracts log a diagnostic; correct the image or assign a compatible handle to
resume drawing. Image view and sampler allocation changes refresh bindings.

`sample_type` is `"float"` (default), `"sint"`, `"uint"` or `"depth"`.
For integer or unfilterable float formats, set `filterable=False` and
`sampler="non_filtering"`, configure `ImageSampler.nearest()`, and use
`textureLoad` where WGSL requires it. For example:
`TextureBinding(sample_type="uint", filterable=False, sampler="non_filtering")`
pairs with `TextureFormat.Rgba32Uint` and emits `texture_2d<u32>`.
Depth contracts use `filterable=False`; `sampler="comparison"` emits
`sampler_comparison` and requires an Image sampler descriptor with `compare`
set. Use `textureSampleCompare` with a reference depth. Depth 3D textures are
invalid. Headless Vulkan tests cover comparisons for 2D, 2D-array, cube and
cube-array views, including comparison-function changes and image uploads.

For CPU-authored depth data, `TextureFormat.Depth16Unorm` accepts two little-endian
bytes per texel: `struct.pack("<H", 49151)` represents approximately 0.75.
Set `ImageSampler.Descriptor(ImageSamplerDescriptor(compare=ImageCompareFunction.Less))`
(import these sampler types from `pybevy.image`). A reference of 0.5 then passes;
0.9 fails. Depth32Float CPU uploads are forbidden by wgpu's copy rules; this
upload recipe does not generalize to every depth format.

Actual min/mag/mipmap filtering, wrap modes, LOD clamps and anisotropy stay on
`Image.sampler`, using `ImageSampler.Descriptor(ImageSamplerDescriptor(...))`.
`sampler="filtering"` permits linear or nearest filtering; `"non_filtering"`
requires nearest filtering. `filterable=True` requires a filterable texture
format/device feature. These image-owned settings affect every paired binding
using that image. Use independent sampler fields below to share its pixels with
different sampling settings. Images do
not acquire mipmaps automatically. See the [image sampling guide](sprites.md) for existing
image/sampler controls. No new Cargo feature flag is required.

## Independent Samplers

Use `TextureBinding(sampler=None)` to emit only the texture binding, and declare
`SamplerBinding` fields for separate native
samplers. This maps to Bevy's separate texture and sampler bindings. Register
`ShaderMaterialPlugin()`, including for sampler-only declarations.

<!-- pybevy-snippet: typecheck -->
```python
from pybevy.image import ImageSamplerDescriptor
from pybevy.material import SamplerBinding, TextureBinding, material
from pybevy.pbr import Material, ShaderMaterialPlugin

@material(fragment_shader="shaders/sampling.wgsl")
class Sampling(Material):
    image: TextureBinding = TextureBinding(sampler=None)
    sharp: SamplerBinding = SamplerBinding("non_filtering")
    soft: SamplerBinding = SamplerBinding(default=ImageSamplerDescriptor.linear())
```

```wgsl
#import pybevy::gen::Sampling as params
#import bevy_pbr::forward_io::VertexOutput

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let sharp = textureSample(params::image, params::sharp, input.uv);
    let soft = textureSample(params::image, params::soft, input.uv);
    return mix(sharp, soft, 0.5);
}
```

`SamplerBinding()` defaults to `"filtering"`; `"non_filtering"` requires nearest
min/mag/mipmap filters. `"comparison"` requires a descriptor with `compare`
set, emits `sampler_comparison`, and works with depth textures and
`textureSampleCompare`. Other kinds require `compare=None`. WGSL must use a
sampler compatible with the sampled texture. A sampler-free texture also works
with `textureLoad`, which needs no sampler at all.

The descriptor controls filtering, address modes, LOD clamps and anisotropy.
Defaults and constructor values are copied per material. Through
`ResMut[Assets[Sampling]]`, `surface.soft.mag_filter = ...` writes through and
refreshes rendering; replacing `surface.soft` also works. Read-only asset
access rejects changes, and borrowed descriptors expire with their system.
A raw owned `StorageShaderMaterial.get_sampler()` returns a read-only snapshot;
use `set_sampler()` to replace it. No image copy is needed, and these fields
ignore `Image.sampler` when the texture's paired sampler is omitted.

Descriptors with invalid LOD clamps, incompatible filtering/comparison modes,
or unsupported border-address features leave the material pending with a
render diagnostic. Correcting the descriptor resumes rendering. Anisotropy
above one requires all-linear filtering; mipmaps still require actual mip levels.
Border modes depend on enabled device features. No new Cargo feature is needed.
Native descriptors share cached sampler allocations, retired when no live
material needs them. Explicit sampler bindings start at `109 + storage_count`,
in field order, and count against the device's combined material limits.
Persistent errors log once until the diagnostic changes or rendering recovers,
even when a system repeatedly writes the same invalid descriptor. Messages name
the binding number, sampler kind and offending filter/comparison property; match
the number to the declaration order or generated WGSL to find its Python field.
Binding arrays, storage textures and multisampled textures remain unsupported.

## Runnable Binding Examples

The [material examples](../../../examples/materials/README.md) include complete
Python scenes and WGSL, with procedural assets and no downloads:

| Example | Binding technique |
|---|---|
| `independent_samplers.py` | One checker image and material, sampled with nearest and linear descriptors side by side |
| `structured_buffer.py` | A 32-byte WGSL struct array plus a separate `u32` index buffer, animated with `ShaderBuffer.set_data` |
| `cubemap.py` | Six square faces, a cube view, linear filtering and reflected-direction sampling |
| `array_texture.py` | One 2D-array image, nearest filtering and `MeshTag` selecting a layer per mesh |
| `volume_texture.py` | A genuine D3 image with linearly filtered, animated volume slices |

These use forward shaders. Add matching prepass/deferred hooks when extending
vertex deformation to shadows or motion vectors. The cubemap example demonstrates
direct sampling; use Bevy's environment-lighting path for prefiltered PBR lighting.
The volume example displays slices; ray-marched volume rendering requires its own
shader. Pixel regressions exercise the examples' actual setup systems and data
updates; finite-frame smoke tests exercise their entrypoints.

## WGSL Shader Side

Both vertex and fragment shaders can read the material uniforms at
`@group(3) @binding(100)`. The decorator generates a WGSL module containing
the matching uniform, texture, sampler, and optional storage declarations. Import that module by
the material class name:

Independent classes with the same short name can keep this import spelling.
PyBevy selects their matching generated layouts through pipeline specialization,
including when old material assets survive a class reload. Import the bindings
for the material drawing the mesh; importing another material's module is an error.

```wgsl
#import pybevy::gen::GlowMaterial as params

// Standard PBR imports
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // alpha is the exposure weight, not opacity: keep it 0.0
    pbr_input.material.emissive = vec4<f32>(
        params::material.color.rgb * params::material.intensity, 0.0);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
```

## One File for Vertex and Fragment Stages

Set both shader paths to the same WGSL file when its vertex and fragment entry
points share parameters:

```python
from pybevy.color import LinearRgba
from pybevy.math import Vec3
from pybevy.prelude import Material, material

@material(
    vertex_shader="shaders/offset_color.wgsl",
    fragment_shader="shaders/offset_color.wgsl",
)
class OffsetColorMaterial(Material):
    offset: Vec3 = Vec3.ZERO
    color: LinearRgba = LinearRgba(0.0, 1.0, 0.0, 1.0)
```

The vertex and fragment paths may also name different files. Omit
`vertex_shader` to keep Bevy's default vertex stage, or omit `fragment_shader`
to keep Bevy's default PBR fragment stage.

The matching `assets/shaders/offset_color.wgsl` below is suitable for ordinary
static meshes. A custom vertex shader replaces Bevy's vertex stage, so it must
produce the required `VertexOutput` fields:

```wgsl
#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}
#import pybevy::gen::OffsetColorMaterial as params

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var vertex = vertex_in;
    vertex.position += params::material.offset;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index,
    );
#endif
#ifdef VERTEX_POSITIONS
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
    out.position = position_world_to_clip(out.world_position.xyz);
#endif
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index,
    );
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return params::material.color;
}
```

For skinned or morph-target meshes, start from Bevy's full mesh vertex shader
flow and apply the displacement after skinning or morphing as appropriate.
Large vertex displacement can also move geometry outside its CPU-side culling
bounds. Supply matching prepass/deferred vertex shaders for the passes that
must see that displacement; the forward vertex hook alone leaves them on
Bevy's defaults. See the pass contracts below.

## Depth, Shadows, Motion Vectors and Deferred Rendering

Choose each shader stage explicitly. Omitted paths use Bevy's default stage;
a forward shader is never copied automatically into a different pass.

| Decorator / `ShaderMaterial` keyword | Runs in | Shader interface |
|---|---|---|
| `vertex_shader`, `fragment_shader` | Forward rendering | `bevy_pbr::forward_io` |
| `prepass_vertex_shader`, `prepass_fragment_shader` | Depth, normal, motion-vector prepasses and shadow maps | `bevy_pbr::prepass_io` |
| `deferred_vertex_shader`, `deferred_fragment_shader` | Deferred G-buffer pass | `bevy_pbr::prepass_io`, `pbr_deferred_functions::deferred_output` |

These optional keywords need no additional Bevy feature flags in the standard
native build. Shader loading errors identify the stage, and file edits reload
its shader. Generated uniforms, textures and bool definitions are available to
each explicitly customized pass.

The PBR fragment example above handles both forward and deferred output. Opt
into its deferred entry point using the same asset path:

<!-- pybevy-snippet: typecheck -->
```python
from pybevy.color import LinearRgba
from pybevy.material import material
from pybevy.pbr import Material

@material(
    fragment_shader="shaders/glow.wgsl",
    deferred_fragment_shader="shaders/glow.wgsl",
)
class GlowMaterial(Material):
    color: LinearRgba = LinearRgba(0.0, 1.0, 0.5, 1.0)
    intensity: float = 1.5
```

Configure the scene's deferred camera with `DepthPrepass()`, `DeferredPrepass()`
and `Msaa.Off`, and select deferred rendering on the base material or through
`DefaultOpaqueRendererMethod.deferred()`. A material whose base explicitly uses
`OpaqueRendererMethod.Forward` still uses its forward hooks in that scene.
Deferred shaders write PBR/G-buffer data; an effect applied after forward
lighting may need a separate later effect instead of a deferred equivalent.
Do not use this G-buffer fragment as an ordinary prepass/shadow fragment.

For a custom prepass, use `PREPASS_PIPELINE` to select the prepass vertex
interface and `DEFERRED_PREPASS` to distinguish the G-buffer pass. A forward
shader can also see `DEFERRED_PREPASS` when the camera has a deferred prepass;
check `PREPASS_PIPELINE` first when sharing a file across stages.

In prepass fragments, `NORMAL_PREPASS`, `MOTION_VECTOR_PREPASS` and
`UNCLIPPED_DEPTH_ORTHO_EMULATION` determine the outputs. `PREPASS_FRAGMENT`
enables `prepass_io::FragmentOutput`; a depth-only or shadow fragment can have
no color output and return nothing. Providing `prepass_fragment_shader` runs
that fragment even for an opaque depth-only pass, allowing custom `discard`.
Match the forward cutout in every relevant pass. Ordinary blend transparency
does not acquire opaque prepass participation just by providing a hook.

Animated vertex deformation must provide **current and previous** positions.
Use the current object transform for the current deformation and
`mesh_functions::get_previous_world_from_local` for the previous deformation;
the prepass fragment can use `pbr_prepass_functions::calculate_motion_vector`.
Keep the previous animation parameters yourself. Camera/object transform
history alone cannot recover an arbitrary shader's previous shape.
Matching depth and motion are prerequisites for correct depth effects and
motion blur. Shader displacement still does not update CPU culling bounds;
use conservative mesh bounds or `NoFrustumCulling()` where appropriate.

### Grouping Parameters

One `@material` class defines one flat material bind group shared by its
shader stages. Nested classes and dataclasses are not supported as material fields.
Keep related values together by declaration order and use descriptive prefixes
such as `wave_amplitude`, `wave_frequency`, and `surface_color`. The generated
WGSL module preserves the Python declaration order and corresponding WGSL types.

Uniform fields occupy binding 100. A `bool` is a shader definition rather than
uniform data, and each `Handle[Image] | None` field uses a separate texture and
sampler binding.
The decorator does not expose additional custom bind groups.

### Non-PBR (simple) fragment shader

For materials that don't use PBR lighting:

```wgsl
#import bevy_pbr::forward_io::VertexOutput
#import pybevy::gen::MyMaterial as params

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return params::material.color;
}
```

## Choosing PBR Extension vs Non-PBR

| Use case | Pattern | Why |
|----------|---------|-----|
| Modify PBR properties (add emissive glow on top of normal lighting, posterize output, tint result) | PBR extension | You want Bevy's lighting, just tweaked |
| Fully self-lit material (emissive-only, no PBR lighting needed) | Non-PBR | You compute all color yourself |
| Procedural planets, LED screens, toon shaders with custom lighting | Non-PBR | PBR lighting would fight your output |
| `base_color` is BLACK and all visual output comes from emissive/procedural color | Non-PBR | You compute all color yourself, so PBR buys you nothing |

**`emissive.a` is the exposure weight, not opacity.** Bevy packs
`StandardMaterial.emissive_exposure_weight` into the alpha channel of the
material's `emissive` value, and `apply_pbr_lighting` computes:

```wgsl
emissive_light = emissive.rgb * mix(1.0, view.exposure, emissive.a);
```

So the weight decides **how much camera exposure attenuates the emissive**:

| `emissive.a` | factor at the default camera exposure | result |
|---|---|---|
| `0.0` (Bevy's default) | `1.0` | full strength, exposure-independent |
| `1.0` | `view.exposure` ≈ `1/1000` | ~1000x dimmer, reads as unlit |
| `> 1.0` | **negative** (`mix` is not clamped) | clamps to black |

Two consequences for a PBR extension shader:

- **Write the alpha yourself, and use `0.0`.** A `vec4 * scalar` multiplies the
  alpha too, so `emissive = color * intensity` sets the weight to `intensity`,
  and any `intensity` above 1.0 renders **black**. Write
  `vec4<f32>(color.rgb * intensity, 0.0)` instead.
- **`base=StandardMaterial(emissive_exposure_weight=...)` has no effect here.**
  `pbr_input_from_standard_material()` consumes the base material's weight, and
  the next line of the snippet overwrites `pbr_input.material.emissive`
  wholesale.

## Shader Defs (bool fields)

Bool fields become compile-time `#ifdef` directives instead of uniform data:

```python
@material(fragment_shader="shaders/character.wgsl")
class CharacterMaterial(Material):
    color: LinearRgba = LinearRgba(1.0, 1.0, 1.0, 1.0)
    is_highlighted: bool = False  # enables #ifdef IS_HIGHLIGHTED
    is_damaged: bool = False      # enables #ifdef IS_DAMAGED
```

In WGSL:
```wgsl
#import bevy_pbr::forward_io::VertexOutput
#import pybevy::gen::CharacterMaterial as params

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = params::material.color;
#ifdef IS_HIGHLIGHTED
    color = mix(color, vec4(1.0, 0.9, 0.3, 1.0), 0.5);
#endif
#ifdef IS_DAMAGED
    color = mix(color, vec4(1.0, 0.0, 0.0, 1.0), 0.3);
#endif
    return color;
}
```

Different bool combinations produce different compiled shader pipelines (cached automatically).

## Texture Slots

Up to 4 `Handle[Image] | None` fields map to texture/sampler bindings:

```python
from pybevy.assets import Handle
from pybevy.image import Image

@material(fragment_shader="shaders/terrain.wgsl")
class TerrainMaterial(Material):
    blend_sharpness: float = 2.0
    grass: Handle[Image] | None = None  # slot 0: bindings 101/102
    rock: Handle[Image] | None = None   # slot 1: bindings 103/104
```

`Handle[Image]` and `Optional[Handle[Image]]` also declare a texture slot. An
`Image` annotation, or a union with any other type, raises `TypeError` when the
class is decorated.

In the setup system, pass loaded image handles:

```python
def setup(commands, meshes, materials, asset_server: Res[AssetServer]):
    grass_tex = asset_server.load_image("textures/grass.png")
    rock_tex = asset_server.load_image("textures/rock.png")
    mat = TerrainMaterial(
        grass=grass_tex,
        rock=rock_tex,
    )
    handle = materials.add(mat)
```

The attribute holds a `Handle[Image]`, or `None` for an empty slot. Reading it
returns that handle from a new material, from `get()`, `get_mut()`, iteration,
and `remove()`, so `mat.grass = mat.grass` keeps the texture. Assign a handle
or `None` through `get_mut()` to change the slot; `get()` and iterated
materials reject writes. A returned handle is an ordinary strong handle that
stays valid after the system and keeps its image alive.

In WGSL:
```wgsl
#import bevy_pbr::forward_io::VertexOutput
#import pybevy::gen::TerrainMaterial as params

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let grass_color = textureSample(params::grass, params::grass_sampler, in.uv);
    let rock_color = textureSample(params::rock, params::rock_sampler, in.uv);
    return mix(rock_color, grass_color, params::material.blend_sharpness);
}
```

## Per-Instance Mesh Tags

`MeshTag` supplies a small per-entity integer to a shader while entities keep
sharing one mesh and material:

```python
from pybevy.mesh import MeshTag

commands.spawn(Mesh3d(mesh), MeshMaterial3d(material), MeshTag(3))
```

Read it from a fragment shader whose input is `VertexOutput`:

```wgsl
#import bevy_pbr::mesh_functions

let tag = mesh_functions::get_tag(in.instance_index);
```

Use the tag as an index or branch selector. It is not arbitrary structured
instance data.

## Runtime Mutation

Modify material fields at 60fps from update systems:

```python
def animate_materials(
    materials: ResMut[Assets[GlowMaterial]],
    query: Query[MeshMaterial3d[GlowMaterial]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    for mesh_mat in query:
        mat = materials.get_mut(mesh_mat.handle)
        if mat is not None:
            mat.intensity = 1.0 + math.sin(t * 2.0) * 0.5
```

`materials.get_mut()` returns a typed wrapper with property accessors.
Each property set writes the native material asset; Bevy uploads changed data
during render preparation.
`Vec2`, `Vec3`, `Vec4`, and `LinearRgba` fields are live views of the packed
uniform, so `mat.color.alpha = 0.25` writes through a `get_mut()` material.
Through `get()` or iteration they are read-only, and every view expires with
its system. On a material not yet added to `Assets`, or one returned by
`remove()`, they are read-only snapshots; assign the whole field instead, for
example `mat.color = LinearRgba(1.0, 0.0, 0.0, 0.25)`.

## Binding Layout

| Binding | Slot         | Type                      |
|---------|--------------|---------------------------|
| 100     | uniforms     | `var<uniform>` struct     |
| 101     | texture_0    | `texture_2d<f32>`         |
| 102     | sampler_0    | `sampler`                 |
| 103     | texture_1    | `texture_2d<f32>`         |
| 104     | sampler_1    | `sampler`                 |
| 105     | texture_2    | `texture_2d<f32>`         |
| 106     | sampler_2    | `sampler`                 |
| 107     | texture_3    | `texture_2d<f32>`         |
| 108     | sampler_3    | `sampler`                 |
| 109     | opt-in storage | `var<storage, read>` vec4 array |

All bindings are at `@group(3)` (the material bind group).
Unused texture slots automatically get Bevy's 1x1 white fallback texture.

## Bevy Globals in Shaders

You can access Bevy's built-in globals (time, etc.) without custom uniforms:

```wgsl
#import bevy_pbr::mesh_view_bindings::globals

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = sin(globals.time * 2.0) * 0.5 + 0.5;
    return vec4(t, 0.0, 1.0 - t, 1.0);
}
```

Use imports that match the pass's native bindings. Forward view resources are
not universally available in a shadow/prepass layout. The optional pass hooks
above determine which custom stages run; providing only forward paths keeps
the base material's default prepass, shadow and deferred behavior.

All `@material` classes use the shared `ShaderMaterialPlugin`. You may add a new
decorated class during hot reload as long as that plugin was installed when the
app started. Adding the plugin itself to an already-running app still requires a
restart.

## Brightness & Tone Mapping

Shader output values go through tone mapping before reaching the screen. The behavior differs between rendering paths.

### Non-PBR shaders

Output goes directly to the HDR framebuffer. A separate post-processing pass applies tone mapping.

| Output range | On-screen result |
|--------------|------------------|
| `[0, 1]` | Normal surface colors - rendered at roughly face value |
| `> 1.0` | Triggers bloom glow (requires `Bloom` on the camera). The further above 1.0, the stronger the glow |

Reference values for common colors:

| Desired color | Approximate output value |
|---------------|--------------------------|
| Deep ocean blue | `vec3(0.04, 0.12, 0.5)` |
| Green land | `vec3(0.18, 0.45, 0.12)` |
| White clouds | `vec3(0.7, 0.72, 0.75)` |
| Bright ice/snow | `vec3(0.95, 0.97, 1.0)` |
| Glowing atmosphere rim | `1.0–3.0 * fresnel` |
| City lights / bloom dots | `vec3(1.5, 1.1, 0.5) * mask` |

### PBR extension shaders

Output goes through `apply_pbr_lighting` (PBR calculations) then `main_pass_post_lighting_processing` (fog, etc.), and finally tone mapping in a separate pass.

- Emissive brightness depends on `emissive_exposure_weight`, which Bevy packs into the **alpha** of `emissive` and applies as `mix(1.0, view.exposure, emissive.a)`: `0.0` (the default) is full strength, `1.0` divides by the camera exposure (about 1000x at the default)
- `emissive_exposure_weight=1.0` is for making emissive track exposure like real light, not for making it brighter
- In a PBR extension shader, set the alpha yourself: `vec4<f32>(rgb * intensity, 0.0)`. Values above 1.0 in that channel make `mix` extrapolate negative and the surface renders black
- If all your color comes from emissive/procedural computation, prefer the non-PBR path

## Required Plugin

Always add `ShaderMaterialPlugin()` before using any `@material` classes:

```python
app.add_plugins(ShaderMaterialPlugin())
```

## Examples

See `examples/bevy/shaders/` for complete working examples:
- `shader_material.py` - basic uniform tint + emissive
- `shader_defs.py` - bool fields enable `#ifdef` conditional compilation
- `extended_material.py` - PBR extension with posterize effect
- `animate_shader.py` - time-based animation via `globals.time`
