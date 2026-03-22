# Transmission Materials

Making see-through and translucent surfaces: glass, water, backlit leaves, paper.

## Specular Transmission (Glass / Water)

Three parameters work together:

| Parameter | What it does | Typical range |
|-----------|-------------|---------------|
| `specular_transmission` | How see-through (0 = opaque, 1 = fully transparent) | `0.5–1.0` |
| `ior` | Index of refraction — how much light bends | See table below |
| `thickness` | Optical thickness for refraction/attenuation | `0.1–5.0` |

### IOR Reference

| Material | `ior` | `specular_transmission` | `thickness` | Visual |
|----------|-------|----------------------|-----------|--------|
| Air | `1.0` | — | — | No refraction |
| Water | `1.33` | 0.9 | 0.5 | Slight distortion, clear |
| Glass | `1.5` | 0.95 | 0.3 | Visible refraction, transparent |
| Crystal | `1.8` | 0.85 | 0.5 | Strong refraction, sparkle |
| Diamond | `2.42` | 0.8 | 0.4 | Dramatic refraction, rainbow edge hints |

**IOR differences** are subtle between 1.3–1.5 but dramatic above 2.0. **Thickness** controls background distortion: below 0.2 looks like flat transparency, above 0.5 produces noticeable distortion.

### Glass Sphere Example

```python
glass = materials.add(StandardMaterial(
    base_color=Color.srgba(0.9, 0.95, 1.0, 0.1),
    specular_transmission=1.0,
    ior=1.5,
    thickness=1.0,
    perceptual_roughness=0.0,     # Clear glass
    metallic=0.0,
    reflectance=0.5,
    alpha_mode=AlphaMode.Blend(),
))
commands.spawn(Mesh3d(sphere_mesh), MeshMaterial3d(glass))
```

**Important:** `alpha_mode=AlphaMode.Blend()` is required for specular transmission to render correctly.

### Frosted Glass

Increase roughness to scatter the transmission:

```python
frosted = materials.add(StandardMaterial(
    specular_transmission=0.8,
    ior=1.5,
    thickness=0.5,
    perceptual_roughness=0.4,     # Frosted look
    alpha_mode=AlphaMode.Blend(),
))
```

## Colored Glass (Attenuation)

Use `attenuation_color` and `attenuation_distance` to tint light passing through:

```python
wine_glass = materials.add(StandardMaterial(
    specular_transmission=1.0,
    ior=1.5,
    thickness=2.0,
    perceptual_roughness=0.0,
    attenuation_color=Color.srgb(0.6, 0.0, 0.1),  # Deep red tint
    attenuation_distance=1.0,                        # How far light travels before fully tinted
    alpha_mode=AlphaMode.Blend(),
))
```

Thicker regions appear more saturated. Good for stained glass, colored bottles, gemstones.

## Diffuse Transmission (Leaves / Paper)

`diffuse_transmission` scatters light through the surface. Unlike specular transmission, it doesn't distort — light just passes through diffusely.

| Material | `diffuse_transmission` |
|----------|----------------------|
| Thick leaf | `0.3` |
| Thin leaf / paper | `0.6` |
| Thin fabric (lampshade) | `0.5` |

### Backlit Leaf Example

```python
leaf = materials.add(StandardMaterial(
    base_color=Color.srgb(0.2, 0.5, 0.1),
    diffuse_transmission=0.5,
    double_sided=True,           # Light from both sides
    cull_mode=None,              # Render both faces
    perceptual_roughness=0.8,
    metallic=0.0,
))
```

**Tip:** Combine `double_sided=True` + `cull_mode=None` for thin geometry like leaves and paper, so both sides are lit and visible.

### TransmittedShadowReceiver

For shadows to appear on the backside of transmissive surfaces, add the `TransmittedShadowReceiver` component:

```python
from pybevy.light import TransmittedShadowReceiver

commands.spawn(
    Mesh3d(leaf_mesh),
    MeshMaterial3d(leaf_mat),
    TransmittedShadowReceiver(),
)
```

**For all parameters:** `get_type_definition('StandardMaterial')`
