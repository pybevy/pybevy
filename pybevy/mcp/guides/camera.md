# Camera Guide

Camera setup, positioning, post-processing effects, and common camera movement patterns.

## Basic Camera

Every scene needs at least one camera. `Camera3d` for 3D scenes, `Camera2d` for 2D.

```python
from pybevy.prelude import *

# Simple 3D camera - look at Y=1 (not ZERO) to avoid ground-centered framing
commands.spawn(
    Camera3d(),
    Transform.from_xyz(8, 6, 8).looking_at(Vec3(0, 1, 0), Vec3.Y),
)
```

**`looking_at(target, up)`** orients the camera to face `target`. The `up` vector is almost always `Vec3.Y`.

## Positioning

```python
# From specific position, looking at a target
Transform.from_xyz(10, 8, 10).looking_at(Vec3(0, 2, 0), Vec3.Y)

# From rotation (useful for top-down or angled views)
Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.5, 0.3, 0.0))

# Useful Transform methods
transform.forward()    # Camera's forward direction (Vec3)
transform.right()      # Camera's right direction
transform.up()         # Camera's up direction
transform.look_at(target, Vec3.Y)  # Re-orient to face target
```

### Orthographic 3D

Use `OrthographicProjection.default_3d()` so the near/far planes match a 3D
camera, then choose how much world height should remain visible:

```python
from pybevy.camera import OrthographicProjection, Projection, ScalingMode

orthographic = OrthographicProjection.default_3d()
orthographic.scaling_mode = ScalingMode.FixedVertical(10.0)
commands.spawn(
    Camera3d(),
    Projection.Orthographic(orthographic),
    Transform.from_xyz(8, 6, 8).looking_at(Vec3(0, 1, 0), Vec3.Y),
)
```

## Post-Processing Effects

Post-processing components are added directly to the camera entity.

### Bloom (Glow)

Makes bright/emissive objects glow. Adding `Bloom` also adds the required `Hdr`
marker to that camera; a bare `Camera3d` is not HDR by default.

```python
commands.spawn(
    Camera3d(),
    Transform.from_xyz(5, 5, 5).looking_at(Vec3.ZERO, Vec3.Y),
    Bloom(intensity=0.2, low_frequency_boost=0.6),
)

# Or use a preset
commands.spawn(Camera3d(), Bloom.NATURAL)
```

**Bloom fields:**
- `intensity` - Overall bloom strength (0.0–1.0, default ~0.15)
- `low_frequency_boost` - Enhances large soft glow (0.0–1.0). `0.0–0.2` = sharp point-source glow (neon). `0.4–0.6` = soft atmospheric haze (fog, dreamy).
- `low_frequency_boost_curvature` - Controls how broadly the low-frequency boost affects neighboring frequencies (0.0–1.0, default 0.95)
- `high_pass_frequency` - Controls the maximum scattering angle (0.0–1.0, default 1.0)
- `prefilter` - `BloomPrefilter(threshold, threshold_softness)` limits bloom to bright regions
- `composite_mode` - `BloomCompositeMode.EnergyConserving` by default; use `Additive` with a non-default prefilter
- `max_mip_dimension` - Largest bloom mip dimension (default 512); raise only to reduce sampling artifacts at large scales
- `scale` - `Vec2` stretch per axis; values greater than 1 widen bloom on that axis

**Bloom presets by scene style:**

| Style | Code | When to use |
|-------|------|-------------|
| Subtle | `Bloom(intensity=0.08, low_frequency_boost=0.2)` | Clean/architectural scenes |
| Standard | `Bloom(intensity=0.12, low_frequency_boost=0.4)` | General purpose - good default |
| Atmospheric | `Bloom(intensity=0.15, low_frequency_boost=0.5)` | Foggy, moody, dreamy scenes |
| Neon/Sci-fi | `Bloom(intensity=0.20, low_frequency_boost=0.6)` | Cyberpunk, sci-fi, neon signs |

**Presets:**
- `Bloom.NATURAL` - Subtle, physically-based glow
- `Bloom.ANAMORPHIC` - Wide horizontal streaks (cinematic lens flare look)
- `Bloom.OLD_SCHOOL` - Hard threshold glow (retro/arcade look)
- `Bloom.SCREEN_BLUR` - Blurs the whole screen (dream/flashback effect)

**Anamorphic bloom (manual):** Use `scale=Vec2(4.0, 1.0)` to stretch bloom horizontally:

