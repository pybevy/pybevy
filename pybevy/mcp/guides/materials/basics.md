# Material Basics

Metallic-roughness PBR workflow: the two most important parameters for realistic surfaces.

## Metallic vs Non-Metallic

| `metallic` | What it means | Examples |
|------------|---------------|---------|
| `0.0` | Dielectric (non-metal) — reflects environment at grazing angles only | Plastic, wood, stone, skin |
| `1.0` | Metal — reflects colored light at all angles | Gold, copper, steel, aluminum |
| `0.0–1.0` | Blend — useful for worn/oxidized metals | Rusty iron, tarnished silver |

**Rule of thumb:** Real materials are almost always 0.0 or 1.0. Values between indicate mixed surfaces (dirt on metal, etc.).

### Metallic Visual Thresholds

| `metallic` | `perceptual_roughness` | Result |
|------------|----------------------|--------|
| 0.0 | 0.7–0.9 | Matte plastic, rubber, concrete |
| 0.0 | 0.3–0.5 | Smooth plastic, varnished wood |
| 0.3–0.5 | 0.3–0.4 | Semi-metallic (ground/floor — adds depth vs flat matte) |
| 0.8 | 0.15–0.25 | Polished metal |
| 0.9 | 0.1–0.15 | Shiny metal (sweet spot for "metal but not mirror") |
| 0.95+ | 0.05 | Mirror-like chrome |

The jump from 0.8 to 0.95 is dramatic. Below 0.8 looks semi-metallic; above 0.95 looks like chrome.

## Roughness

`perceptual_roughness` controls how sharp reflections are:

| Value | Look | Use for |
|-------|------|---------|
| `0.0` | Mirror-sharp reflections | Chrome, still water, polished marble |
| `0.1–0.3` | Soft reflections, highlights visible | Polished metal, varnished wood |
| `0.4–0.6` | Diffuse highlights | Brushed metal, satin finish, skin |
| `0.7–0.9` | Almost no visible reflections | Concrete, rough wood, fabric |
| `1.0` | Completely diffuse | Chalk, raw clay, matte rubber |

## Reflectance

`reflectance` controls how much light dielectrics reflect at normal incidence (looking straight on). Default `0.5` = 4% Fresnel, correct for most materials.

| Value | F0 | Material |
|-------|-----|---------|
| `0.35` | ~2% | Water |
| `0.5` | ~4% | Most dielectrics (glass, plastic, gemstone) |
| `1.0` | ~16% | Very reflective dielectric (diamond-like) |

**Tip:** Only change reflectance for specific materials. The default 0.5 works for almost everything.

## Examples

### Shiny Metal (Gold)

```python
gold = materials.add(StandardMaterial(
    base_color=Color.srgb(1.0, 0.84, 0.0),
    metallic=1.0,
    perceptual_roughness=0.2,
))
```

For metals, `base_color` tints the reflections. Gold reflects yellow light, copper reflects orange.

### Matte Plastic

```python
plastic = materials.add(StandardMaterial(
    base_color=Color.srgb(0.8, 0.1, 0.1),  # Red plastic
    metallic=0.0,
    perceptual_roughness=0.7,
))
```

### Polished Wood

```python
wood = materials.add(StandardMaterial(
    base_color=Color.srgb(0.55, 0.35, 0.15),
    metallic=0.0,
    perceptual_roughness=0.35,
    reflectance=0.5,
))
```

Wood with a clear coat finish (varnished table) — use `clearcoat` instead. See `guide://materials/advanced`.

## Base Color Reference

RGB values that produce natural-looking results under standard two-directional lighting:

| Surface | `base_color` RGB | `metallic` | `perceptual_roughness` |
|---------|-----------------|----------|-----------|
| Stone/concrete | `(0.35–0.45, 0.33–0.42, 0.30–0.38)` | 0.0 | 0.7–0.8 |
| Dark wood | `(0.25, 0.15, 0.08)` | 0.0 | 0.5–0.7 |
| Light wood | `(0.55, 0.40, 0.25)` | 0.0 | 0.4–0.6 |
| Ground/floor | `(0.10–0.15, 0.10–0.15, 0.12–0.18)` | 0.3–0.5 | 0.3–0.4 |
| Grass | `(0.15–0.25, 0.35–0.50, 0.10–0.20)` | 0.0 | 0.7–0.9 |
| Sand | `(0.55–0.65, 0.50–0.58, 0.35–0.42)` | 0.0 | 0.8–0.9 |
| Water | `(0.10, 0.25, 0.45)` | 0.0 | 0.1 + transmission |
| Gold | `(0.83, 0.68, 0.22)` | 0.95 | 0.1 |
| Chrome | `(0.70, 0.72, 0.75)` | 0.98 | 0.05 |

**Ground/floor tip:** Slight metallic (0.3–0.5) with low roughness (0.3) gives depth. Pure `metallic=0.0, roughness=1.0` looks like construction paper.

## Unlit Materials

Set `unlit=True` to skip PBR lighting entirely. The material renders at its `base_color` brightness with no shading.

```python
star_mat = materials.add(StandardMaterial(
    base_color=Color.srgb(1.0, 0.95, 0.8),
    unlit=True,
))
```

Use for: stars, skybox stand-ins, LED screens, debug visualization, any surface where you don't want lighting.

## Alpha Blending (Transparency)

By default, materials are fully opaque. Use `alpha_mode` for transparency:

```python
# Translucent material (e.g., glass, atmosphere shell, ghost effect)
glass = materials.add(StandardMaterial(
    base_color=Color.srgba(0.2, 0.4, 1.0, 0.1),  # alpha < 1.0
    alpha_mode=AlphaMode.Blend(),
))

# Alpha cutoff (e.g., foliage, chain-link fence)
leaves = materials.add(StandardMaterial(
    base_color_texture=asset_server.load_image("textures/leaf.png"),
    alpha_mode=AlphaMode.Mask(0.5),  # pixels below 0.5 alpha are discarded
))
```

| Mode | Use for |
|------|---------|
| `AlphaMode.Opaque()` | Default — fully solid |
| `AlphaMode.Blend()` | Smooth transparency (glass, water, ghosts) |
| `AlphaMode.Mask(threshold)` | Binary cutoff (foliage, fences, decals) |
| `AlphaMode.Add()` | Additive blending (particles, laser beams) |
| `AlphaMode.Multiply()` | Darkening blend (shadows, tinted glass) |

## Double-Sided Rendering

By default, back faces are culled. Set `cull_mode=None` for double-sided materials:

```python
leaf = materials.add(StandardMaterial(
    base_color=Color.srgb(0.2, 0.5, 0.1),
    cull_mode=None,           # render both sides
    double_sided=True,        # flip normals on back face for correct lighting
))
```

Use for: foliage, cloth, paper, thin shells, atmosphere layers.

## ClearColor (Background)

Not a material, but commonly needed alongside materials. Set the scene background color:

```python
# In @entrypoint or via commands:
app.insert_resource(ClearColor(Color.BLACK))          # space scenes
app.insert_resource(ClearColor(Color.srgb(0.5, 0.7, 1.0)))  # sky blue
```

Default is dark gray. For space scenes, night scenes, or custom skyboxes, set `ClearColor(Color.BLACK)`.

**For all parameters:** `get_type_definition('StandardMaterial')`
