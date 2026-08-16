# Lighting & Atmosphere Guide

Setting up lights, fog, atmosphere, and emissive materials for indoor and outdoor scenes.

## Default Lighting Setup (Start Here)

Every 3D scene should start with a warm key light + cool fill light. Single-light scenes look flat with pure-black shadows.

```python
# Key light: warm, strong, shadows
commands.spawn(
    DirectionalLight(illuminance=7000.0, shadow_maps_enabled=True,
                     color=Color.srgb(1.0, 0.95, 0.85)),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.6, 0.4, 0.0)),
)
# Fill light: cool, ~40% of key, no shadows, opposite direction
commands.spawn(
    DirectionalLight(illuminance=3000.0, shadow_maps_enabled=False,
                     color=Color.srgb(0.6, 0.7, 0.9)),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.3, -1.2, 0.0)),
)
```

The warm/cool contrast creates visual depth. Fill at ~40% of key eliminates black shadows without flattening the scene.

## Light Types

### DirectionalLight (Sun)

Global light with parallel rays, like the sun. Affects all objects regardless of distance.

```python
from pybevy.prelude import *

commands.spawn(
    DirectionalLight(
        illuminance=10000.0,       # Bright sunny day
        color=Color.srgb(1.0, 0.95, 0.85),
        shadow_maps_enabled=True,
    ),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
)
```

**Illuminance reference values:**

| Value | Looks like |
|-------|------------|
| 500 | Overcast / dim indoor |
| 2000 | Cloudy day |
| 4000 | Twilight minimum (still visible) |
| 8000 | Bright but not harsh |
| 10000–15000 | Full sunlight |

**Tip:** Direction comes from the Transform rotation, not translation. Use `Quat.from_euler` or `.looking_at()` to aim it.

### PointLight (Torch / Lamp)

Emits light in all directions from a point, with distance falloff.

```python
commands.spawn(
    PointLight(
        intensity=80000.0,
        color=Color.srgb(1.0, 0.7, 0.3),  # Warm torch color
        range=8.0,
        shadow_maps_enabled=True,
    ),
    Transform.from_xyz(2.0, 3.0, 0.0),
)
```

**Intensity reference values:**

| Value | Use case |
|-------|----------|
| 10,000 | Dim candle |
| 80,000 | Torch / wall sconce |
| 100,000 | Bright lamp |
| 500,000+ | Very bright point source |

### SpotLight

Cone-shaped light, like a flashlight or stage light.

```python
commands.spawn(
    SpotLight(
        intensity=100000.0,
        color=Color.srgb(0.0, 1.0, 0.0),
        shadow_maps_enabled=True,
        inner_angle=0.6,  # Full-brightness cone (radians)
        outer_angle=0.8,  # Falloff cone edge
    ),
    Transform.from_xyz(-1.0, 2.0, 0.0).looking_at(Vec3(-1.0, 0.0, 0.0), Vec3.Z),
)
```

**Aiming downward:** For overhead spotlights pointing straight down, use `Vec3.Z` (not `Vec3.Y`) as the up-vector in `looking_at`, since the light direction aligns with Y:

```python
# Overhead spot pointing straight down at (x, 0, z)
Transform.from_xyz(x, 7.0, z).looking_at(Vec3(x, 0.0, z), Vec3.Z)
```

### RectLight (Area Light)

Soft light emitted from a rectangular surface: window panes, softboxes, glowing ceiling panels, neon signs. The rectangle lies in the entity's local XY plane (sized by `width`/`height`) and shines along local -Z, so aim it with `looking_at` exactly like a spotlight.

```python
from pybevy.light import RectLight

# Warm 2x1 ceiling panel shining straight down
commands.spawn(
    RectLight(
        color=Color.srgb(1.0, 0.9, 0.8),
        intensity=100_000.0,  # luminous power in lumens, spread over the rect
        range=20.0,           # hard cutoff; tune together with intensity
        width=2.0,
        height=1.0,
    ),
    Transform.from_xyz(0.0, 4.0, 0.0).looking_at(Vec3(0.0, 0.0, 0.0), Vec3.Z),
)
```

**Hard limit of 8 per view.** Bevy uploads at most 8 rect lights and silently drops the rest (it warns once at startup). They are not clustered, so this is a whole-scene budget: with 40 rect lights only 8 render, and raising `intensity` does nothing because the others were never uploaded.

