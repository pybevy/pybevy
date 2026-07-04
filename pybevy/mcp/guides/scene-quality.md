# Scene Quality Guide

Composition, lighting, color, and common mistakes that produce dark or flat results. Consult when refining visuals — not necessarily before writing the first line of code.

## Lighting Quick-Reference

For paste-ready lighting recipes by scene type (outdoor, interior, cave, industrial, moody), see `guide://lighting` (Recipes section). Always start bright, dim later.

### Minimum Brightness Floors (non-negotiable)

| Setting | Outdoor | Interior/Cave | Game Level |
|---------|---------|---------------|------------|
| GlobalAmbientLight brightness | 300 | 500 | 400 |
| Key light illuminance | 8000 | N/A (use PointLights) | 6000 |
| PointLight intensity (interior) | — | 150,000 | 100,000 |
| base_color (walls, floors, platforms) | 0.20 | 0.25 | 0.20 |
| base_color (darkest element) | 0.10 | 0.15 | 0.10 |
| DistanceFog density max | 0.005 | 0.004 | 0.006 |

**Never set base_color below 0.08** — it's indistinguishable from black.

**Dark/moody exceptions:** For intentionally dark scenes (galleries, space, underwater), ambient can go as low as 100 if emissive sources provide sufficient readability. The key test: can you distinguish midground subjects from background?

### Two-Light Setup (eliminates dark-side problem)

Every scene needs at minimum a **key light** (shadows) + **fill light** (~40% intensity, no shadows, opposite direction). See `guide://lighting` (Default Lighting Setup) for code.

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
- **60% Dominant** — overall mood (blue cave, green forest, warm stone interior)
- **30% Secondary** — complementary or analogous hue for key features
- **10% Accent** — contrasting pop color for focal points (emissive glow, character)

### Recommended Pairings

| Scene mood | Dominant (60%) | Secondary (30%) | Accent (10%) |
|------------|---------------|-----------------|--------------|
| Forest | Green (0.3, 0.42, 0.25) | Warm brown (0.45, 0.28, 0.12) | Amber glow (emissive 3.0, 1.5, 0.3) |
| Cave | Dark blue-gray (0.15, 0.18, 0.25) | Purple rock (0.25, 0.15, 0.30) | Cyan crystal (emissive 0.5, 2.0, 3.0) |
| Village | Warm stone (0.50, 0.45, 0.38) | Dark wood (0.30, 0.18, 0.08) | Orange torch (emissive 4.0, 2.0, 0.5) |
| Sci-fi | Cool gray (0.20, 0.22, 0.28) | Teal (0.10, 0.25, 0.30) | Hot pink (emissive 5.0, 0.5, 3.0) |
| Sunset | Warm amber (0.55, 0.35, 0.15) | Cool shadow blue (0.15, 0.18, 0.30) | White highlight (0.90, 0.88, 0.85) |

### Material Variation Rule
Use 2-3 shades within each hue family. Never use one color for all surfaces of a type:
```python
# Stone family — 3 variants
stone_light = materials.add(StandardMaterial(base_color=Color.srgb(0.50, 0.45, 0.38)))
stone_mid   = materials.add(StandardMaterial(base_color=Color.srgb(0.40, 0.36, 0.30)))
stone_dark  = materials.add(StandardMaterial(base_color=Color.srgb(0.30, 0.27, 0.22)))
```

---

## Camera Placement by Scene Type

**Default is eye-level, NOT overhead.** The patterns guide template uses `(8, 3, 8)` looking at `(0, 1.5, 0)`.

| Scene type | Camera Y | Looking at Y | Notes |
|------------|----------|-------------|-------|
| Nature / landscape | 2–4 | 1–2 | Low wide angle emphasizes horizon |
| Village / architecture | 3–5 | 1.5–2.5 | Slightly above eye level, looking through scene |
| Interior | 1.5–2.0 | 1.2–1.5 | Eye height, looking across room |
| Game level / platformer | Character height | Horizontally across level | Side-scroller feel |
| Atmospheric / mood | 1.5–3 | 1–2 | Low = more immersive |
| Top-down / strategy | 15–25 | 0 | Only when explicitly a god-view game |

**Distance:** Close enough that the main subject fills 30–50% of the frame width. If the subject is tiny, move closer.

---

## Composition Rules

### Three Depth Layers (mandatory)
Every scene MUST have:
1. **Foreground** (0–3m from camera) — framing elements, nearby props, ground detail
2. **Midground** (3–10m) — main subject matter, key architecture, characters
3. **Background** (10m+) — distant context, walls, terrain, fog fade

If the background is pure black, add a back wall, distant terrain, or set fog color above pure black.

### Element Placement
- **Accents:** each platform/surface gets AT MOST one decorative accent. Leave some bare.
- **Clusters of 2–3, not 1 or 10.** Group decorations in small intentional clusters.
- **Anchored, not floating.** Decorations grow FROM surfaces — never hover unless explicitly magical.
- **Varied repetition.** N copies of something → vary at least 2 of: scale (0.6x–1.4x), rotation, color tint.

