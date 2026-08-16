# Advanced Materials

Clearcoat, anisotropy, normal maps, parallax mapping, UV tiling, texture loading with repeat/sampler settings, and rendering method control.

## Clearcoat (Car Paint / Wet Surfaces)

Adds a second specular layer on top of the base material - simulates lacquer, varnish, or water film.

| Parameter | What it does | Range |
|-----------|-------------|-------|
| `clearcoat` | Strength of the coat layer | `0.0–1.0` |
| `clearcoat_perceptual_roughness` | Smoothness of the coat | `0.0–1.0` |

```python
# Car paint - metallic base with smooth clear coat
car_paint = materials.add(StandardMaterial(
    base_color=Color.srgb(0.7, 0.0, 0.0),
    metallic=0.8,
    perceptual_roughness=0.4,        # Slightly rough base
    clearcoat=1.0,
    clearcoat_perceptual_roughness=0.1,  # Very smooth coat
))

# Wet stone - rough base with glossy water layer
wet_stone = materials.add(StandardMaterial(
    base_color=Color.srgb(0.3, 0.3, 0.3),
    metallic=0.0,
    perceptual_roughness=0.8,        # Rough stone
    clearcoat=0.6,
    clearcoat_perceptual_roughness=0.05,
))
```

**Tip:** Clearcoat also accepts normal map textures (`clearcoat_normal_texture`) for orange-peel effects.

## Anisotropy (Brushed Metal)

Makes reflections stretch in one direction - for brushed metal, hair, silk.
The mesh must have tangents whenever `anisotropy_strength` is nonzero, even
when no anisotropy texture is used. Built-in primitive meshes do not include
tangents; build the mesh first and generate them before adding it to `Assets`:

```python
mesh_data = Sphere(1.4).mesh().uv(64, 36)
mesh_data.generate_tangents()
brushed_mesh = meshes.add(mesh_data)
```

| Parameter | What it does |
|-----------|-------------|
| `anisotropy_strength` | How much stretching (`0.0–1.0`) |
| `anisotropy_rotation` | Direction of grain (radians, `0.0–π`) |

```python
brushed_steel = materials.add(StandardMaterial(
    base_color=Color.srgb(0.7, 0.7, 0.75),
    metallic=1.0,
    perceptual_roughness=0.3,
    anisotropy_strength=0.8,
    anisotropy_rotation=0.0,    # Horizontal brush direction
))
```

For grain that varies across the surface, use `anisotropy_texture`. R/G encode
the direction (remapped `[0, 1]` to `[-1, 1]`), B multiplies
`anisotropy_strength`:

```python
from pybevy.image import ImageLoaderSettings

# Texels are vectors, not colours: load linear.
groove = asset_server.load_image_with_settings(
    "textures/groove_anisotropy.png", ImageLoaderSettings(is_srgb=False)
)

# Tangents are required by anisotropy itself, not only by the texture.
mesh_data = Sphere(1.4).mesh().uv(64, 36)
mesh_data.generate_tangents()
mesh = meshes.add(mesh_data)

vinyl = materials.add(StandardMaterial(
    metallic=1.0,
    perceptual_roughness=0.25,
    anisotropy_strength=1.0,
    anisotropy_texture=groove,
))
```

**Anisotropy textures are unavailable on macOS and iOS** (Metal sampler limit):
passing one there raises `RuntimeError`. Scalar anisotropy is supported there,
but still requires mesh tangents.

## Normal Maps

Add surface detail without extra geometry. The texture encodes per-pixel surface normals.

```python
wall = materials.add(StandardMaterial(
    base_color_texture=asset_server.load_image("bevy/textures/parallax_example/cube_color.png"),
    normal_map_texture=asset_server.load_image("bevy/textures/parallax_example/cube_normal.png"),
    perceptual_roughness=0.7,
))
```

**`flip_normal_map_y`**: Set to `True` if your normal map uses OpenGL convention (green channel points up). Bevy expects DirectX convention by default.

**Tangents required:** Normal maps need tangent data, and no primitive ships it: `Cuboid`, `Sphere` and the rest carry position, normal and UV only. Call `mesh.generate_tangents()` on the built mesh (positions, normals, UVs and indices must already be set), or the normal map is silently ignored and the surface renders flat.

## Parallax Mapping

Adds depth illusion to flat surfaces using a height/depth map.

```python
parallax_surface = materials.add(StandardMaterial(
    base_color_texture=asset_server.load_image("bevy/textures/parallax_example/cube_color.png"),
    normal_map_texture=asset_server.load_image("bevy/textures/parallax_example/cube_normal.png"),
    depth_map=asset_server.load_image("bevy/textures/parallax_example/cube_depth.png"),
    parallax_depth_scale=0.05,       # Depth intensity (keep low to avoid artifacts)
    max_parallax_layer_count=16.0,   # Quality (higher = better but slower)
))
```

**Tip:** Keep `parallax_depth_scale` at `0.02–0.08`. Higher values cause visible swimming artifacts at steep angles.

## Baked Lightmaps

`Lightmap` uses a mesh's second UV set. Built-in primitive meshes provide
`Mesh.ATTRIBUTE_UV_0`, but not `Mesh.ATTRIBUTE_UV_1`; supply authored lightmap
UVs or copy the first set when both textures intentionally use the same layout:

```python
import numpy as np

mesh_data = Plane3d(Vec3.Y, Vec2(4.0, 4.0)).mesh().build()
uv1 = np.array(mesh_data.attribute(Mesh.ATTRIBUTE_UV_0), dtype=np.float32)
mesh_data.insert_attribute(Mesh.ATTRIBUTE_UV_1, uv1)

mesh = meshes.add(mesh_data)
material = materials.add(StandardMaterial(lightmap_exposure=4.0))
lightmap = asset_server.load_image("lightmaps/floor.ktx2")

commands.spawn(
    Mesh3d(mesh),
    MeshMaterial3d(material),
    Lightmap(image=lightmap),
)
```