**Budget them as hero sources** and use `PointLight` for everything else. Good spends: a window or doorway, a large screen, a wall wash, or a few big overhead soft-boxes that replace flat ambient light. Bad spend: one per lamp, monitor, or prop.

**No shadow maps:** objects lit by a RectLight do not cast shadows from it (upstream limitation). Pair it with a dim shadow-casting light if you need grounding shadows.

**Intensity scales with area.** `intensity` is total luminous power spread over `width * height`, so widening a panel dims it. A 20x26 ceiling soft-box needs roughly 400k lumens to match a 200k point light at the same height, so scale from the light you are replacing rather than from the panel's size.

### Ambient Light

Flat light applied everywhere - prevents pure-black shadows.

```python
# GlobalAmbientLight (resource, recommended)
commands.insert_resource(GlobalAmbientLight(
    brightness=300.0,
    color=Color.srgb(0.6, 0.65, 0.85),  # Cool blue-ish fill
))

# AmbientLight (component): overrides the global value for one camera
commands.spawn(Camera3d(), AmbientLight(color=Color.WHITE, brightness=200.0))
```

**Brightness 100–300** is typical for outdoor scenes. **Indoor/cave scenes need 500+** since there's no sky bounce. Higher values wash out shadows - but invisible scenes are worse than washed-out ones. Start bright, dim later.

## ClearColor (Background)

`ClearColor` is a resource that sets the background color behind everything. Essential for dark/night scenes - without it, the default gray sky breaks the mood:

```python
# Near-black for night/industrial scenes
commands.insert_resource(ClearColor(Color.srgb(0.02, 0.02, 0.04)))

# Deep blue for underwater
commands.insert_resource(ClearColor(Color.srgb(0.0, 0.05, 0.15)))
```

**Match ClearColor to fog color** to avoid a visible seam at the fog boundary. When they diverge, there's a jarring color discontinuity where fog ends.

### ClearColor Defaults by Scene Type

| Scene Type | ClearColor RGB | Notes |
|-----------|---------------|-------|
| Night/dark | `(0.02, 0.02, 0.03)` | Near-black with slight blue |
| Indoor/cave | `(0.01, 0.01, 0.02)` | Deeper black |
| Dusk/sunset | `(0.05, 0.03, 0.02)` | Warm dark brown |
| Underwater | `(0.02, 0.04, 0.06)` | Dark teal |
| Daytime | Use `Atmosphere` instead | Sky fills background |
| 2D game | `(0.015, 0.015, 0.03)` | Very dark blue-black |

Set it in your Startup system alongside `GlobalAmbientLight`. For outdoor daytime scenes using `Atmosphere`, you don't need it - the sky fills the background.

## Fog

`DistanceFog` is a camera component that fades distant objects into a fog color.

```python
from pybevy.pbr import DistanceFog, FogFalloff

commands.spawn(
    Camera3d(),
    Transform.from_xyz(5, 5, 5).looking_at(Vec3.ZERO, Vec3.Y),
    DistanceFog(
        color=Color.srgb(0.6, 0.65, 0.75),
        falloff=FogFalloff.Exponential(0.002),
        directional_light_color=Color.srgb(1.0, 0.85, 0.6),
        directional_light_exponent=40.0,
    ),
)
```

**Fog density reference:**

| Density | Effect |
|---------|--------|
| 0.001 | Very subtle, distant haze only |
| 0.002 | Light fog, good for outdoor scenes |
| 0.003–0.005 | Interior haze / moody indoor atmosphere |
| 0.005–0.008 | Thick fog, objects fade quickly |
| 0.01+ | Pea soup - very short visibility |

**Interior vs outdoor:** Indoor scenes need lower density than you'd expect. Walls constrain the camera to short distances, so even 0.005 can feel thick. Start with **0.003–0.004** for interior fog and adjust up. For outdoor scenes, 0.002 is a good default.

**Gotcha:** Fog interacts with lighting. A density that looks fine in daylight can make nighttime scenes nearly invisible. Dark interiors with low ambient light amplify fog opacity - if your scene looks washed out, reduce density before increasing light intensity. If you have a day/night cycle, keep density at 0.002 or lower.

