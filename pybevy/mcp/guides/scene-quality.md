# Scene Quality Guide

Composition, lighting, color, and common mistakes that produce dark or flat results. Consult when refining visuals - not necessarily before writing the first line of code.

NumPy examples require the optional dependency: install `pybevy[numpy]` in the
scene's Python environment.

## Choose the Look Before the Settings

Quality means executing the requested style well. Realism is one possible goal;
effects, detail, brightness and smoothing serve whichever style is chosen.
Translate the request or reference into a short visual brief: dominant shapes,
palette, contrast, surface treatment, camera/projection, and motion. When the
style is unspecified, choose a coherent direction and state the assumption.

| Intended look | Useful choices | Preserve when intentional |
|---|---|---|
| Realistic / product | Detailed silhouettes, bevels, PBR maps, environment lighting, controlled exposure | Material differences and believable light direction |
| Low-poly / graphic | Faceted geometry, deliberate flat shading, simple materials and clear silhouettes | Large color areas and visible facets |
| Pixel art / retro | Deliberate render resolution, nearest texture filtering, pixel-aligned presentation where appropriate | Crisp pixel structure |
| Toon / illustration | Designed color regions, stylized shading or custom shaders, selective outlines | Graphic contrast and simplified light response |
| Noir / silhouette | Restricted palette, strong shape separation, selective illumination | Deep blacks and intentionally hidden detail |
| Dreamlike / emissive | Selected glow, atmospheric depth, controlled color and motion | Mood without losing intended focal points |

Treat these as directions. Establish a small representative scene first.
Compare at the intended delivery size and, for animation, in motion. Adjust the
largest mismatch to the visual brief before adding detail. A histogram reveals
clipping or a blank frame; judge the style itself by eye.

## Optional Realistic Rendering Workflow

For a realistic brief, start with geometry, materials, light, and composition:
silhouettes, bevels, texture detail and indirect lighting come from the scene,
and post-processing finishes it. These profiles are starting points:

| Profile | Camera and rendering choices | Tradeoff |
|---|---|---|
| Clean forward, including glass/custom shaders | Explicit `Hdr`, exposure and tonemapping; MSAA4 or MSAA8; Gaussian shadows; environment lighting | Stable edges; assess contact shadows separately |
| Opaque scene with ambient occlusion | Forward rendering, SSAO High/Ultra, MSAA off; larger render target followed by downsampling | Better grounding; extra pixels needed to reduce edge aliasing |
| Local screen-space reflections | Deferred opaque rendering and SSR, MSAA off | Screen-space coverage and noise limits; compare repeated frames before choosing |

Read `guide://camera` for HDR, exposure, SSAO/SSR and AA compatibility;
`guide://lighting` for prefiltered environment maps, local probes and baked GI;
`guide://materials/advanced` for linear data textures, tangents and mipmaps;
and `guide://shadows` for resolution, cascades and contact-shadow caveats.

Use a scene-matched environment map for reflective surfaces. Generate mesh
tangents for normal maps and anisotropy. Match shadow cascade distances to the
subject before increasing map size. Choose exposure with the light intensities,
and set material colors for the material itself.

For smooth high-detail stills, try rendering at twice the delivery width and
height, then downsample once: four times the pixels. This can improve shader
and specular detail beyond what MSAA resolves. Keep the native resolution for a
deliberate pixel grid. See `guide://headless`
for capture resolution and repeat-frame checks. Baked indirect light, authored
assets, and more deliberate art direction can raise quality beyond these presets.

## Going Beyond Built-In Rendering Presets

A polished scene may combine native components with authored assets and custom
rendering. Distinguish these levels when planning effort and explaining results:

- **Native settings:** PBR materials, exposure, environment lighting, cascaded
  shadows, SSAO, transmission, atmosphere and volumetrics. Select a compatible
  subset for the requested style before adding specialized code.
- **Asset and scene construction:** scanned or carefully authored surfaces,
  bevels and silhouette detail, varied vegetation, coherent scale, local light
  placement, camera paths, and composition.
- **Custom rendering:** planar reflection cameras, surface reconstruction,
  procedural water, depth-aware scattering, approximate indirect-light transport,
  or temporal image filtering. These need dedicated implementation and
  validation.

