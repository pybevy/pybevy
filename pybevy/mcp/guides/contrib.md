# Contrib Plugins

Ready-made camera controllers and utilities in `pybevy.contrib`. Import and add to your app - no custom systems needed.

## OrbitCameraPlugin

Mouse-controlled camera that orbits around a target point.

```python
from pybevy.prelude import *
from pybevy.contrib import OrbitCameraPlugin, OrbitCamera

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(OrbitCameraPlugin())
        .add_systems(Startup, setup)
    )

def setup(commands: Commands) -> None:
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0, 50, 100).looking_at(Vec3.ZERO, Vec3.Y),
        Bloom(intensity=0.15),
        OrbitCamera(
            distance=100.0,
            yaw=0.0,
            pitch=0.4,
            target=Vec3.ZERO,
        ),
    )

if __name__ == "__main__":
    main().run()
```

### Controls

| Input | Action |
|-------|--------|
| Left mouse drag | Rotate camera around target |
| Shift + left mouse drag | Pan camera (move target point) |
| Mouse wheel | Zoom in/out |

### OrbitCamera Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `distance` | float | *(required)* | Distance from target point |
| `yaw` | float | *(required)* | Horizontal rotation angle (radians) |
| `pitch` | float | *(required)* | Vertical rotation angle (radians) |
| `target` | Vec3 | *(required)* | Point to orbit around |
| `rotate_sensitivity` | float | 0.003 | Mouse rotation sensitivity |
| `pan_sensitivity` | float | 0.001 | Mouse pan sensitivity multiplier |
| `zoom_sensitivity` | float | 0.1 | Mouse wheel zoom sensitivity |
| `min_distance` | float | 5.0 | Minimum zoom distance |
| `max_distance` | float | 1000.0 | Maximum zoom distance |

### Coordinate System

OrbitCamera uses **spherical coordinates** to position the camera around the target:

```
camera.x = target.x + distance * cos(pitch) * sin(yaw)
camera.y = target.y + distance * sin(pitch)
camera.z = target.z + distance * cos(pitch) * cos(yaw)
```

- **Yaw** (radians): Horizontal rotation around the **world Y-axis**. `yaw=0` places the camera on the +Z side of the target. Increasing yaw rotates counter-clockwise when viewed from above.
- **Pitch** (radians): Vertical elevation angle. `pitch=0` is level with the target. Positive values look down from above. Mouse-drag rotation clamps it to approximately ±1.47 rad (±84°); values assigned directly are not clamped.

Common starting values: `yaw=0.0, pitch=0.4` gives a slightly elevated view from the +Z side. `yaw=1.57` (~π/2) views from the +X side.

## FlyCameraPlugin

Free-moving camera with keyboard and mouse controls.

```python
from pybevy.prelude import *
from pybevy.contrib import FlyCameraPlugin, FlyCamera

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(FlyCameraPlugin())
        .add_systems(Startup, setup)
    )

def setup(commands: Commands) -> None:
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0, 10, 20).looking_at(Vec3.ZERO, Vec3.Y),
        Bloom(intensity=0.15),
        FlyCamera(move_speed=15.0),
    )

if __name__ == "__main__":
    main().run()
```

### Controls

| Input | Action |
|-------|--------|
| W / Up arrow | Move forward |
| S / Down arrow | Move backward |
| A / Left arrow | Strafe left |
| D / Right arrow | Strafe right |
| Q | Move down |
| E | Move up |
| Shift | Sprint (faster movement) |
| Right mouse + drag | Look around |

### FlyCamera Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `move_speed` | float | 10.0 | Movement speed (units/sec) |
| `sprint_multiplier` | float | 2.5 | Speed multiplier when shift held |
| `look_sensitivity` | float | 0.003 | Mouse look sensitivity |
| `pitch` | float | 0.0 | Current vertical rotation (radians) |
| `yaw` | float | 0.0 | Current horizontal rotation (radians) |
| `max_pitch` | float | ~1.56 | Maximum pitch angle (radians) |

## Direct Registration Pattern

For hot-reload compatibility, you can import individual components and systems from contrib modules instead of using the plugin:

```python
from pybevy.contrib.orbit_camera import OrbitCamera, orbit_camera_control_system, OrbitCameraState

@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .init_resource(OrbitCameraState)
        .add_systems(Update, orbit_camera_control_system)
    )
```

This gives you finer control over which systems are registered and allows mixing contrib systems with your own camera logic.

## See Also

- `guide://camera` - Camera positioning, post-processing, and manual camera movement patterns
- `guide://hot-reload` - How plugin build() works with hot reload
