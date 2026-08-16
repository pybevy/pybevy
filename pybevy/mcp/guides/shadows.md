# Shadows Guide

Shadow configuration, quality tuning, cascade setup, and common problems.

## Enabling Shadows

Shadows are off by default. Enable per-light:

```python
commands.spawn(
    DirectionalLight(illuminance=10000.0, shadow_maps_enabled=True),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
)
commands.spawn(
    PointLight(intensity=80000.0, shadow_maps_enabled=True),
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
| `512` | Low - visible stairstepping | Minimal |
| `1024` | Default for point lights | Low |
| `2048` | Default for directional lights, good for most scenes | Medium |
| `4096` | High quality - sharp shadows up close | High |

## Shadow Filtering

Controls how shadow edges look. Added to the camera entity:

```python
from pybevy.light import ShadowFilteringMethod

commands.spawn(
    Camera3d(),
    ShadowFilteringMethod.GAUSSIAN,  # Default, stable without temporal AA
)
```

| Method | When to use |
|--------|------------|
| `HARDWARE_2X2` | Fast, hard shadow edges. Budget/mobile. |
| `GAUSSIAN` | Soft edges, good general-purpose (default) |
| `TEMPORAL` | Softest, randomized; can shimmer because PyBevy does not yet expose TAA. |

## Contact Shadows (Screen-Space)

Shadow maps miss the small contact points where objects meet surfaces (feet on ground, cup on table); contact shadows ray-march the depth buffer to darken exactly those spots. Two parts: a `ContactShadows` component on the camera (which needs a `DepthPrepass`), and `contact_shadows_enabled=True` on each participating light.

```python
from pybevy.camera import DepthPrepass
from pybevy.pbr import ContactShadows

commands.spawn(
    Camera3d(),
    DepthPrepass(),       # required: contact shadows read the depth buffer
    ContactShadows(
        linear_steps=16,  # ray-march steps: more = smoother, slower
        thickness=0.1,    # assumed surface thickness (world units)
        length=0.3,       # max contact-shadow reach (world units)
    ),
)

commands.spawn(
    DirectionalLight(shadow_maps_enabled=True, contact_shadows_enabled=True),
    Transform.from_xyz(4.0, 8.0, 4.0).looking_at(Vec3(0.0, 0.0, 0.0), Vec3.Y),
)
```

`PointLight`, `SpotLight`, and `DirectionalLight` all have the `contact_shadows_enabled` flag (default `False`). Keep `length` short: this is a small-scale grounding effect layered on top of shadow maps, not a replacement for them.

Do not combine `ContactShadows` with
`DefaultOpaqueRendererMethod.deferred()`. The deferred lighting pipeline does
not support the contact-shadow view binding and the resulting validation error
prevents the scene from rendering. Use forward rendering for contact shadows.

## Bias Tuning

Two bias values prevent shadow artifacts. Each light type has its own defaults:

| Parameter | What it fixes | Default (Dir) | Default (Point) |
|-----------|--------------|---------------|-----------------|
| `shadow_depth_bias` | Shadow acne (moire patterns on lit surfaces) | `0.02` | `0.08` |
| `shadow_normal_bias` | Peter panning (shadow detached from object base) | `1.8` | `0.6` |

```python
# If you see shadow acne, increase depth_bias
DirectionalLight(shadow_maps_enabled=True, shadow_depth_bias=0.05)

# If shadow floats away from object, decrease normal_bias
PointLight(shadow_maps_enabled=True, shadow_normal_bias=0.3)
```

**Rule of thumb:** Increase `depth_bias` to fix acne; decrease `normal_bias` to fix peter panning. They trade off against each other - find the balance for your scene.

## Cascade Shadow Config (Directional Lights)

Directional lights use cascaded shadow maps - multiple shadow maps at different distances. Closer cascades have higher resolution.

```python
from pybevy.light import CascadeShadowConfig

# Bevy's default: 4 geometrically spaced cascades from 10 to 150 units
commands.spawn(
    DirectionalLight(illuminance=10000.0, shadow_maps_enabled=True),
    CascadeShadowConfig(),
)
```

### What `bounds` Means

Each value is the maximum distance (in world units) that cascade covers. Bevy's
computed defaults are approximately `[10, 24.66, 60.82, 150]`:

| Cascade | Far bound | Coverage |
|---------|---------------------------|----------|
| 0 | 0–10 | Near objects - highest detail |
| 1 | 10–24.66 | Mid-range |
| 2 | 24.66–60.82 | Far |
| 3 | 60.82–150 | Very far - lowest detail |

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
1. `shadow_maps_enabled=True` on the light?
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