For a flat mirror or water plane, a reflected camera rendering to an image can
capture geometry absent from the main view, unlike SSR. This costs another view
per plane. Exclude the reflective surface from its own capture, synchronize the
camera pose, clip geometry below the reflection plane, and keep HDR/tonemapping
consistent. Complex water then uses the reflection in its shader; this is an
advanced multi-camera technique.

Keep deformed geometry consistent across color, depth, shadows and any motion
inputs. A wind shader that moves leaves only in the color pass produces stale
shadows and incorrect screen-space effects. Use transforms shared by all passes
or implement matching deformation in the relevant shader passes.

Review a finished frame and a motion segment alongside the list of enabled
features. Historical showcase scenes may use older APIs: verify compatibility
and preserve their original assets and recordings when adapting them.

## Lighting Quick-Reference

For paste-ready lighting recipes by scene type (outdoor, interior, cave, industrial, moody), see `guide://lighting` (Recipes section). Use their numbers as starting points for a readable, conventionally lit scene; adapt them to exposure, scale, and style.

### Lighting Starting Points

| Setting | Outdoor | Interior/Cave | Game Level |
|---------|---------|---------------|------------|
| GlobalAmbientLight brightness | 300 | 500 | 400 |
| Key light illuminance | 8000 | N/A (use PointLights) | 6000 |
| PointLight intensity (interior) | - | 150,000 | 100,000 |
| base_color (walls, floors, platforms) | 0.20 | 0.25 | 0.20 |
| base_color (darkest element) | 0.10 | 0.15 | 0.10 |
| DistanceFog density max | 0.005 | 0.004 | 0.006 |

Dark materials below 0.08 can be intentional and read well under suitable lighting. Evaluate the rendered subject and exposure.

**Fog density is scale-dependent:** the DistanceFog maxima above assume long outdoor
sightlines (100m+). For enclosed spaces ~50m across, choose a density in the
0.01-0.03 range (for example, `FogFalloff.Exponential(density=0.02)`); judge by
whether the far wall still reads, not by the number.

**Dark/moody scenes:** Ambient may be lower than these starting points, including zero. Judge whether the intended subjects or silhouettes remain readable.

### Key and Fill Option

A key plus fill is useful when both sides of a subject should read. Fill may come from an environment map, baked light, or another light; its ratio is artistic. Atmosphere scenes should use one sun and environment fill, as explained in `guide://lighting`. Unlit and silhouette styles need different setups.

### ClearColor Defaults

| Scene Type | ClearColor RGB | Notes |
|-----------|---------------|-------|
| Night/dark | `(0.02, 0.02, 0.03)` | Near-black with slight blue |
| Indoor/cave | `(0.01, 0.01, 0.02)` | Deeper black |
| Dusk/sunset | `(0.05, 0.03, 0.02)` | Warm dark brown |
| Underwater | `(0.02, 0.04, 0.06)` | Dark teal |
| 2D game | `(0.015, 0.015, 0.03)` | Very dark blue-black |

**Match ClearColor to fog color** to avoid visible seam at fog boundary.

---

## Color Palette Guidance

### The 60/30/10 Rule
- **60% Dominant** - overall mood (blue cave, green forest, warm stone interior)
- **30% Secondary** - complementary or analogous hue for key features
- **10% Accent** - contrasting pop color for focal points (emissive glow, character)

### Recommended Pairings

| Scene mood | Dominant (60%) | Secondary (30%) | Accent (10%) |
|------------|---------------|-----------------|--------------|
| Forest | Green (0.3, 0.42, 0.25) | Warm brown (0.45, 0.28, 0.12) | Amber glow (emissive 3.0, 1.5, 0.3) |
| Cave | Dark blue-gray (0.15, 0.18, 0.25) | Purple rock (0.25, 0.15, 0.30) | Cyan crystal (emissive 0.5, 2.0, 3.0) |
| Village | Warm stone (0.50, 0.45, 0.38) | Dark wood (0.30, 0.18, 0.08) | Orange torch (emissive 4.0, 2.0, 0.5) |
| Sci-fi | Cool gray (0.20, 0.22, 0.28) | Teal (0.10, 0.25, 0.30) | Hot pink (emissive 5.0, 0.5, 3.0) |
| Sunset | Warm amber (0.55, 0.35, 0.15) | Cool shadow blue (0.15, 0.18, 0.30) | White highlight (0.90, 0.88, 0.85) |

