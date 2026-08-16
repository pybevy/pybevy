# Materials Guide

Creating realistic surfaces with `StandardMaterial`. Read this overview first, then drill into sub-guides for specifics.

## Quick Recipe Table

One-liner materials for common surfaces:

| Surface | Key kwargs |
|---------|-----------|
| Gold | `metallic=1.0, perceptual_roughness=0.3, base_color=Color.srgb(1.0, 0.84, 0.0)` |
| Chrome | `metallic=1.0, perceptual_roughness=0.05, base_color=Color.srgb(0.95, 0.95, 0.95)` |
| Glass | `specular_transmission=1.0, ior=1.5, thickness=0.5, perceptual_roughness=0.0, alpha_mode=AlphaMode.Opaque()` |
| Marble | `metallic=0.0, perceptual_roughness=0.15, base_color=Color.srgb(0.95, 0.93, 0.88)` |
| Wood | `metallic=0.0, perceptual_roughness=0.6, base_color=Color.srgb(0.55, 0.35, 0.15)` |
| Rubber | `metallic=0.0, perceptual_roughness=0.95, base_color=Color.srgb(0.15, 0.15, 0.15)` |
| Neon glow | `emissive=LinearRgba.rgb(10.0, 0.0, 5.0)` - requires Bloom on camera |
| Car paint | `metallic=0.8, perceptual_roughness=0.3, clearcoat=1.0, clearcoat_perceptual_roughness=0.1` |
| Water | `specular_transmission=0.9, ior=1.33, perceptual_roughness=0.0, base_color=Color.srgb(0.3, 0.5, 0.7), alpha_mode=AlphaMode.Opaque()` |

## Which Sub-Guide Do I Need?

| I want to... | Read |
|--------------|------|
| Set metallic/roughness, understand reflectance | `guide://materials/basics` |
| Make glass, water, backlit leaves | `guide://materials/transmission` |
| Use clearcoat, anisotropy, normal maps, parallax, tiling | `guide://materials/advanced` |
| Make objects glow (emissive + bloom) | `guide://lighting` (Emissive Materials section) |
| Make transparent objects | `guide://lighting` (Transparency section) |

## Minimal Example

```python
from pybevy.prelude import *

mat = materials.add(StandardMaterial(
    base_color=Color.srgb(1.0, 0.84, 0.0),
    metallic=1.0,
    perceptual_roughness=0.3,
))
commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat))
```

Emissive values interact with camera Bloom settings. See `guide://lighting` (Emissive Materials section) for intensity tiers and bloom pairing guidance.

**For all parameters:** `get_type_definition('StandardMaterial')`
