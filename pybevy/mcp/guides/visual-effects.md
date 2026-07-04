# Visual Effects Recipes

Material + lighting combinations for specific visual effects. These go beyond surface realism — they're techniques for achieving a *look*.

## Rim Lighting (Fresnel Silhouettes)

Dark objects that only show their edges when backlit. The PBR Fresnel effect makes grazing angles reflective, so a strong light behind an object creates bright rim outlines on an otherwise invisible surface.

**Material:**
```python
silhouette_mat = materials.add(StandardMaterial(
    base_color=Color.srgb(0.02, 0.02, 0.03),  # Near-black face color
    metallic=0.0,
    perceptual_roughness=0.15,  # Smooth = sharper rim
    reflectance=1.0,            # Max Fresnel = strongest rim
))
```

**Why it works:**
- `base_color` near black → front-facing surfaces are invisible
- `reflectance=1.0` → maximum Fresnel effect at grazing angles
- `perceptual_roughness` low → rim highlight is tight and clean (higher = softer, wider rim)
- `metallic=0.0` → non-metallic materials have stronger dielectric Fresnel

**Lighting setup:** Place the key light *behind* the objects relative to the camera:
```python
# Light pointing toward the camera (backlight)
commands.spawn(
    DirectionalLight(illuminance=6000.0, color=Color.srgb(0.6, 0.7, 1.0), shadow_maps_enabled=True),
    Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.3, 3.14, 0.0)),
)
# Keep ambient very low so front faces stay dark
commands.insert_resource(GlobalAmbientLight(brightness=15.0, color=Color.srgb(0.3, 0.35, 0.5)))
```

**Tuning the rim:**

| Parameter | Lower | Higher |
|-----------|-------|--------|
| `reflectance` | Fainter rim | Brighter rim (1.0 = max) |
| `perceptual_roughness` | Sharp thin edge | Soft wide glow |
| `base_color` brightness | Purer silhouette | More visible fill |
| Backlight `illuminance` | Subtle edge | Blazing rim |

**Combine with:** Bloom on camera amplifies the rim into a halo. DistanceFog fades distant rims for depth.

## Emissive Path Markers

Ground-level navigation aids that glow through darkness and fog.

```python
# Cool blue path strip
marker_mat = materials.add(StandardMaterial(
    base_color=Color.srgb(0.01, 0.01, 0.02),
    emissive=LinearRgba.rgb(0.4, 0.8, 2.5),
))
strip_mesh = meshes.add(Cuboid(0.15, 0.04, 0.8))  # Thin, flat
commands.spawn(Mesh3d(strip_mesh), MeshMaterial3d(marker_mat), Transform.from_xyz(x, 0.02, z))

# Warm amber waypoint beacon
beacon_mat = materials.add(StandardMaterial(
    base_color=Color.srgb(0.02, 0.01, 0.005),
    emissive=LinearRgba.rgb(2.0, 0.8, 0.2),
))
```

**Tips:**
- Place strips at `y=0.02` (just above ground) to avoid z-fighting
- Use two colors (cool path + warm waypoints) so the player reads structure vs. decision points
- Emissive values 1.0–3.0 are visible in darkness; 5.0+ blooms aggressively
- `fog_enabled=True` (default) lets fog attenuate distant markers naturally

## Dark Fog Atmosphere

Intentionally near-black fog for horror/mystery/silhouette scenes. Different from typical gray fog — the goal is to *hide* the world and reveal only lit edges.

```python
DistanceFog(
    color=Color.srgb(0.04, 0.04, 0.06),        # Near-black, not gray
    falloff=FogFalloff.Exponential(0.05-0.08),   # Very dense
    directional_light_color=Color.srgb(0.5, 0.55, 0.7),  # Cool backscatter
    directional_light_exponent=80.0,             # Tight light cone through fog
)
```

**Key differences from standard fog:**

| | Standard fog | Dark fog |
|---|---|---|
| Fog color | Gray (0.5–0.7) | Near-black (0.03–0.06) |
| Density | 0.002–0.01 | 0.05–0.08 |
| Ambient light | 100–300 | 5–20 |
| ClearColor | Sky color | Near-black |
| Primary visibility | Surfaces + shadows | Emissive + rim only |