### Material Variation Rule
For organic variation, try 2-3 shades within a hue family. Keep uniform colors when the requested graphic style calls for them:
```python
# Stone family - 3 variants
stone_light = materials.add(StandardMaterial(base_color=Color.srgb(0.50, 0.45, 0.38)))
stone_mid   = materials.add(StandardMaterial(base_color=Color.srgb(0.40, 0.36, 0.30)))
stone_dark  = materials.add(StandardMaterial(base_color=Color.srgb(0.30, 0.27, 0.22)))
```

---

## Camera Placement by Scene Type

Choose the viewpoint for the requested composition. Eye-level is a useful starting point for immersive scenes; overhead, orthographic, and close product views are valid choices. The patterns guide template uses `(8, 3, 8)` looking at `(0, 1.5, 0)`.

| Scene type | Camera Y | Looking at Y | Notes |
|------------|----------|-------------|-------|
| Nature / landscape | 2–4 | 1–2 | Low wide angle emphasizes horizon |
| Village / architecture | 3–5 | 1.5–2.5 | Slightly above eye level, looking through scene |
| Interior | 1.5–2.0 | 1.2–1.5 | Eye height, looking across room |
| Game level / platformer | Character height | Horizontally across level | Side-scroller feel |
| Atmospheric / mood | 1.5–3 | 1–2 | Low = more immersive |
| Top-down / strategy | 15–25 | 0 | Strategy, diagrams, or an intentional overhead composition |

**Distance:** Close enough that the main subject fills 30–50% of the frame width. If the subject is tiny, move closer.

---

## Composition Rules

### Three Depth Layers (optional composition pattern)
For an immersive environment, consider:
1. **Foreground** (0–3m from camera) - framing elements, nearby props, ground detail
2. **Midground** (3–10m) - main subject matter, key architecture, characters
3. **Background** (10m+) - distant context, walls, terrain, fog fade

If unintended black space weakens the composition, add a backdrop or distant context. Keep black or flat backgrounds for styles that call for them.

### Element Placement
- **Accents:** each platform/surface gets AT MOST one decorative accent. Leave some bare.
- **Clusters of 2–3, not 1 or 10.** Group decorations in small intentional clusters.
- **Anchored, not floating.** Decorations grow FROM surfaces - never hover unless explicitly magical.
- **Varied repetition.** N copies of something → vary at least 2 of: scale (0.6x–1.4x), rotation, color tint.

### Prompt Fidelity
Before writing code, extract every noun from the user's prompt:
- Each noun → at least one entity
- "lake" → water plane MUST exist
- "village" → multiple buildings MUST exist
- Style references (e.g., "Silksong style") → research and include 3+ visual traits

### Guide Reading
Start with `guide://patterns` + one matching recipe. Read topic guides (lighting, materials, shadows, etc.) iteratively as you add those features - not all upfront.

---

## Optional Scene-Type Checklists

Select items that support the requested look. These lists describe
conventional environments.

### Outdoor / Nature
- [ ] `Atmosphere.earth(medium_handle)` on its own entity + `AtmosphereSettings()` on the camera for sky (no flat gray)
- [ ] Ground plane large enough that edges aren't visible (>= 40x40)
- [ ] Trees/rocks at perimeter with scale variation (0.7x–1.5x)
- [ ] Water or other landmarks when requested or useful to the composition
- [ ] Vegetation gradient: bushes near structures, trees at edges
- [ ] DistanceFog density 0.002–0.005

### Architecture / Interior
- [ ] Lights placed at intended fixtures; fill or indirect light where needed
- [ ] Fill appropriate to the room, kept inside its walls
- [ ] Exposure and indirect lighting preserve the intended material colors
- [ ] Furniture/props to define spaces
- [ ] Bloom intensity low (0.05–0.12)

### Game Level / Platformer
- [ ] Camera at character height, looking horizontally
- [ ] Platforms form a CLEAR PATH - eye follows the route
- [ ] Back wall or backdrop to frame the level
- [ ] Accent lighting marks the path (glow near platforms, point lights at waypoints)
- [ ] Platform surfaces clean - minimal clutter on playable areas

### Atmospheric / Mood
- [ ] Midground subjects distinguishable from background (>= 20% brightness difference)
- [ ] Emissive elements as primary visual anchors
- [ ] ClearColor matches fog color (no color discontinuity)
- [ ] Bloom intensity 0.15–0.35

---

## Environmental Detail Lists by Scene Type

