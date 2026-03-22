# Shadows Guide

Shadow configuration, quality tuning, cascade setup, and common problems.

## Enabling Shadows

Shadows are off by default. Enable per-light:

```python
commands.spawn(
    DirectionalLight(illuminance=10000.0, shadows_enabled=True),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
)
commands.spawn(
    PointLight(intensity=80000.0, shadows_enabled=True),
    Transform.from_xyz(2.0, 3.0, 0.0),
)
```

## Shadow Map Resolution

Global resources control shadow map texture size. Higher = sharper shadows, more VRAM.

```python
commands.insert_resource(DirectionalLightShadowMap(size=4096))  # Default: 2048
commands.insert_resource(PointLightShadowMap(size=2048))        # Default: 1024
```

| Size | Quality | VRAM cost |
|------|---------|-----------|
| `512` | Low — visible stairstepping | Minimal |
| `1024` | Default for point lights | Low |
| `2048` | Default for directional lights, good for most scenes | Medium |
| `4096` | High quality — sharp shadows up close | High |

## Shadow Filtering

Controls how shadow edges look. Added to the camera entity:

```python
commands.spawn(
    Camera3d(),
    ShadowFilteringMethod.GAUSSIAN,  # Default — good without TAA
)
```

| Method | When to use |
|--------|------------|
| `HARDWARE_2X2` | Fast, hard shadow edges. Budget/mobile. |
| `GAUSSIAN` | Soft edges, good general-purpose (default) |
| `TEMPORAL` | Softest, randomized. Best with TAA enabled. |

## Bias Tuning

Two bias values prevent shadow artifacts. Each light type has its own defaults:

| Parameter | What it fixes | Default (Dir) | Default (Point) |
|-----------|--------------|---------------|-----------------|
| `shadow_depth_bias` | Shadow acne (moire patterns on lit surfaces) | `0.02` | `0.08` |
| `shadow_normal_bias` | Peter panning (shadow detached from object base) | `1.8` | `0.6` |

```python
# If you see shadow acne, increase depth_bias
DirectionalLight(shadows_enabled=True, shadow_depth_bias=0.05)

# If shadow floats away from object, decrease normal_bias
PointLight(shadows_enabled=True, shadow_normal_bias=0.3)
```

**Rule of thumb:** Increase `depth_bias` to fix acne; decrease `normal_bias` to fix peter panning. They trade off against each other — find the balance for your scene.

## Cascade Shadow Config (Directional Lights)

Directional lights use cascaded shadow maps — multiple shadow maps at different distances. Closer cascades have higher resolution.

```python
from pybevy.light import CascadeShadowConfig

# Default: 4 cascades
commands.spawn(
    DirectionalLight(illuminance=10000.0, shadows_enabled=True),
    CascadeShadowConfig(
        bounds=[10.0, 28.0, 78.0, 150.0],
        overlap_proportion=0.2,
        minimum_distance=0.1,
    ),
)
```

### What `bounds` Means

Each value is the maximum distance (in world units) that cascade covers:

| Cascade | Bounds `[10, 28, 78, 150]` | Coverage |
|---------|---------------------------|----------|
| 0 | 0–10 | Near objects — highest detail |
| 1 | 10–28 | Mid-range |
| 2 | 28–78 | Far |
| 3 | 78–150 | Very far — lowest detail |

### Scene-Specific Tuning

```python
# Small scene (tabletop, room interior)
CascadeShadowConfig(bounds=[3.0, 8.0, 20.0, 40.0])

# Large scene (outdoor landscape)
CascadeShadowConfig(bounds=[20.0, 60.0, 200.0, 500.0])

# Two cascades only (less GPU cost)
CascadeShadowConfig(bounds=[15.0, 50.0])
```

**Tip:** Fewer cascades = less GPU cost. Two cascades work well for small to medium scenes.

## Shadow Markers

Control which meshes cast and receive shadows:

```python
from pybevy.light import NotShadowCaster, NotShadowReceiver, TransmittedShadowReceiver

# This mesh doesn't cast shadows (e.g., transparent particle)
commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat), NotShadowCaster())

# This mesh doesn't receive shadows (e.g., skybox, UI element)
commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat), NotShadowReceiver())

# Receive shadows on the transmission side (backlit leaves)
commands.spawn(Mesh3d(leaf_mesh), MeshMaterial3d(leaf_mat), TransmittedShadowReceiver())
```

## Common Problems

### Shadow Acne (Moire Patterns)

**Symptoms:** Striped/moire pattern on lit surfaces, especially at grazing angles.
**Fix:** Increase `shadow_depth_bias` (try `0.05`–`0.1`). If artifacts remain, also slightly increase `shadow_normal_bias`.

### Peter Panning (Floating Shadows)

**Symptoms:** Shadow appears detached from the base of the object, floating slightly.
**Fix:** Decrease `shadow_normal_bias` (try `0.3`–`0.5`). Be careful not to go too low or acne returns.

### No Shadows Visible

**Checklist:**
1. `shadows_enabled=True` on the light?
2. Light is actually illuminating the area? (check `range` for point/spot)
3. Camera is within cascade bounds? (increase `CascadeShadowConfig` bounds)
4. Object isn't marked `NotShadowCaster()`?

### Shadows Too Pixelated

**Fix:** Increase shadow map resolution and/or tighten cascade bounds:
```python
commands.insert_resource(DirectionalLightShadowMap(size=4096))
CascadeShadowConfig(bounds=[5.0, 15.0, 40.0, 80.0])  # Tighter = more pixels per unit
```

**For all parameters:** `get_type_definition('DirectionalLight')`, `get_type_definition('PointLight')`, `get_type_definition('CascadeShadowConfig')`
