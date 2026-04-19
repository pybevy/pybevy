# Advanced Materials

Clearcoat, anisotropy, normal maps, parallax mapping, UV tiling, texture loading with repeat/sampler settings, and rendering method control.

## Clearcoat (Car Paint / Wet Surfaces)

Adds a second specular layer on top of the base material — simulates lacquer, varnish, or water film.

| Parameter | What it does | Range |
|-----------|-------------|-------|
| `clearcoat` | Strength of the coat layer | `0.0–1.0` |
| `clearcoat_perceptual_roughness` | Smoothness of the coat | `0.0–1.0` |

```python
# Car paint — metallic base with smooth clear coat
car_paint = materials.add(StandardMaterial(
    base_color=Color.srgb(0.7, 0.0, 0.0),
    metallic=0.8,
    perceptual_roughness=0.4,        # Slightly rough base
    clearcoat=1.0,
    clearcoat_perceptual_roughness=0.1,  # Very smooth coat
))

# Wet stone — rough base with glossy water layer
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

Makes reflections stretch in one direction — for brushed metal, hair, silk.

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

Use `anisotropy_texture` for spatially varying grain direction (e.g., a vinyl record).

## Normal Maps

Add surface detail without extra geometry. The texture encodes per-pixel surface normals.

```python
brick_wall = materials.add(StandardMaterial(
    base_color_texture=asset_server.load("bevy/textures/brick_color.png"),
    normal_map_texture=asset_server.load("bevy/textures/brick_normal.png"),
    perceptual_roughness=0.7,
))
```

**`flip_normal_map_y`**: Set to `True` if your normal map uses OpenGL convention (green channel points up). Bevy expects DirectX convention by default.

**Tangents required:** Normal maps need tangent data on the mesh. Primitive meshes (Cuboid, Sphere, etc.) include tangents by default. For custom meshes, call `mesh.generate_tangents()` after setting positions, normals, UVs, and indices — otherwise normal mapping will look wrong.

## Parallax Mapping

Adds depth illusion to flat surfaces using a height/depth map.

```python
cobblestone = materials.add(StandardMaterial(
    base_color_texture=asset_server.load("bevy/textures/cobble_color.png"),
    normal_map_texture=asset_server.load("bevy/textures/cobble_normal.png"),
    depth_map=asset_server.load("bevy/textures/cobble_depth.png"),
    parallax_depth_scale=0.05,       # Depth intensity (keep low to avoid artifacts)
    max_parallax_layer_count=16.0,   # Quality (higher = better but slower)
))
```

**Tip:** Keep `parallax_depth_scale` at `0.02–0.08`. Higher values cause visible swimming artifacts at steep angles.

## UV Transform (Tiling)

Scale, rotate, or offset texture coordinates with `uv_transform`:

```python
from pybevy.math import Affine2

# Tile the texture 4x4
tiled_floor = materials.add(StandardMaterial(
    base_color_texture=asset_server.load("bevy/textures/tile.png"),
    uv_transform=Affine2.from_scale(Vec2(4.0, 4.0)),
))
```

## Texture Loading with Repeat Sampler

By default, Bevy textures clamp to edge — UV coordinates outside `[0, 1]` repeat the edge pixel. To make textures tile (repeat), load them with `ImageAddressMode.Repeat`:

```python
from pybevy.image import (
    ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
    ImageAddressMode,
)
from pybevy.math import Affine2

# Create repeat sampler settings (reuse for all tiling textures)
repeat_settings = ImageLoaderSettings(sampler=ImageSampler.descriptor(
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
- `asset_server.load_image_with_settings(path, settings)` — convenience for images
- `asset_server.load_with_settings(path, Image, settings)` — generic version (same result)
- `ImageAddressMode.Repeat` makes the texture repeat when UVs exceed `[0, 1]`
- `ImageAddressMode.MirrorRepeat` — repeats but mirrors every other tile (avoids visible seams)
- `ImageAddressMode.ClampToEdge` — default, stretches edge pixels
- `uv_transform` scales UVs but does **not** change the sampler — you need both for tiling
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

**For all parameters:** `get_type_definition('StandardMaterial')`