Add these "lived-in" details to transform scenes from tech demos to places.

### Forest / Nature
- Fallen logs (rotated cylinders, half-buried)
- Moss-covered rocks (green-tinted spheres with `positions_mut()` displacement)
- Mushroom clusters (cone + cylinder, groups of 2–3, varied scale)
- Broken branches (thin cylinders at angles)
- Ground cover (small flattened spheres scattered near bases)

### Village / Town
- Fences between properties (thin cuboid rails + cylinder posts)
- Stacked crates/barrels near buildings (cuboids + cylinders, groups of 2–4)
- Cart or wheelbarrow (cuboid body + torus wheels)
- Woodpile against a wall (stacked thin cuboids)
- Well (cylinder + torus rim + thin cylinder rope)
- Paths connecting buildings (lighter cuboid strips, y=0.02)

### Cave / Dungeon
- Stalagmites/stalactites (cones, varied height 0.5–3.0)
- Loose rocks (displaced icospheres via `Sphere().mesh().ico(3)` + `positions_mut()`)
- Glowing crystals (emissive cones or cuboids, 2–3 per cluster)
- Puddles (flat dark cylinders, metallic=0.9, roughness=0.05)
- Cobwebs (ultra-thin cuboids at corners, alpha=0.3)

### Interior / Room
- Tables and chairs (cuboid surfaces + cylinder legs)
- Shelving (cuboid frame + small cuboid items)
- Rugs (flat cuboid, different material from floor)
- Wall-mounted items (thin cuboids as paintings, cuboid+cylinder as torches)
- Scattered items on surfaces (small cuboids, spheres for pottery)

---

## Features That Exist (Common Misconceptions)

The analysis docs flagged these as missing, but they work in PyBevy. Use them.

### Procedural Terrain / Heightmap
Not limited to flat `Plane3d`. Build terrain with `Mesh.insert_attribute()`:
```python
import numpy as np

mesh = Mesh(PrimitiveTopology.TriangleList)
# Generate vertex grid with Y displacement
positions = np.zeros((res * res, 3), dtype=np.float32)
for z in range(res):
    for x in range(res):
        px = (x / (res - 1) - 0.5) * size
        pz = (z / (res - 1) - 0.5) * size
        py = math.sin(px * 0.3) * 2.0 + math.cos(pz * 0.2) * 1.5  # height function
        positions[z * res + x] = [px, py, pz]
mesh.insert_attribute(Mesh.ATTRIBUTE_POSITION, positions)
mesh.insert_attribute(Mesh.ATTRIBUTE_NORMAL, normals)
mesh.insert_indices(indices)
```
Full example: `examples/unsorted/procedural_terrain_visual.py` in the pybevy
source repo (examples are not shipped in the wheel).

### Procedural Textures (No More Flat Colors)
Build the pixel buffer, then hand it to `Image(...)`:
```python
import numpy as np
from pybevy.render import Extent3d

pixels = np.zeros((256, 256, 4), dtype=np.uint8)
# Fill with noise/pattern...
texture = Image(Extent3d(width=256, height=256, depth_or_array_layers=1), data=pixels)
mat = StandardMaterial(base_color_texture=images.add(texture))
```
`data` takes the natural `(height, width, bytes_per_pixel)` uint8 shape;
`format` defaults to `Rgba8UnormSrgb`.
The bounded `pybevy.array` also accepts one `-1` in `reshape`. Direct
out-of-range Python integer writes to a `uint8` array raise `OverflowError`;
float-to-`uint8` `astype` can wrap instead, so clamp brightness calculations
to `[0, 255]` before casting.

`Image.new_fill(Extent3d(width=64, height=64, depth_or_array_layers=1), [255, 0, 0, 255])` repeats a *single* pixel
across the whole texture: solid colours only.

A single-channel texture needs the format as well as the shape, and the shape
alone does not select it:

```python
from pybevy.render import Extent3d, TextureFormat

height = Image(
    Extent3d(width=32, height=32, depth_or_array_layers=1),
    data=np.zeros((32, 32), dtype=np.uint8),
    format=TextureFormat.R8Unorm,
)
```

`TextureFormat` is in `pybevy.render`; `ImageFormat` in `pybevy.image` holds
file formats and has no `R8Unorm`. Without `format=`, the default
`Rgba8UnormSrgb` rejects the buffer as the wrong length.

