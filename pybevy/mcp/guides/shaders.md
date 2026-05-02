# Custom Shaders with `@material`

PyBevy's `@material` decorator lets you define custom shader materials from Python.
It generates WGSL bindings, handles GPU data packing, and integrates with Bevy's PBR pipeline.

## Quick Start

```python
from pybevy.prelude import *
from pybevy.color import LinearRgba
from pybevy.pbr import ShaderMaterialPlugin

@material(fragment_shader="shaders/glow.wgsl")  # relative to assets/ directory
class GlowMaterial:
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
    alpha_mode=AlphaMode.BLEND,    # transparency blending
    double_sided=True,              # render both faces
    cull_mode=None,                 # disable face culling
    unlit=True,                     # ignore lighting
    depth_bias=1.0,                 # prevent z-fighting
)
class GlassMaterial:
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
1. Inspects type hints → computes std140 layout
2. Injects a WGSL struct into Bevy's shader asset system in-memory
3. Creates `__init__` that packs field values into a 256-float buffer
4. Creates property accessors for runtime mutation
5. Registers metadata for `Assets[YourMaterial]` and `MeshMaterial3d[YourMaterial]`

## WGSL Shader Side

Your fragment shader receives the material uniforms at `@group(3) @binding(100)`:

```wgsl
// Import the auto-generated struct, or declare it manually:
struct GlowMaterial {
    color: vec4<f32>,
    intensity: f32,
}
@group(3) @binding(100) var<uniform> material: GlowMaterial;

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
    pbr_input.material.emissive = material.color * material.intensity;
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

### Non-PBR (simple) fragment shader

For materials that don't use PBR lighting:

```wgsl
#import bevy_pbr::forward_io::VertexOutput

struct MyMaterial {
    color: vec4<f32>,
}
@group(3) @binding(100) var<uniform> material: MyMaterial;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return material.color;
}
```

## Choosing PBR Extension vs Non-PBR

| Use case | Pattern | Why |
|----------|---------|-----|
| Modify PBR properties (add emissive glow on top of normal lighting, posterize output, tint result) | PBR extension | You want Bevy's lighting, just tweaked |
| Fully self-lit material (emissive-only, no PBR lighting needed) | Non-PBR | You compute all color yourself |
| Procedural planets, LED screens, toon shaders with custom lighting | Non-PBR | PBR lighting would fight your output |
| `base_color` is BLACK and all visual output comes from emissive/procedural color | Non-PBR | PBR path attenuates emissive unexpectedly |

**Why PBR emissive appears dim:** Bevy's PBR pipeline computes `emissive * mix(1.0, exposure, emissive_exposure_weight)`. The default `emissive_exposure_weight=0.0` means emissive is NOT scaled by camera exposure — so values like `vec4(10.0, 0.0, 0.0, 1.0)` get compressed to near-invisible by tone mapping. Fixes:
- Set `base=StandardMaterial(emissive_exposure_weight=1.0)` so emissive scales with exposure
- Or use much larger emissive values (500+)
- Or use the non-PBR pattern which bypasses `apply_pbr_lighting` entirely

## Shader Defs (bool fields)

Bool fields become compile-time `#ifdef` directives instead of uniform data:

```python
@material(fragment_shader="shaders/character.wgsl")
class CharacterMaterial:
    color: LinearRgba = LinearRgba(1.0, 1.0, 1.0, 1.0)
    is_highlighted: bool = False  # → #ifdef IS_HIGHLIGHTED
    is_damaged: bool = False      # → #ifdef IS_DAMAGED
```

In WGSL:
```wgsl
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = material.color;
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
class TerrainMaterial:
    blend_sharpness: float = 2.0
    grass: Image = None   # slot 0: bindings 101/102
    rock: Image = None    # slot 1: bindings 103/104
```

In the setup system, pass loaded image handles:

```python
def setup(commands, meshes, materials, asset_server: Res[AssetServer]):
    grass_tex = asset_server.load("textures/grass.png")
    rock_tex = asset_server.load("textures/rock.png")
    mat = TerrainMaterial(
        grass=grass_tex,
        rock=rock_tex,
    )
    handle = materials.add(mat)
```

In WGSL:
```wgsl
@group(3) @binding(101) var grass: texture_2d<f32>;
@group(3) @binding(102) var grass_sampler: sampler;
@group(3) @binding(103) var rock: texture_2d<f32>;
@group(3) @binding(104) var rock_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let grass_color = textureSample(grass, grass_sampler, in.uv);
    let rock_color = textureSample(rock, rock_sampler, in.uv);
    return mix(rock_color, grass_color, material.blend_sharpness);
}
```

## Runtime Mutation

Modify material fields at 60fps from update systems:

```python
def animate_materials(
    materials: ResMut[Assets[GlowMaterial]],
    query: Query[MeshMaterial3d[GlowMaterial]],
    time: Res[Time],
) -> None:
    t = time.elapsed_seconds()
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

All bindings are at `@group(3)` (Bevy 0.18 material bind group).
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

> **WARNING: `globals` and `mesh_view_bindings` crash the prepass pipeline (PBR extension only).**
>
> When using the **PBR extension pattern** (with `pbr_input_from_standard_material`), your
> shader is compiled for both forward and prepass passes. `globals` and `view` are only
> available in the forward pass, so you MUST import them inside the `#else` block:
>
> ```wgsl
> #ifdef PREPASS_PIPELINE
> #import bevy_pbr::{
>     prepass_io::{VertexOutput, FragmentOutput},
>     pbr_deferred_functions::deferred_output,
> }
> #else
> #import bevy_pbr::{
>     forward_io::{VertexOutput, FragmentOutput},
>     pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
>     mesh_view_bindings::{globals, view},
> }
> #endif
> ```
>
> **Non-PBR shaders** (simple `-> @location(0) vec4<f32>` return) do **not** have a prepass
> variant, so top-level imports of `globals` and `view` are safe:
>
> ```wgsl
> #import bevy_pbr::forward_io::VertexOutput
> #import bevy_pbr::mesh_view_bindings::{globals, view}  // safe in non-PBR
> ```

> **Tip: Define all `@material` classes before first run.** Each new `@material` class
> registers a distinct Bevy material plugin, which requires an app restart. If you define
> all your material classes upfront, you avoid repeated restarts during iteration.

## Brightness & Tone Mapping

Shader output values go through tone mapping before reaching the screen. The behavior differs between rendering paths.

### Non-PBR shaders

Output goes directly to the HDR framebuffer. A separate post-processing pass applies tone mapping.

| Output range | On-screen result |
|--------------|------------------|
| `[0, 1]` | Normal surface colors — rendered at roughly face value |
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
- `shader_material.py` — basic uniform tint + emissive
- `shader_defs.py` — bool fields → `#ifdef` conditional compilation
- `extended_material.py` — PBR extension with posterize effect
- `animate_shader.py` — time-based animation via `globals.time`