```python
Bloom(intensity=0.3, scale=Vec2(4.0, 1.0), max_mip_dimension=1024)  # Wide horizontal streaks
```

**Bloom threshold** - only bloom bright pixels:

```python
Bloom(intensity=0.3, prefilter=BloomPrefilter(threshold=1.5, threshold_softness=0.3))
```

### Fog & Atmosphere

See `guide://lighting` for full details:
- `DistanceFog(color=..., falloff=FogFalloff.Exponential(0.002))`: camera component, distance-based fog
- `Atmosphere.earth(medium_handle)`: physically-based sky. Spawn on its own entity (it is the planet); the camera opts in with `AtmosphereSettings()`, without which the sky silently does not render
- `Atmosphere.mars(medium_handle)` - Mars variant (use with `ScatteringMedium.mars(...)` for dusty red sky)

### Tonemapping

Controls how HDR colors map to screen colors.

```python
commands.spawn(Camera3d(), Tonemapping.TONY_MC_MAPFACE)
```

Common options: `Tonemapping.TONY_MC_MAPFACE` (cinematic), `Tonemapping.ACES_FITTED` (film), `Tonemapping.BLENDER_FILMIC` (matches Blender renders).

### Wireframe

```python
commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat), Wireframe())
commands.spawn(Mesh3d(mesh), MeshMaterial3d(mat), Wireframe(), WireframeColor(Color.srgb(0, 1, 0)))
# Global: commands.insert_resource(WireframeConfig(global_=True))
```

### Exposure

```python
from pybevy.camera import Exposure
commands.spawn(Camera3d(), Exposure.INDOOR)  # SUNLIGHT, INDOOR, OVERCAST, BLENDER
```

### Color Grading

Most grading controls require HDR. Add `Hdr()` explicitly, or use a component
such as `Bloom` that requires it:

```python
from pybevy.render import ColorGrading, ColorGradingSection, Hdr

commands.spawn(
    Camera3d(),
    Hdr(),
    ColorGrading(midtones=ColorGradingSection(saturation=0.7)),
)
```

Without `Hdr`, only global exposure and post-saturation are applied; hue,
temperature, tint, and the shadow/midtone/highlight controls are ignored.

### Vignette and Lens Distortion

Use these camera components for lens-style framing and warping. Explicit subtle
values are usually a better starting point than the strong defaults:

```python
from pybevy.post_process import LensDistortion, Vignette

commands.spawn(
    Camera3d(),
    Vignette(intensity=0.35, radius=0.8, smoothness=4.0),
    LensDistortion(intensity=0.08, scale=1.02),
)
```

Positive `LensDistortion.intensity` produces barrel distortion; negative values
produce pincushion distortion. Increase `scale` to crop stretched edges. Set
either component's `intensity` to `0.0` to disable its effect.

### Screen-Space Effects

**SSAO** darkens crevices between nearby geometry. **Requires `Msaa.Off`** on
the camera. With MSAA enabled, SSAO does not activate and Bevy logs an error
naming the incompatible `Msaa` value.

```python
from pybevy.pbr import ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel, ScreenSpaceReflections
commands.spawn(
    Camera3d(),
    Msaa.Off,  # Required for SSAO and SSR
    ScreenSpaceAmbientOcclusion(
        quality_level=ScreenSpaceAmbientOcclusionQualityLevel.High()),
    Transform.from_xyz(8, 6, 8).looking_at(Vec3(0, 1, 0), Vec3.Y),
)
```

**SSR** (Screen-Space Reflections) adds real-time reflections on metallic surfaces. Best on `metallic >= 0.9` with `perceptual_roughness` in `0.12–0.55`. Tuning: `min_perceptual_roughness`/`max_perceptual_roughness` take `(start, end)` fade tuples, `edge_fadeout=(x, y)` fades reflections near screen edges:

**SSR needs deferred rendering, enabled before any material is created.** Insert
`DefaultOpaqueRendererMethod.deferred()` in `@entrypoint` (a Startup system also
works, anything before the first frame): inserting it after materials have been
prepared leaves their pipelines untouched and SSR renders nothing.
`ScreenSpaceReflections()` adds `DepthPrepass` and `DeferredPrepass` to its
camera automatically.

Other cameras in an app using deferred rendering must add
`DepthPrepass()` and `DeferredPrepass()` themselves. Without
`DeferredPrepass()`, an otherwise valid deferred camera can render only its
clear colour without reporting a configuration error.