`image.convert(format)` follows Bevy 0.19's narrow conversion bridge. Sources
must use `R8Unorm`, `Rg8Unorm`, `Rgba8UnormSrgb`, `Bgra8UnormSrgb`, or
`Bgra8Unorm`; requested targets are limited to `R8Unorm`, `Rg8Unorm`, and
`Rgba8UnormSrgb`. A request for `R8Unorm` or `Rg8Unorm` returns an
`Rgba8Unorm` image with four bytes per pixel because Bevy expands luma data
back to RGBA. Unsupported source or target formats return `None`.

Unsupported image-encoding errors list the encoders enabled in this build.
Use one of those formats; membership in `ImageFormat` alone does not guarantee
that the build can encode it.

`image.save_to_file("shot.jpg")` infers JPEG from the extension when `format` is omitted or `None`. An explicit `ImageFormat` overrides the extension. A path with no extension writes PNG; an extension naming no supported encoder, such as `.avif` or `.txt`, raises `ValueError` without writing a file. Inferred formats retain normal encoder errors. `quality` controls JPEG encoding.

### Vertex Colors (Gradient a Single Mesh)
```python
wall = Cuboid(4.0, 3.0, 0.3).mesh().build()
positions = wall.positions().to_numpy()
y_norm = (positions[:, 1] + 1.5) / 3.0
colors = np.column_stack((
    0.4 + 0.4 * y_norm,
    0.35 + 0.35 * y_norm,
    0.3 + 0.2 * y_norm,
    np.ones_like(y_norm),
)).astype(np.float32)
wall.insert_attribute(Mesh.ATTRIBUTE_COLOR, colors)
```

### Texture Tiling / UV Control
```python
from pybevy.math import Affine2

StandardMaterial(
    base_color_texture=texture,
    uv_transform=Affine2.from_scale(Vec2(4.0, 4.0)),  # Tile 4x
)
```
Manual UV manipulation: `mesh.uvs_mut()` returns a numpy-compatible context manager.

### Irregular Rocks (Not Just Spheres)
```python
rock = Sphere(1.0).mesh().ico(3)
positions = rock.positions().to_numpy()
lengths = np.linalg.norm(positions, axis=1)
nonzero = lengths > 0
displacement = (
    np.sin(positions[:, 0] * 3.0) * 0.15
    + np.cos(positions[:, 1] * 5.0) * 0.1
)
positions[nonzero] += (
    positions[nonzero]
    * (displacement[nonzero] / lengths[nonzero])[:, None]
)
rock.set_positions(positions)
```

### Soft Shadows
```python
from pybevy.light import ShadowFilteringMethod

commands.spawn(Camera3d(), ShadowFilteringMethod.Gaussian)  # Stable without TAA
```

### Water / Glass Materials
```python
water = StandardMaterial(
    base_color=Color.linear_rgb(0.1, 0.3, 0.5),
    specular_transmission=0.6, ior=1.33,
    perceptual_roughness=0.1, alpha_mode=AlphaMode.Opaque(),
)
```

### Alpha-Masked Foliage
```python
StandardMaterial(
    base_color_texture=leaf_texture,
    alpha_mode=AlphaMode.Mask(0.5),
    double_sided=True, cull_mode=None,
)
```

### Decals
```python
commands.spawn(
    ClusteredDecal(base_color_texture=asset_server.load_image("textures/moss.png")),
    Transform.from_xyz(0.0, 0.01, 0.0),
)
```

### Audio (Spatial 3D Sound)
```python
commands.spawn(
    AudioPlayer(asset_server.load_audio("audio/ambient.ogg")),
    PlaybackSettings(mode=PlaybackMode.Loop, volume=Volume.Decibels(-10.0)),
)
```

---

## Performance Tip: Visibility Toggling

Prefer `Visibility.set_hidden()` over despawn for object pooling. Toggling visibility on hundreds of entities has zero measurable FPS cost - the GPU simply skips hidden entities. Pre-spawn objects, hide them at startup, and show/hide as needed.

## Features Requiring Custom Scene Logic

These patterns currently require scene code:

| Feature | Best workaround |
|---------|----------------|
| Trail / ribbon renderer | Chain of small meshes updated per frame |
| 3D billboard component | Orient quads toward camera in Update system |

Compatible entities sharing mesh and material handles use Bevy's automatic
batching and instancing. Reuse handles for repeated objects; see
`guide://performance`.