**Top-down / overhead cameras:** Fog compounds with camera distance. An overhead camera at y=20 looking down sees every object through 15–20 units of fog, so densities that work for eye-level cameras (0.03–0.05) will black out the scene. Use **3–4x lower density** for top-down views (e.g., 0.01–0.015 instead of 0.04). Also boost ambient light (80+ instead of 20) to compensate.

## Atmosphere & Sky

`Atmosphere` renders a physically-based sky dome with scattering. It describes a planet: spawn it on its **own entity** (its `GlobalTransform` is the planet center; left at default it lands `inner_radius` below the origin, putting your scene on the surface). Cameras opt in with **`AtmosphereSettings()`**; the nearest Atmosphere is used. Without `AtmosphereSettings` on the camera the sky silently does not render (gray clear color, no errors, no warnings).

```python
from pybevy.light import Atmosphere, PhaseFunction, ScatteringMedium, ScatteringTerm
from pybevy.pbr import AtmosphereSettings

# In a system taking mediums: ResMut[Assets[ScatteringMedium]]
medium_handle = mediums.add(ScatteringMedium.earth())
commands.spawn(Atmosphere.earth(medium_handle))  # the planet: its own entity

commands.spawn(
    Camera3d(),
    Transform.from_xyz(5, 5, 5).looking_at(Vec3.ZERO, Vec3.Y),
    AtmosphereSettings(),
)
```

Inside a system with `mediums: ResMut[Assets[ScatteringMedium]]`, terms can be
edited in place. The typed sequence and its elements stay connected to the
stored asset:

```python
medium = mediums.get_mut(medium_handle)
if medium is not None:
    medium.terms[0].absorption.x = 1e-6
    medium.terms.append(ScatteringTerm(phase=PhaseFunction.Isotropic()))
```

Atmospheric falloff mirrors Bevy's enum variants: use `Falloff.Linear()`,
`Falloff.Exponential(scale)`, or `Falloff.Tent(center, width)`. Bevy's custom
`Curve` variant stores a Rust callback and cannot be constructed from Python.

An element kept across an insertion continues to address its numeric index.
Do not keep asset wrappers after the system call ends.

Putting `Atmosphere` on the camera entity also renders, but then the planet center follows the camera (altitude never changes); prefer the separate entity, as in Bevy's own examples.

Atmosphere responds to the DirectionalLight direction - the sky color changes as the "sun" moves.