### Prompt Fidelity
Before writing code, extract every noun from the user's prompt:
- Each noun → at least one entity
- "lake" → water plane MUST exist
- "village" → multiple buildings MUST exist
- Style references (e.g., "Silksong style") → research and include 3+ visual traits

### Guide Reading
Start with `guide://patterns` + one matching recipe. Read topic guides (lighting, materials, shadows, etc.) iteratively as you add those features — not all upfront.

---

## Scene-Type Checklists

### Outdoor / Nature
- [ ] `Atmosphere.earth()` for sky (no flat gray)
- [ ] Ground plane large enough that edges aren't visible (>= 40x40)
- [ ] Trees/rocks at perimeter with scale variation (0.7x–1.5x)
- [ ] At least one water feature if pastoral
- [ ] Vegetation gradient: bushes near structures, trees at edges
- [ ] DistanceFog density 0.002–0.005

### Architecture / Interior
- [ ] Multiple PointLights (4+) to eliminate dark corners
- [ ] DirectionalLight as fill only (shadow_maps_enabled=False)
- [ ] Material base_colors 0.25+ (brighter than outdoor — no sky bounce)
- [ ] Furniture/props to define spaces
- [ ] Bloom intensity low (0.05–0.12)

### Game Level / Platformer
- [ ] Camera at character height, looking horizontally
- [ ] Platforms form a CLEAR PATH — eye follows the route
- [ ] Back wall or backdrop to frame the level
- [ ] Accent lighting marks the path (glow near platforms, point lights at waypoints)
- [ ] Platform surfaces clean — minimal clutter on playable areas

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
See `examples/unsorted/procedural_terrain_visual.py` for a complete example.

### Procedural Textures (No More Flat Colors)
Generate pixel data with `Image.new_fill()`:
```python
import numpy as np

pixels = np.zeros((256, 256, 4), dtype=np.uint8)
# Fill with noise/pattern...
texture = Image.new_fill(width=256, height=256, pixel=list(pixels.flatten()),
    format=TextureFormat.Rgba8UnormSrgb, asset_usage=RenderAssetUsages.all())
mat = StandardMaterial(base_color_texture=images.add(texture))
```

### Vertex Colors (Gradient a Single Mesh)
```python
wall = Cuboid(4.0, 3.0, 0.3).mesh().build()
with wall.attribute(Mesh.ATTRIBUTE_POSITION) as positions:
    colors = np.zeros((len(positions), 4), dtype=np.float32)
    for i, pos in enumerate(positions):
        y_norm = (pos[1] + 1.5) / 3.0
        colors[i] = [0.4 + 0.4 * y_norm, 0.35 + 0.35 * y_norm, 0.3 + 0.2 * y_norm, 1.0]
wall.insert_attribute(Mesh.ATTRIBUTE_COLOR, colors)
```

### Texture Tiling / UV Control
```python
StandardMaterial(
    base_color_texture=texture,
    uv_transform=Affine2.from_scale(Vec2(4.0, 4.0)),  # Tile 4x
)
```
Manual UV manipulation: `mesh.uvs_mut()` returns a numpy-compatible context manager.

### Irregular Rocks (Not Just Spheres)
```python
rock = Sphere(1.0).mesh().ico(3)
with rock.positions_mut() as positions:
    for i in range(len(positions)):
        length = np.sqrt(np.sum(positions[i] ** 2))
        if length > 0:
            normal = positions[i] / length
            displacement = math.sin(positions[i][0] * 3.0) * 0.15 + math.cos(positions[i][1] * 5.0) * 0.1
            positions[i] += normal * displacement
```

### Soft Shadows
```python
commands.spawn(Camera3d(), ShadowFilteringMethod.TEMPORAL)  # or .GAUSSIAN
```

### Water / Glass Materials
```python
water = StandardMaterial(
    base_color=Color.linear_rgba(0.1, 0.3, 0.5, 0.8),
    specular_transmission=0.6, ior=1.33,
    perceptual_roughness=0.1, alpha_mode=AlphaMode.Blend(),
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
    ClusteredDecal(base_color_texture=asset_server.load("textures/moss.png")),
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

Prefer `Visibility.set_hidden()` over despawn for object pooling. Toggling visibility on hundreds of entities has zero measurable FPS cost — the GPU simply skips hidden entities. Pre-spawn objects, hide them at startup, and show/hide as needed.

## Actually Missing Features

These genuinely don't exist in PyBevy:

| Feature | Best workaround |
|---------|----------------|
| Area / rect lights | Multiple PointLights in a line |
| Trail / ribbon renderer | Chain of small meshes updated per frame |
| 3D billboard component | Orient quads toward camera in Update system |
| Gizmos / debug lines | Use MCP `capture_screenshot {"gizmos": true}` for labels |
| GPU instancing | Individual entities with Python loop variation |
