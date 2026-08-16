# Custom Shaders with `@material`

PyBevy's `@material` decorator lets you define custom shader materials from Python.
It generates WGSL bindings, handles GPU data packing, and integrates with Bevy's PBR pipeline.

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

When you need full control over the base material (e.g., setting `base_color`, textures), pass `base=` explicitly:

```python
mat = GlowMaterial(
    base=StandardMaterial(base_color=Color.BLACK, emissive_exposure_weight=1.0),
    color=LinearRgba(0.0, 1.0, 0.5, 1.0),
)
```

## Supported Field Types

| Python type    | WGSL type      | Notes                          |
|----------------|----------------|--------------------------------|
| `float`        | `f32`          | 4 bytes                        |
| `int`          | `f32`          | 4 bytes, cast to float         |
| `Vec2`         | `vec2<f32>`    | 8 bytes, 8-byte aligned        |
| `Vec3`         | `vec3<f32>`    | 12 bytes, 16-byte aligned      |
| `Vec4`         | `vec4<f32>`    | 16 bytes, 16-byte aligned      |
| `LinearRgba`   | `vec4<f32>`    | 16 bytes, 16-byte aligned      |
| `bool`         | shader def     | `#ifdef FIELD_NAME` in WGSL    |
| `Image`        | texture slot   | `texture_2d<f32>` + `sampler`  |

## How It Works

The decorator:
1. Inspects type hints and computes std140 layout
2. Builds the matching material binding layout for Bevy
3. Creates `__init__` that packs field values into a 256-float buffer
4. Creates property accessors for runtime mutation
5. Registers metadata for `Assets[YourMaterial]` and `MeshMaterial3d[YourMaterial]`

Each decorated class keeps a distinct logical identity even though the engine stores
all of them as `ShaderMaterial`. `Assets[GlowMaterial]` rejects handles from another
custom material class, and `Query[MeshMaterial3d[GlowMaterial]]` only matches entities
using `GlowMaterial`. In optional query data, a different custom material class is
treated as absent and yields `None`; it does not remove the entity from the query.

Partial reload preserves a material's identity when its qualified name and field layout
are unchanged. Changing field names, types, or ordering allocates a fresh identity, as
does a full reload, so old handles cannot be interpreted through a different layout.

All custom material collections share Bevy's underlying `Assets[ShaderMaterial]`
resource. A system therefore cannot request two mutable custom-material collections at
once, such as both `ResMut[Assets[A]]` and `ResMut[Assets[B]]`; split those mutations
across ordered systems.

## WGSL Shader Side

Both vertex and fragment shaders can read the material uniforms at
`@group(3) @binding(100)`. The decorator generates a WGSL module containing
the matching uniform, texture, and sampler declarations. Import that module by
the material class name:

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
    pbr_input.material.emissive = params::material.color * params::material.intensity;
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
bounds. Custom vertex displacement does not affect PyBevy's default shadow or
prepass pipelines.

### Grouping Parameters

One `@material` class defines one flat material bind group shared by both
stages. Nested classes and dataclasses are not supported as material fields.
Keep related values together by declaration order and use descriptive prefixes
such as `wave_amplitude`, `wave_frequency`, and `surface_color`. The generated
WGSL module preserves the Python declaration order and corresponding WGSL types.

Uniform fields occupy binding 100. A `bool` is a shader definition rather than
uniform data, and each `Image` field uses a separate texture and sampler binding.
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
| `base_color` is BLACK and all visual output comes from emissive/procedural color | Non-PBR | PBR path attenuates emissive unexpectedly |

**Why PBR emissive appears dim:** Bevy's PBR pipeline computes `emissive * mix(1.0, exposure, emissive_exposure_weight)`. The default `emissive_exposure_weight=0.0` means emissive is NOT scaled by camera exposure - so values like `vec4(10.0, 0.0, 0.0, 1.0)` get compressed to near-invisible by tone mapping. Fixes:
- Set `base=StandardMaterial(emissive_exposure_weight=1.0)` so emissive scales with exposure
- Or use much larger emissive values (500+)
- Or use the non-PBR pattern which bypasses `apply_pbr_lighting` entirely

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

Up to 4 `Image` fields map to texture/sampler bindings:

```python
@material(fragment_shader="shaders/terrain.wgsl")
class TerrainMaterial(Material):
    blend_sharpness: float = 2.0
    grass: Image = None   # slot 0: bindings 101/102
    rock: Image = None    # slot 1: bindings 103/104
```

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
Each property set triggers a Rust FFI call to write directly to the GPU buffer.

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

> **Custom shaders never run in prepass or shadow pipelines.** Those pipelines keep
> bevy's default shaders, so top-level imports of `globals` and `view` are safe in any
> custom shader, and a custom **vertex** shader's displacement does not move prepass
> depth or normals.
>
> **WARNING: `@material` shaders do not run under deferred rendering.** The mesh remains
> visible using its base `StandardMaterial`, but the custom vertex and fragment shaders
> are not applied. Do not combine `@material` custom shaders with
> `DefaultOpaqueRendererMethod.deferred()` (which the SSR recipe requires): keep custom
> shader scenes on forward rendering.

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

- Emissive brightness depends on `emissive_exposure_weight` (default `0.0` = emissive is NOT scaled by camera exposure)
- With `emissive_exposure_weight=0.0`: values like 10.0 appear nearly invisible after tone mapping. Use 500+ for visible emission, or set `emissive_exposure_weight=1.0` on the base `StandardMaterial`
- With `emissive_exposure_weight=1.0`: emissive scales with camera exposure, values in [1, 10] produce clearly visible emission
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