**Exposure:** physically plausible sun values (85,000 to 130,000 lux, as in Bevy's own atmosphere examples) blow out to white at default exposure. Pair them with `Exposure` from `pybevy.camera` on the camera: `Exposure(ev100=13.0)` to `Exposure(ev100=15.0)`, or the `Exposure.OVERCAST` / `Exposure.SUNLIGHT` presets. The 3,000 to 15,000 lux values used elsewhere in this guide are tuned for default exposure instead.

**One sun only:** every `DirectionalLight` feeds the sky, so a high-elevation "fill" light turns a sunset sky midday-blue. In Atmosphere scenes use a single sun and add `AtmosphereEnvironmentMapLight()` on the camera for fill instead (see Environment Lighting below).

## Materials

`StandardMaterial` has many fields beyond what's shown here (PBR metallic/roughness, normal maps, occlusion, etc.). Use `get_type_definition(type_name="StandardMaterial")` to see all fields and their defaults.

## Emissive Materials (Glow)

Objects that emit light visually (for bloom) use the `emissive` field:

```python
from pybevy.color import LinearRgba

# Glowing flame
flame_mat = materials.add(StandardMaterial(
    base_color=Color.srgb(0.30, 0.16, 0.04),
    emissive=LinearRgba.rgb(8.0, 4.0, 0.8),  # HDR values > 1.0 for bloom
))

# Glowing window
window_mat = materials.add(StandardMaterial(
    base_color=Color.srgb(0.1, 0.08, 0.06),
    emissive=LinearRgba.rgb(2.5, 2.0, 0.8),  # Warm interior glow
))
```

**Emissive values:** 1.0–5.0 for subtle glow, 5.0–50.0 for bright glow, 100+ for intense bloom. Requires `Bloom` on the camera to be visible.

**`unlit=True` ignores `emissive`.** An unlit material outputs `base_color` alone, so put the glow colour there instead. `base_color` accepts HDR values and blooms just like `emissive`:

```python
# ❌ Renders plain white: emissive is ignored under unlit
StandardMaterial(emissive=LinearRgba.rgb(12.0, 3.0, 20.0), unlit=True)

# ✅ Lit surface with glow: dark base_color, HDR emissive
StandardMaterial(base_color=Color.srgb(0.14, 0.05, 0.22), emissive=LinearRgba.rgb(12.0, 3.0, 20.0))

# ✅ Unlit glow, cheapest since it skips all lighting: HDR base_color
StandardMaterial(base_color=Color.linear_rgb(12.0, 3.0, 20.0), unlit=True)
```

Use the lit form for glowing surfaces that still need shading (lava crust, a lit-room screen); keep its `base_color` dark, a bright one washes the glow out. Use the unlit form for pure light effects: particles, beams, additive VFX with `AlphaMode.Add()`.

**Bloom + emissive pairing:** High emissive values with high bloom intensity will blow out small meshes into shapeless blobs. Use this as a starting guide:

| Bloom intensity | Safe emissive range | Result |
|-----------------|---------------------|--------|
| 0.05–0.10 | 3.0–20.0 | Subtle halo, shape preserved |
| 0.10–0.20 | 2.0–12.0 | Visible glow, shape still clear |
| 0.20–0.30 | 1.0–8.0 | Strong glow, small meshes may blur |
| 0.30+ | 1.0–5.0 | Very intense, only large meshes keep shape |

Thin meshes (torus rings, thin cylinders) bloom faster than solid shapes. Reduce emissive or bloom if ring/wireframe shapes turn into solid discs.

**Metallic alternative for rings/torus shapes:** When reducing emissive still blooms rings into solid discs (especially multiple overlapping rings), use metallic materials lit by a nearby PointLight instead of emissive. The specular highlights follow the ring geometry without bloom blowout:

```python
# Metallic ring + nearby colored light - preserves ring shape
ring_mat = materials.add(StandardMaterial(
    base_color=Color.srgb(0.5, 0.35, 0.75),
    metallic=0.95, perceptual_roughness=0.08,
))
# PointLight near the ring assembly provides specular highlights
commands.spawn(
    PointLight(intensity=150000.0, color=Color.srgb(0.6, 0.4, 1.0), range=12.0),
    Transform.from_xyz(0, 6.0, 0),
)
```

### Multi-Emissive Scenes

Scenes with 10+ emissive objects get washed-out bloom if all sources use the same intensity. Use tiered values:

| Role | Emissive range | Example |
|------|---------------|---------|
| Hero (1–2 focal points) | 8.0–15.0 | Main crystal, fire |
| Accent (3–6 secondary) | 2.0–5.0 | Lanterns, runes |
| Subtle (ambient fill) | 0.5–1.5 | Window glow, moss |

Set bloom `prefilter` threshold to 0.8–1.0 so subtle emissives add color without blooming. Alternate warm/cool hues between adjacent sources to keep halos visually distinct.

## Transparency

Set `alpha_mode` on `StandardMaterial`:

```python
# Glass
glass = materials.add(StandardMaterial(
    base_color=Color.srgba(0.8, 0.9, 1.0, 0.3),
    alpha_mode=AlphaMode.Blend(),
))

# Cutout (foliage)
leaf = materials.add(StandardMaterial(
    base_color_texture=asset_server.load_image("leaf.png"),
    alpha_mode=AlphaMode.Mask(0.5),
))

# Additive glow
energy = materials.add(StandardMaterial(
    base_color=Color.srgba(0.2, 0.5, 1.0, 0.5),
    alpha_mode=AlphaMode.Add(),
    emissive=LinearRgba.rgb(2.0, 4.0, 8.0),
))
```

Key modes: `Blend()` (glass/water), `Mask(threshold)` (foliage), `Add()` (fire/lasers), `Opaque()` (default).

**Important:** `base_color` alpha < 1.0 has no effect without setting `alpha_mode`.

## Day/Night Cycle Pattern

Animate the DirectionalLight to simulate time of day:

```python
@component
class Sun(Component):
    pass

def day_night_cycle(
    query: Query[tuple[Mut[Transform], Mut[DirectionalLight]], With[Sun]],
    time: Res[Time],
):
    t = time.elapsed_secs() * 0.08  # Full cycle ~78 seconds
    sun_height = math.sin(t)  # -1 to 1
    daylight = max(0.0, sun_height)

    for transform, light in query:
        # Orbit the sun
        transform.translation = Vec3(
            math.cos(t) * 15.0,
            max(sun_height * 12.0 + 5.0, 1.0),
            math.sin(t) * 15.0,
        )
        transform.look_at(Vec3.ZERO, Vec3.Y)

        # Dim at night, bright at noon
        light.illuminance = 4000.0 + daylight * 8000.0

        # Warm at horizon, white at noon
        light.color = Color.srgb(1.0, 0.85 + daylight * 0.1, 0.6 + daylight * 0.35)
```

**Tip:** Keep the minimum illuminance at 3000–4000 so nighttime is dim but not pitch black.

## Fill Light (Eliminates Dark Side)

A single directional light leaves one side of every object black. Always add a **fill light** from the opposite direction with no shadows:

```python
# Key light (warm, shadows)
commands.spawn(
    DirectionalLight(illuminance=10000.0, color=Color.srgb(1.0, 0.95, 0.85), shadow_maps_enabled=True),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
)
# Fill light (cool, no shadows, ~50% of key)
commands.spawn(
    DirectionalLight(illuminance=5000.0, color=Color.srgb(0.7, 0.75, 0.9), shadow_maps_enabled=False),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.5, -1.2, 0.0)),
)
```

## Warm Light Color Shift

**Warning:** Warm directional light (`Color.srgb(1.0, 0.95, 0.85)`) shifts brown materials toward red. A wood surface with `base_color=Color.srgb(0.25, 0.15, 0.08)` under warm light looks distinctly reddish, not brown. This is physically correct PBR behavior but often surprising.

**Fix:** Desaturate the warm light slightly (`0.95, 0.93, 0.88`) or push wood base_color toward cooler brown (`0.22, 0.16, 0.12`).

## Recipes

### Bright Outdoor Scene
```python
commands.spawn(
    DirectionalLight(illuminance=10000.0, shadow_maps_enabled=True),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
)
commands.insert_resource(GlobalAmbientLight(brightness=300.0))
```

### Torch-Lit Interior
```python
commands.insert_resource(GlobalAmbientLight(brightness=500.0, color=Color.srgb(0.35, 0.3, 0.4)))
commands.spawn(
    PointLight(intensity=200000.0, color=Color.srgb(1.0, 0.7, 0.3), range=12.0, shadow_maps_enabled=True),
    Transform.from_xyz(0, 3, 0),
)
# Use 4+ PointLights to eliminate dark corners. Add fill lights at 100k+ in corners.
```

### Industrial Interior (Spotlights + Fog)
```python
# Cool ambient - 500+ for enclosed spaces (no sky bounce)
commands.insert_resource(GlobalAmbientLight(brightness=500.0, color=Color.srgb(0.5, 0.5, 0.6)))
commands.insert_resource(ClearColor(Color.srgb(0.04, 0.03, 0.05)))
# Weak directional for general fill
commands.spawn(
    DirectionalLight(illuminance=8000.0, shadow_maps_enabled=True),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -1.2, 0.3, 0.0)),
)
# Harsh overhead spots (repeat for each light position)
commands.spawn(
    SpotLight(intensity=800000.0, color=Color.srgb(1.0, 0.85, 0.55),
              shadow_maps_enabled=True, inner_angle=0.3, outer_angle=0.7, range=15.0),
    Transform.from_xyz(0.0, 7.0, 0.0).looking_at(Vec3(0.0, 0.0, 0.0), Vec3.Z),
)
# On camera: interior fog (keep density low for enclosed spaces)
# DistanceFog(color=Color.srgb(0.15, 0.13, 0.14), falloff=FogFalloff.Exponential(0.004))
```

### Moody Fog Scene
```python
# On camera:
DistanceFog(
    color=Color.srgb(0.4, 0.45, 0.5),
    falloff=FogFalloff.Exponential(0.004),
    directional_light_color=Color.srgb(1.0, 0.85, 0.6),
    directional_light_exponent=40.0,
)
```

## Volumetric Fog

Volumetric fog renders light shafts (god rays) and localized fog volumes. Requires three parts:

1. **`VolumetricFog`** on the camera - enables the volumetric rendering pass
2. **`VolumetricLight`** on lights - marks which lights contribute to the fog
3. **`FogVolume`** (optional) - localized fog regions with custom density

**Performance warning:** the pass raymarches every pixel against every shadowed
`VolumetricLight`. A scene-sized `FogVolume` with 2-3 shadowed volumetric lights can
drop mid-range GPUs below 30 FPS at high resolutions. Start with `step_count=16-32`,
ONE volumetric light, and small volumes; verify with `get_performance` before adding
more. For cheap fake light shafts, use tall additive unlit cones instead
(`AlphaMode.Add()`, `unlit=True`, alpha 0.02-0.05).

```python
from pybevy.light import VolumetricFog, VolumetricLight, FogVolume

# Camera with volumetric fog enabled
commands.spawn(
    Camera3d(),
    Transform.from_xyz(5, 5, 5).looking_at(Vec3.ZERO, Vec3.Y),
    VolumetricFog(
        ambient_color=Color.srgb(0.05, 0.05, 0.08),
        ambient_intensity=0.1,
        step_count=64,
        jitter=1.0,        # Reduces banding; can shimmer without temporal AA
    ),
)

# Sun that creates god rays
commands.spawn(
    DirectionalLight(illuminance=10000.0, shadow_maps_enabled=True),
    VolumetricLight(),      # This light affects fog
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
)
```

### Localized Fog Volume

```python
# A box of dense fog (e.g., swamp area, smoke cloud)
commands.spawn(
    FogVolume(
        density_factor=0.3,
        fog_color=Color.srgb(0.5, 0.55, 0.5),
        absorption=0.3,
        scattering=0.5,
    ),
    Transform.from_xyz(0, 1, 0).with_scale(Vec3(5.0, 2.0, 5.0)),
)
```

The `Transform` scale controls the volume size. See `guide://recipes/volumetric` for a complete scene.

The volume is a hard-edged box with no falloff, so its boundary draws a visible seam
wherever it crosses the frame: size ground mist wider than the visible floor. A volume
that contains the camera applies to the entire frame at once.

**FogVolume parameter reference:**

| Parameter | What it does | Low value | High value |
|-----------|-------------|-----------|------------|
| `density_factor` | Overall thickness | Faint haze (0.05) | Thick soup (0.5) |
| `absorption` | Light absorbed passing through | Bright mist (0.01) | **Dark/black fog** (0.3+) |
| `scattering` | Light bounced toward camera | Invisible volume (0.1) | Bright white mist (0.7) |
| `scattering_asymmetry` | Forward vs back scatter (0–1) | Even glow (0.2) | Sun-facing glow (0.6) |

**Critical:** `absorption` and `scattering` have opposite visual effects. High absorption makes fog **dark** (it eats light). High scattering makes fog **bright** (it bounces light toward the camera). Getting this backwards is the #1 FogVolume mistake.

**Recipes by look:**

```python
# Bright white mist (waterfall spray, steam, morning fog)
FogVolume(density_factor=0.3, absorption=0.02, scattering=0.7, scattering_asymmetry=0.5)

# Dark smoke / smog
FogVolume(density_factor=0.3, absorption=0.3, scattering=0.15, scattering_asymmetry=0.3)

# Subtle atmospheric haze
FogVolume(density_factor=0.05, absorption=0.005, scattering=0.1, scattering_asymmetry=0.2)
```

### Layered Fog Gradient (Mist That Thins With Height)

Stack multiple FogVolumes at increasing heights with decreasing density. Each layer is wider and thinner than the one below:

```python
# Dense ground layer (waterfall base, swamp)
commands.spawn(
    FogVolume(density_factor=0.4, absorption=0.02, scattering=0.7, scattering_asymmetry=0.5,
              fog_color=Color.srgb(0.75, 0.8, 0.85)),
    Transform.from_xyz(0, 0.5, 0).with_scale(Vec3(8.0, 1.2, 8.0)),
)
# Mid layer - wider, less dense
commands.spawn(
    FogVolume(density_factor=0.2, absorption=0.015, scattering=0.5, scattering_asymmetry=0.4,
              fog_color=Color.srgb(0.7, 0.75, 0.8)),
    Transform.from_xyz(0, 2.0, 0).with_scale(Vec3(13.0, 3.0, 13.0)),
)
# Upper haze - very wide, barely visible
commands.spawn(
    FogVolume(density_factor=0.08, absorption=0.01, scattering=0.25,
              fog_color=Color.srgb(0.6, 0.65, 0.7)),
    Transform.from_xyz(0, 4.0, 0).with_scale(Vec3(18.0, 4.0, 18.0)),
)
```

**Tips:**
- Keep `absorption` very low (0.01–0.03) across all layers to avoid dark spots where volumes overlap
- Add a `VolumetricLight` point light inside the dense layer to illuminate the mist from within
- Pair with `DistanceFog` on the camera for background haze beyond the volumes

## Light Cookies

Project a texture pattern through a light - for window shadows, stained glass, gobos.
The filenames below are placeholders for project-owned textures; they are not
bundled with PyBevy.

```python
from pybevy.camera import CubemapLayout
from pybevy.light import DirectionalLightTexture, SpotLightTexture, PointLightTexture

# Directional light with a projected pattern (e.g., window frame shadow)
commands.spawn(
    DirectionalLight(illuminance=10000.0, shadow_maps_enabled=True),
    DirectionalLightTexture(
        image=asset_server.load_image("textures/window_cookie.png"),
        tiled=False,
    ),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
)

# Spot light with gobo texture
commands.spawn(
    SpotLight(intensity=100000.0, shadow_maps_enabled=True, outer_angle=0.6),
    SpotLightTexture(image=asset_server.load_image("textures/gobo.png")),
    Transform.from_xyz(0, 5, 0).looking_at(Vec3.ZERO, Vec3.Z),
)

# Point light with a packed cubemap cookie
commands.spawn(
    PointLight(intensity=100000.0),
    PointLightTexture(
        image=asset_server.load_image("textures/point_cookie_cross.png"),
        cubemap_layout=CubemapLayout.CrossVertical,
    ),
    Transform.from_xyz(0, 3, 0),
)
```

`PointLightTexture` requires a cubemap layout: `CubemapLayout.CrossVertical`, `CrossHorizontal`, `SequenceVertical`, or `SequenceHorizontal`.

## SunDisk

Renders a visible sun disc on directional lights. Requires an `Atmosphere` entity in the scene and `AtmosphereSettings` on the camera.

```python
from pybevy.light import SunDisk

commands.spawn(
    DirectionalLight(illuminance=10000.0),
    SunDisk.EARTH,                    # Physically accurate earth sun size
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.6, 0.3, 0.0)),
)
```

Custom size: `SunDisk(angular_size=0.02, intensity=1.0)` - larger `angular_size` = bigger sun.

## Environment Lighting

**Essential for metallic/reflective materials.** Without environment lighting, metals appear flat black because they rely on reflections. Add one of these to any scene with `metallic > 0.5`.

Four approaches, from easiest to most control:

### 1. AtmosphereEnvironmentMapLight (Easiest)

Derives environment lighting from the atmosphere sky, so no texture loading is needed. The
scene needs an `Atmosphere` entity: without one the camera component is a silent no-op and
metallic surfaces stay as dark as they were with no environment light at all.

```python
from pybevy.light import Atmosphere, AtmosphereEnvironmentMapLight, ScatteringMedium
from pybevy.pbr import AtmosphereSettings

# Required. Spawn once per scene, alongside the camera below.
# In a system taking mediums: ResMut[Assets[ScatteringMedium]]
commands.spawn(Atmosphere.earth(mediums.add(ScatteringMedium.earth())))
commands.spawn(
    Camera3d(),
    AtmosphereSettings(),
    AtmosphereEnvironmentMapLight(intensity=1.0),
)
```

**Best for:** Outdoor scenes already using `Atmosphere`.

### 2. EnvironmentMapLight (HDR Cubemap)

**This is the single biggest visual quality upgrade for metallic scenes.** Without env maps, chrome/metal surfaces reflect only ambient color (flat gray). With env maps, they show realistic environment reflections. Recommended whenever a scene has `metallic >= 0.8` materials.

Load a pre-baked HDR environment map for indoor or studio lighting. The source
checkout includes Bevy's prefiltered Pisa pair at the paths below. Wheels omit
sample assets, so installed projects must supply equivalent files under
`assets/` or change the paths. `EnvironmentMapLight` needs a diffuse cubemap and
a mipmapped specular cubemap; `rgb9e5` and `zstd` only describe this sample's
encoding and compression.

```python
from pybevy.light import EnvironmentMapLight

commands.spawn(
    Camera3d(),
    EnvironmentMapLight(
        diffuse_map=asset_server.load_image("bevy/environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
        specular_map=asset_server.load_image("bevy/environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
        intensity=1000.0,
    ),
)
```

Pair with `Skybox` for a visible background: see `guide://camera` (Skybox section).

A procedural cubemap created from a single `Image` has only one mip level, so
using it directly as `specular_map` cannot blur reflections as material
roughness increases. Use a prefiltered, mipmapped asset here, or pass the source
cubemap to `GeneratedEnvironmentMapLight` below so Bevy generates the diffuse
and specular maps at runtime.

**Reflection probes and parallax correction:** an `EnvironmentMapLight` can also ride on a `LightProbe` entity to light just one region (a room, a courtyard). Probe reflections default to `ParallaxCorrection.Auto()`: they are box-projected against the probe's bounds (its `Transform` scale), which anchors reflections to nearby walls instead of smearing them at infinity. Use `ParallaxCorrection.None_()` when the cubemap depicts distant scenery (sky, horizon), or `ParallaxCorrection.Custom(half_extents)` when the projection box should differ from the probe volume:

```python
from pybevy.light import LightProbe, EnvironmentMapLight, ParallaxCorrection

commands.spawn(
    LightProbe(),
    EnvironmentMapLight(
        diffuse_map=asset_server.load_image("bevy/environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
        specular_map=asset_server.load_image("bevy/environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
        intensity=1000.0,
    ),
    ParallaxCorrection.Custom(Vec3(4.0, 2.5, 4.0)),  # half-extents of the room
    Transform.from_xyz(0.0, 2.5, 0.0).with_scale(Vec3(8.0, 5.0, 8.0)),
)
```

### 3. GeneratedEnvironmentMapLight (Runtime Filtering)

Use `GeneratedEnvironmentMapLight` when you have one source cubemap and want
Bevy to generate the diffuse and mipmapped specular maps on the GPU. The source
must be a square, power-of-two cubemap no wider than 8192 pixels.

```python
from pybevy.light import GeneratedEnvironmentMapLight

source_cubemap = asset_server.load_image(
    "bevy/environment_maps/sky_skybox.ktx2"
)
commands.spawn(
    Camera3d(),
    GeneratedEnvironmentMapLight(
        environment_map=source_cubemap,
        intensity=1000.0,
    ),
)
```

The source checkout includes this sample cubemap; wheels omit sample assets.
Runtime filtering completes asynchronously and Bevy inserts the resulting
`EnvironmentMapLight` when it is ready.

### 4. IrradianceVolume + LightProbe (Baked GI)

For pre-baked global illumination in static scenes:

The path below is a placeholder for a project-owned baked volume; PyBevy does
not bundle `irradiance/volume.ktx2`.

```python
from pybevy.light import LightProbe, IrradianceVolume

commands.spawn(
    LightProbe(),
    IrradianceVolume(
        voxels=asset_server.load_image("irradiance/volume.ktx2"),
        intensity=500.0,
    ),
    Transform.from_xyz(0, 2, 0).with_scale(Vec3(10.0, 5.0, 10.0)),
)
```

The `Transform` scale defines the volume the probes cover.

## Shadow Cost Guidance

There is no universal light-count budget: shadow resolution, light type, scene
geometry, target hardware, and camera coverage all affect the cost. Measure the
target scene with `get_performance` and disable shadows on lights that do not
need them.

**General rule:** Shadow-casting lights are expensive. For scenes with many lights (10+), set `shadow_maps_enabled=False` on decorative and fill lights, and reserve `shadow_maps_enabled=True` for key lights (sun, main spot, 1–2 hero point lights).

## Related Guides

- **Materials:** `guide://materials` - PBR surfaces, transmission, clearcoat
- **Shadows:** `guide://shadows` - Shadow maps, cascades, bias tuning
- **Camera:** `guide://camera` - Bloom (needed for emissive glow), color grading