```python
from pybevy.pbr import DefaultOpaqueRendererMethod

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(DefaultOpaqueRendererMethod.deferred())
        .add_systems(Startup, setup)
    )

# then in setup():
commands.spawn(
    Camera3d(),
    Msaa.Off,
    ScreenSpaceReflections(),
    Transform.from_xyz(8, 6, 8).looking_at(Vec3(0, 1, 0), Vec3.Y),
)
```

**Do not go below `perceptual_roughness=0.12`.** The default
`min_perceptual_roughness=(0.08, 0.12)` is a fade-*in*, so a mirror-smooth
surface at 0.05 receives exactly zero SSR: the flat-mirror instinct produces a
blank floor. Lower `min_perceptual_roughness` if you truly want a mirror.

Deferred does not support transparency or MSAA, and
`DefaultOpaqueRendererMethod` is app-global (every camera, every `Auto`
material), so this is a whole-app decision, not a per-effect one. It also
silently hides `@material` custom shaders (see `guide://shaders`): keep custom
shader scenes on forward rendering.

Specular-transmission materials can remain in the same scene, but Bevy renders
those meshes in a separate transmissive phase rather than into the deferred
G-buffer. SSR therefore applies to surrounding opaque surfaces, not to the
transmissive mesh itself, and may show screen-space artifacts along glass edges.

ChromaticAberration, DepthOfField, and MotionBlur are not yet available in PyBevy.

### Skybox

A cube-map texture rendered as the background. Pair with `EnvironmentMapLight`
for reflections that match. The source checkout includes Bevy's prefiltered
Pisa maps at the paths below; wheels omit sample assets, so installed projects
must supply equivalent files under `assets/` or change the paths.

```python
from pybevy.light import EnvironmentMapLight, Skybox

pisa_diffuse = asset_server.load_image(
    "bevy/environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"
)
pisa_specular = asset_server.load_image(
    "bevy/environment_maps/pisa_specular_rgb9e5_zstd.ktx2"
)

commands.spawn(
    Camera3d(),
    Skybox(image=pisa_specular, brightness=1000.0),
    EnvironmentMapLight(
        diffuse_map=pisa_diffuse,
        specular_map=pisa_specular,
        intensity=1000.0,
    ),
)
```

For a procedural image containing six vertically stacked square faces, mark the
array texture's view as a cubemap before adding it to `Assets[Image]`:

```python
from pybevy.render import TextureViewDimension

cubemap.reinterpret_stacked_2d_as_array(6)
cubemap.texture_view_dimension = TextureViewDimension.Cube
cubemap_handle = images.add(cubemap)
commands.spawn(Camera3d(), Skybox(image=cubemap_handle, brightness=1000.0))
```

Or with atmosphere (no texture needed): use `AtmosphereEnvironmentMapLight` - see `guide://lighting` (Environment Lighting section).

### Anti-Aliasing (MSAA)

MSAA is a **per-camera component** - add it directly to the camera entity:

```python
commands.spawn(Camera3d(), Msaa.Sample4)  # Default - good balance
```

| Option | When to use |
|--------|------------|
| `Msaa.Off` | Required for SSAO, SSR, deferred rendering |
| `Msaa.Sample2` | Light AA, better performance |
| `Msaa.Sample4` | Default - smooth edges |
| `Msaa.Sample8` | Maximum quality, higher GPU cost |

**Compatibility note:** SSAO and screen-space reflections require `Msaa.Off`.
PyBevy does not yet expose Bevy's TAA, FXAA, or SMAA components, so there is no
supported edge-anti-aliasing replacement for these effects yet.

### RenderLayers

Selective rendering - assign cameras and objects to layers so only matching layers are visible:

```python
from pybevy.camera import RenderLayers

# Main camera sees layer 0 (default)
commands.spawn(Camera3d(), RenderLayers.layer(0))

# UI camera sees layer 1 only
commands.spawn(Camera3d(), RenderLayers.layer(1))

# Object visible to both cameras
commands.spawn(
    Mesh3d(mesh), MeshMaterial3d(mat),
    RenderLayers.layer(0),  # Use .with_(1) to add layer 1 too
)

# Object only visible to UI camera
commands.spawn(Mesh3d(ui_mesh), MeshMaterial3d(ui_mat), RenderLayers.layer(1))
```

Default: all entities and cameras are on layer 0.

### Viewport (Split Screen)

Render multiple cameras to different screen regions:

```python
from pybevy.camera import Viewport

# Left half
commands.spawn(
    Camera3d(),
    Camera(order=0, viewport=Viewport(
        physical_position=UVec2(0, 0),
        physical_size=UVec2(960, 1080),
    )),
    Transform.from_xyz(5, 5, 5).looking_at(Vec3.ZERO, Vec3.Y),
)

# Right half
commands.spawn(
    Camera3d(),
    Camera(order=1, viewport=Viewport(
        physical_position=UVec2(960, 0),
        physical_size=UVec2(960, 1080),
    )),
    Transform.from_xyz(-5, 5, -5).looking_at(Vec3.ZERO, Vec3.Y),
)
```

### VisibilityRange (Distance LOD)

Hide or fade objects based on camera distance - simple level-of-detail:

```python
from pybevy.camera import VisibilityRange

# Show detailed model only when close (0–50 units)
commands.spawn(
    Mesh3d(detailed_mesh), MeshMaterial3d(mat),
    VisibilityRange.abrupt(0.0, 50.0),
)

# Show simple model far away (40–500 units)
commands.spawn(
    Mesh3d(simple_mesh), MeshMaterial3d(mat),
    VisibilityRange.abrupt(40.0, 500.0),
)
```

The overlap range (40–50) allows crossfade if dithering is enabled.

`ViewVisibility` is aggregated across every active view, including shadow-map
views. A visible value therefore does not mean the entity is on screen in a
particular camera.

### Full Camera Setup

A cinematic camera combines all effects on one entity:

```python
commands.spawn(Atmosphere.earth(medium_handle))  # planet entity (once per scene)
commands.spawn(
    Camera3d(),
    Transform.from_xyz(10, 8, 10).looking_at(Vec3(0, 2, 0), Vec3.Y),
    Bloom(intensity=0.2, low_frequency_boost=0.6),
    DistanceFog(color=Color.srgb(0.6, 0.65, 0.75), falloff=FogFalloff.Exponential(0.002)),
    AtmosphereSettings(),  # opts this camera into the atmosphere sky
)
```

## Camera Movement Patterns

> **Ready-made plugins:** For interactive orbit and fly cameras, use `from pybevy.contrib import OrbitCameraPlugin, FlyCameraPlugin`. See `guide://contrib`.

### Orbit Camera

Circles around a target point. Good for object inspection.

```python
@component
class OrbitCamera(Component):
    pass

def orbit_camera(query: Query[Mut[Transform], With[OrbitCamera]], time: Res[Time]):
    t = time.elapsed_secs() * 0.3
    radius = 15.0
    height = 8.0 + math.sin(t * 0.5) * 2.0
    for transform in query:
        transform.translation = Vec3(math.cos(t) * radius, height, math.sin(t) * radius)
        transform.look_at(Vec3(0, 2, 0), Vec3.Y)
```

### Cinematic Multi-Shot Camera

Cuts between predefined shots with smooth transitions. Good for trailers or flythroughs.

```python
# (pos_x, pos_y, pos_z, look_x, look_y, look_z, hold_secs, transition_secs)
SHOTS = [
    (0, 20, -30, 0, 3, 0, 5, 3),   # Wide establishing shot
    (5, 3, 10,  0, 2, 5, 5, 3),     # Ground level
    (0, 10, 0,  8, 5, 0, 5, 3),     # Mid-air overview
]

def _smoothstep(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return t * t * (3.0 - 2.0 * t)

def cinematic_camera(query: Query[Mut[Transform], With[OrbitCamera]], time: Res[Time]):
    # Compute total cycle duration
    total = sum(s[6] + s[7] for s in SHOTS)
    elapsed = time.elapsed_secs() % total

    # Find current shot
    acc = 0.0
    for i, shot in enumerate(SHOTS):
        hold, trans = shot[6], shot[7]
        if elapsed < acc + hold:
            # In hold phase - static position
            pos = Vec3(float(shot[0]), float(shot[1]), float(shot[2]))
            look = Vec3(float(shot[3]), float(shot[4]), float(shot[5]))
            break
        elif elapsed < acc + hold + trans:
            # In transition - lerp to next shot
            t = _smoothstep((elapsed - acc - hold) / trans)
            nxt = SHOTS[(i + 1) % len(SHOTS)]
            pos = Vec3(float(shot[0]), float(shot[1]), float(shot[2])).lerp(
                Vec3(float(nxt[0]), float(nxt[1]), float(nxt[2])), t)
            look = Vec3(float(shot[3]), float(shot[4]), float(shot[5])).lerp(
                Vec3(float(nxt[3]), float(nxt[4]), float(nxt[5])), t)
            break
        acc += hold + trans

    for transform in query:
        transform.translation = pos
        transform.look_at(look, Vec3.Y)
```