`lightmap_exposure` is asset-dependent; tune it for the radiance range stored
by the baker. Do not attach `Lightmap` to a mesh without `ATTRIBUTE_UV_1`:
Bevy 0.19 selects its lightmap shader from the component and that pipeline is
invalid without the corresponding vertex attribute.

## UV Transform (Tiling)

Scale, rotate, or offset texture coordinates with `uv_transform`:

```python
from pybevy.math import Affine2

# Tile the texture 4x4
tiled_floor = materials.add(StandardMaterial(
    base_color_texture=asset_server.load_image("bevy/textures/parallax_example/cube_color.png"),
    uv_transform=Affine2.from_scale(Vec2(4.0, 4.0)),
))
```

Nested material fields remain connected when the material comes from mutable
asset access:

```python
material = materials.get_mut(tiled_floor)
if material is not None:
    material.uv_transform.translation.x = 0.25
    material.base_color.set_alpha(0.8)
```

Do this inside the system call that supplied `materials`; retained asset
wrappers expire when that call ends.

## Texture Loading with Repeat Sampler

By default, Bevy textures clamp to edge - UV coordinates outside `[0, 1]` repeat the edge pixel. To make textures tile (repeat), load them with `ImageAddressMode.Repeat`:

```python
from pybevy.image import (
    ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
    ImageAddressMode,
)
from pybevy.math import Affine2

# Create repeat sampler settings (reuse for all tiling textures)
repeat_settings = ImageLoaderSettings(sampler=ImageSampler.Descriptor(
    ImageSamplerDescriptor(
        address_mode_u=ImageAddressMode.Repeat,
        address_mode_v=ImageAddressMode.Repeat,
    )
))

# Load textures with repeat enabled
color_tex = asset_server.load_image_with_settings("textures/brick.png", repeat_settings)
normal_tex = asset_server.load_image_with_settings("textures/brick_normal.png", repeat_settings)

# Combine with uv_transform to control tiling count
wall_mat = materials.add(StandardMaterial(
    base_color_texture=color_tex,
    normal_map_texture=normal_tex,
    uv_transform=Affine2.from_scale(Vec2(4.0, 2.0)),  # 4x2 tiles
    perceptual_roughness=0.85,
))
```

**Key points:**
- `asset_server.load_image_with_settings(path, settings)` - convenience for images
- `asset_server.load_with_settings(path, Image, settings)` - generic version (same result)
- `ImageAddressMode.Repeat` makes the texture repeat when UVs exceed `[0, 1]`
- `ImageAddressMode.MirrorRepeat` - repeats but mirrors every other tile (avoids visible seams)
- `mipmap_filter`, LOD clamps, and camera `MipBias` only affect images with a
  mip chain. PNG/JPEG and programmatic `Image` values do not generate one
  automatically; load a pre-mipmapped DDS or KTX2 texture when minification
  filtering matters.
- `ImageAddressMode.ClampToEdge` - default, stretches edge pixels
- `uv_transform` scales UVs but does **not** change the sampler - you need both for tiling
- Create `repeat_settings` once and reuse for all textures in the same material (color, normal, depth)

| Address Mode | Effect | Use case |
|---|---|---|
| `ClampToEdge` (default) | Stretches edge pixels | Single-use textures, skyboxes |
| `Repeat` | Tiles seamlessly | Floors, walls, terrain |
| `MirrorRepeat` | Tiles with mirroring | Reduces visible seam lines |
| `ClampToBorder` | Shows border color outside UV | Special effects |

## Double-Sided Rendering

For thin geometry (leaves, paper, curtains) that should be visible from both sides:

```python
foliage = materials.add(StandardMaterial(
    base_color=Color.srgb(0.2, 0.5, 0.1),
    double_sided=True,    # Normals flip for back faces
    cull_mode=None,        # Render both faces (default culls back faces)
))
```

| Setting | When to use |
|---------|------------|
| `double_sided=True, cull_mode=None` | Thin geometry visible from both sides (leaves, flags) |
| `double_sided=False` (default) | Solid closed meshes (cubes, spheres) |

## Rendering Method

Controls whether the material uses forward or deferred rendering:

```python
from pybevy.camera import DeferredPrepass, DepthPrepass
from pybevy.material import OpaqueRendererMethod

# Every camera that renders a forced-deferred material needs a deferred phase.
commands.spawn(
    Camera3d(),
    Msaa.Off,
    DepthPrepass(),
    DeferredPrepass(),
)

# Force deferred for complex lighting scenes
mat = materials.add(StandardMaterial(
    opaque_render_method=OpaqueRendererMethod.Deferred,
))
```

| Method | When to use |
|--------|------------|
| `Auto` (default) | Let Bevy decide |
| `Forward` | Transparency, MSAA, simple scenes |
| `Deferred` | Many lights, complex post-processing |

Do not force a material to `Deferred` unless every camera that can render it
has `DeferredPrepass()`. Bevy 0.19 expects the deferred render phase to exist
when it queues that material; omitting the camera component can panic the
renderer. `DepthPrepass()` and `Msaa.Off` complete the normal deferred-camera
setup. For an app-wide choice, prefer
`DefaultOpaqueRendererMethod.deferred()`, which selects deferred rendering for
`Auto` materials. Camera prepass markers are still required; the SSR component
adds them automatically, but an ordinary deferred camera does not. See
`guide://camera`.

**For all parameters:** `get_type_definition('StandardMaterial')`