**Critical:** Set `ClearColor` to match the fog color, otherwise distant objects fade to a mismatched background:
```python
commands.insert_resource(ClearColor(Color.srgb(0.015, 0.015, 0.025)))
```

## Mesh-Based Mist Particles

When volumetric FogVolumes aren't enough visual detail, supplement with flat mesh "puffs" that drift slowly. These add texture to mist without real particle systems.

**Material:** Use `AlphaMode.Add()` + `unlit=True` so puffs brighten over any background without casting shadows:

```python
mist_mat = materials.add(StandardMaterial(
    base_color=Color.srgba(0.85, 0.88, 0.92, 0.06),  # Very low alpha
    alpha_mode=AlphaMode.Add(),
    unlit=True,
))
```

**Mesh:** Ultra-flat cuboids (height 0.02) in varied sizes. Multiple sizes prevent visual repetition:

```python
mist_sizes = [
    meshes.add(Cuboid(2.0, 0.02, 1.8)),
    meshes.add(Cuboid(1.6, 0.02, 2.2)),
    meshes.add(Cuboid(2.5, 0.02, 2.0)),
]
```

**Spawn in a ring** around the mist source with random-ish rotation to break up alignment:

```python
for m in range(60):
    angle = m * math.tau / 60.0 + math.sin(m * 1.3) * 0.7
    radius = 1.0 + (m % 9) * 1.2
    y = 0.15 + (m % 5) * 0.25
    commands.spawn(
        Mesh3d(mist_sizes[m % 3]), MeshMaterial3d(mist_mat),
        Transform.from_xyz(math.cos(angle) * radius, y, math.sin(angle) * radius)
            .with_rotation(Quat.from_euler(EulerRot.XYZ, 0.0, m * 0.5, 0.0)),
        MistDrift(phase=m * 0.7, speed=0.1 + (m % 3) * 0.06, base_y=y),
    )
```

**Animate** with slow circular drift + gentle vertical bob in an Update system:

```python
def animate_mist(query: Query[tuple[Mut[Transform], MistDrift]], time: Res[Time]) -> None:
    t = time.elapsed_secs()
    for transform, drift in query:
        transform.translation.x += math.sin(t * drift.speed + drift.phase) * 0.003
        transform.translation.z += math.cos(t * drift.speed * 0.7 + drift.phase) * 0.002
        transform.translation.y = drift.base_y + math.sin(t * 0.4 + drift.phase) * 0.15
```

**Tips:**
- Alpha 0.04–0.08 for additive blend — higher looks like solid planes
- These look best at mid-range. Up close, the flat geometry is visible. Use FogVolumes for the heavy lifting, mesh puffs for texture
- Vary `base_y` so lower puffs are denser (more opaque material) and higher puffs use a lighter material

## Burst Particle Effects

Event-driven particles (sparks, explosions, impacts) — spawn a batch, animate with gravity, despawn on timeout.

### Components

```python
@component
@dataclass
class Particle(Component):
    vel: Vec3 = field(default_factory=lambda: Vec3.ZERO)
    life: float = 1.0
```

### Spawn Burst

Cache mesh/material in a `@resource` (see `guide://performance`). Spawn 20–30 per burst max:

```python
def spawn_sparks(commands: Commands, assets: Res[SparkAssets]) -> None:
    for i in range(20):
        angle = i * math.tau / 20.0 + math.sin(i * 2.3) * 0.4
        speed = 3.0 + (i % 5) * 0.8
        commands.spawn(
            Mesh3d(assets.mesh), MeshMaterial3d(assets.mat),
            Transform.from_xyz(0, 1, 0),
            Particle(velocity=Vec3(math.cos(angle) * speed, 4.0 + (i % 3), math.sin(angle) * speed), life=1.0),
        )
```

### Animate + Despawn

```python
def animate_particles(
    commands: Commands,
    query: Query[tuple[Entity, Mut[Transform], Mut[Particle]]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    for entity, t, p in query:
        p.vel.y -= 9.8 * dt   # gravity
        t.translation.x += p.vel.x * dt
        t.translation.y += p.vel.y * dt
        t.translation.z += p.vel.z * dt
        p.life -= dt
        if p.life <= 0.0:
            commands.entity(entity).despawn()
```

**Tips:** Use `emissive` + `unlit=True` for glowing sparks. Scale down over lifetime for a fade effect (`t.scale = Vec3.splat(p.life)`).
