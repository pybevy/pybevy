# Animated Scene Recipe

Moving objects: custom Velocity component, update system, and time.delta_secs(). Demonstrates the system-per-frame pattern.

```python
import math
from dataclasses import dataclass
from pybevy.prelude import *

@component
@dataclass
class Velocity(Component):
    x: float = 0.0
    y: float = 0.0
    z: float = 0.0

@component
@dataclass
class Bobbing(Component):
    speed: float = 2.0
    amplitude: float = 1.0
    phase: float = 0.0

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (apply_velocity, apply_bobbing))
    )

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(8.0, 6.0, 8.0).looking_at(Vec3.ZERO, Vec3.Y),
        Name("camera"),
    )

    # Light
    commands.spawn(
        DirectionalLight(illuminance=8000.0, shadows_enabled=True),
        Transform.from_rotation(Quat.from_euler(EulerRot.XYZ, -0.8, 0.4, 0.0)),
        Name("sun"),
    )
    commands.insert_resource(GlobalAmbientLight(brightness=200.0))

    # Ground
    ground = meshes.add(Plane3d(Vec3.Y, half_size=Vec2(10.0, 10.0)))
    ground_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.35, 0.35, 0.35),
    ))
    commands.spawn(Mesh3d(ground), MeshMaterial3d(ground_mat), Name("ground"))

    # Moving cube (linear velocity)
    cube = meshes.add(Cuboid(0.5, 0.5, 0.5))
    red_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(0.9, 0.2, 0.2),
    ))
    commands.spawn(
        Mesh3d(cube), MeshMaterial3d(red_mat),
        Transform.from_xyz(-4.0, 0.25, 0.0),
        Velocity(x=2.0, y=0.0, z=0.0),
        Name("moving_cube"),
    )

    # Bobbing spheres (sine wave motion)
    sphere = meshes.add(Sphere(0.3))
    for i in range(5):
        mat = materials.add(StandardMaterial(
            base_color=Color.srgb(0.2, 0.5 + i * 0.1, 0.8),
        ))
        commands.spawn(
            Mesh3d(sphere), MeshMaterial3d(mat),
            Transform.from_xyz(-2.0 + i * 1.0, 1.5, 3.0),
            Bobbing(speed=2.0 + i * 0.3, amplitude=0.8, phase=i * 0.5),
        )

def apply_velocity(
    query: Query[tuple[Mut[Transform], Velocity]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    for transform, vel in query:
        transform.translation = Vec3(
            transform.translation.x + vel.x * dt,
            transform.translation.y + vel.y * dt,
            transform.translation.z + vel.z * dt,
        )
        # Wrap around
        if transform.translation.x > 5.0:
            transform.translation = Vec3(-5.0, transform.translation.y, transform.translation.z)

def apply_bobbing(
    query: Query[tuple[Mut[Transform], Bobbing]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    for transform, bob in query:
        transform.translation = Vec3(
            transform.translation.x,
            1.5 + math.sin(t * bob.speed + bob.phase) * bob.amplitude,
            transform.translation.z,
        )

if __name__ == "__main__":
    main().run()
```

## Key points

- **`time.delta_secs()`** — seconds since last frame; use for velocity-based motion
- **`time.elapsed_secs()`** — total seconds since start; use for periodic/sine-wave motion
- **Custom components** need `@component` decorator and extend `Component`
- **`add_systems(Update, (sys1, sys2))`** — tuple registers multiple systems in one call
- **`Query[tuple[Mut[Transform], Velocity]]`** — `Mut` for writable, bare type for read-only
- Assign a whole new `Vec3` to `transform.translation` for reliable updates
- **Verify numerically**: Use `run_code` + `get_logs()` to print transform values at a paused moment — see the "Animation Verification" section in the scene-editing guide