### Path-Following Camera

Camera follows a procedural curve through space. Good for tunnels, flythroughs, and chase cameras.

**Key pattern:** Set the position, then call `look_at()` in-place on the mutable transform:

```python
def camera_follow_path(
    query: Query[Mut[Transform], With[Camera3d]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    speed = 8.0

    # Procedural path - camera position
    z = -speed * t
    x = math.sin(z * 0.1 + t * 0.5) * 3.0
    y = math.cos(z * 0.08 + t * 0.3) * 2.0 + 5.0

    # Look-ahead point (same path, further along)
    lz = z - 20.0
    lx = math.sin(lz * 0.1 + t * 0.5) * 3.0
    ly = math.cos(lz * 0.08 + t * 0.3) * 2.0 + 5.0

    for transform in query:
        transform.translation = Vec3(x, y, z)
        transform.look_at(Vec3(lx, ly, lz), Vec3.Y)
```

**Why `look_at()` instead of `looking_at()`?** `look_at()` modifies the transform in-place (works on `Mut[Transform]`). `looking_at()` is a builder method for owned transforms (e.g., in Startup systems).

**Transition between cameras:** Use smoothstep to blend from one camera mode to another:

```python
def transition_camera(
    query: Query[Mut[Transform], With[Camera3d]],
    state: Res[CameraState],
    time: Res[Time],
) -> None:
    sm = smoothstep((time.elapsed_secs() - state.transition_start) / 2.0)

    # Source: static overview
    sx, sy, sz = 0.0, 10.0, 15.0
    slx, sly, slz = 0.0, 0.0, 0.0

    # Target: path position (computed from your path function)
    tx, ty, tz = get_path_pos(time.elapsed_secs())
    tlx, tly, tlz = get_path_pos_ahead(time.elapsed_secs())

    for transform in query:
        nt = Transform.from_xyz(
            sx + (tx - sx) * sm, sy + (ty - sy) * sm, sz + (tz - sz) * sm,
        ).looking_at(Vec3(
            slx + (tlx - slx) * sm, sly + (tly - sly) * sm, slz + (tlz - slz) * sm,
        ), Vec3.Y)
        transform.translation = nt.translation
        transform.rotation = nt.rotation
```

See `guide://recipes/demoscene` for a complete example with camera transitioning from a static view into a tunnel flythrough.

### Mouse Orbit (Interactive)

```python
from pybevy.input import AccumulatedMouseMotion, ButtonInput, MouseButton

def orbit(
    camera: Single[Mut[Transform], With[Camera3d]],
    mouse_buttons: Res[ButtonInput[MouseButton]],
    mouse_motion: Res[AccumulatedMouseMotion],
) -> None:
    delta = Vec2(mouse_motion.delta.x, mouse_motion.delta.y)
    yaw, pitch, roll = camera.rotation.to_euler(EulerRot.YXZ)
    pitch = max(-1.5, min(pitch + delta.y * 0.003, 1.5))
    yaw = yaw + delta.x * 0.004
    camera.rotation = Quat.from_euler(EulerRot.YXZ, yaw, pitch, roll)
    camera.translation = Vec3.ZERO - camera.forward() * 20.0
```

### Debug Camera Limitations (MCP Tools)

When using `capture_screenshot` or `capture_turnaround` with `position`/`look_at` overrides, the debug camera renders geometry from the specified viewpoint. However, **shader view uniforms** (`view.world_position` from `mesh_view_bindings`) may still reflect the scene camera's position due to render pipeline timing.

This means **view-dependent shader effects** - Fresnel, specular highlights, parallax, atmosphere rims - will appear incorrect in debug captures. All turnaround angles may show the same Fresnel/specular distribution.

- Use debug cameras for **geometry and layout verification**
- Use the **scene camera** (no position override) for **shader visual validation**
- Sun-direction-dependent effects (diffuse `dot(N, L)`) are view-independent and render correctly in debug captures

### Interior Camera Placement

For enclosed rooms, place the camera **inside** the walls at eye level. Use `get_bounding_box` on walls to compute safe bounds rather than guessing coordinates.

- **Position:** Keep `|x|` and `|z|` within `half_width - 1.0` to avoid clipping walls
- **Height:** Y = 1.5–2.0 (eye level)
- **`capture_turnaround` distance:** Use `room_radius * 0.7` or less to stay inside the room
