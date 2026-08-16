# Gizmos Guide

Draw immediate-mode debug geometry and visualize entity bounds and lights from Python systems.

## Setup

`DefaultPlugins` includes gizmo rendering. Add a system with a `Gizmos` parameter
and draw the geometry again on every frame that it should remain visible:

```python
from pybevy.prelude import (
    App,
    Camera3d,
    Color,
    Commands,
    DefaultPlugins,
    Gizmos,
    Startup,
    Transform,
    Update,
    Vec3,
    entrypoint,
)


def setup(commands: Commands) -> None:
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(4.0, 3.0, 6.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


def draw_axes(gizmos: Gizmos) -> None:
    gizmos.ray(Vec3.ZERO, Vec3.X * 2.0, Color.srgb(1.0, 0.0, 0.0))
    gizmos.ray(Vec3.ZERO, Vec3.Y * 2.0, Color.srgb(0.0, 1.0, 0.0))
    gizmos.ray(Vec3.ZERO, Vec3.Z * 2.0, Color.srgb(0.0, 0.4, 1.0))


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, draw_axes)
    )


if __name__ == "__main__":
    main().run()
```

Use `DefaultPlugins` for rendered gizmos. `GizmoPlugin` registers gizmo data but
does not replace the separate renderer that `DefaultPlugins` installs.

`Gizmos` is a system parameter, not a constructible service. Do not save it in a
global, resource, or component; an instance becomes invalid when its system call
returns. It is also rejected in run conditions because drawing mutates the gizmo
buffer.

Read `gizmos.config` to inspect the default group's configuration for the current
system call. It is a read-only borrowed reference and expires when the system
returns:

```python
def draw_when_enabled(gizmos: Gizmos) -> None:
    if gizmos.config.enabled:
        gizmos.line(Vec3.ZERO, Vec3.X, Color.WHITE)
```

Mutate the default group's persistent configuration through
`ResMut[GizmoConfigStore]`:

```python
from pybevy.prelude import GizmoConfigStore, ResMut


def configure_gizmos(store: ResMut[GizmoConfigStore]) -> None:
    config = store.config_mut()
    config.line.width = 4.0
    config.line.perspective = True
```

Pass a registered native config-group class to receive Bevy's two-part
configuration: the common rendering settings and the selected group's typed
settings. For example, this toggles the light group and enables drawing gizmos
for every light without affecting the default immediate-mode group:

```python
from pybevy.prelude import GizmoConfigStore, LightGizmoConfigGroup, ResMut


def toggle_light_gizmos(store: ResMut[GizmoConfigStore]) -> None:
    config, light_config = store.config_mut(LightGizmoConfigGroup)
    config.enabled = not config.enabled
    light_config.draw_all = True
```

## Lines, Rays, and Arrows

Use `line` when both endpoints are known. The second argument to `ray` is a
direction vector, not an endpoint:

```python
from pybevy.prelude import Color, Gizmos, Vec3


def draw_vectors(gizmos: Gizmos) -> None:
    origin = Vec3(0.0, 1.0, 0.0)

    gizmos.line(origin, Vec3(2.0, 1.0, 0.0), Color.WHITE)
    gizmos.ray(origin, Vec3(0.0, 2.0, 0.0), Color.srgb(0.2, 1.0, 0.4))
    gizmos.arrow(
        origin,
        Vec3(0.0, 1.0, 2.0),
        Color.srgb(1.0, 0.7, 0.1),
        tip_length=0.25,
        double_ended=True,
    )
```

`line_gradient` and `ray_gradient` accept separate start and end colors.
`linestrip` joins each position to the next, while `lineloop` also joins the
last position back to the first:

```python
def draw_path(gizmos: Gizmos) -> None:
    points = [
        Vec3(-2.0, 0.1, -1.0),
        Vec3(-1.0, 0.1, 1.0),
        Vec3(1.0, 0.1, 1.5),
        Vec3(2.0, 0.1, -1.0),
    ]
    gizmos.linestrip(points, Color.srgb(0.9, 0.3, 1.0))
```

Prefer one `linestrip` or `lineloop` call over many individual `line` calls for
a connected path.

## 3D Shapes

Shapes use `Isometry3d` for translation and rotation. The isometry has no scale;
size is passed separately:

```python
from pybevy.prelude import Color, Gizmos, Isometry3d, Quat, Vec2, Vec3


def draw_shapes(gizmos: Gizmos) -> None:
    frame = Isometry3d(
        translation=Vec3(0.0, 1.5, 0.0),
        rotation=Quat.IDENTITY,
    )

    gizmos.rect(frame, Vec2(3.0, 2.0), Color.WHITE)
    gizmos.cross(frame, 0.5, Color.srgb(1.0, 0.2, 0.2))
    gizmos.circle(frame, 1.0, Color.srgb(0.2, 0.8, 1.0), resolution=48)
    gizmos.ellipse(
        frame,
        Vec2(1.5, 0.75),
        Color.srgb(0.8, 0.4, 1.0),
        resolution=48,
    )
    gizmos.sphere(frame, 1.0, Color.srgb(1.0, 0.8, 0.2), resolution=32)
```

- `rect` takes the full rectangle size.
- `cross` takes a half-size.
- `ellipse` takes half-sizes for its two axes.
- `circle` and `sphere` take a radius.
- `resolution` controls curved-shape tessellation; higher values are smoother
  but emit more line segments.

## 2D Drawing

The 2D methods use `Vec2` and `Isometry2d`:

```python
from pybevy.prelude import Color, Gizmos, Isometry2d, Vec2


def draw_2d_debug(gizmos: Gizmos) -> None:
    gizmos.line_2d(Vec2(-100.0, 0.0), Vec2(100.0, 0.0), Color.WHITE)
    gizmos.ray_2d(Vec2.ZERO, Vec2(0.0, 80.0), Color.srgb(0.2, 1.0, 0.3))
    gizmos.arrow_2d(
        Vec2(-60.0, -40.0),
        Vec2(60.0, 40.0),
        Color.srgb(1.0, 0.5, 0.1),
        tip_length=12.0,
    )

    frame = Isometry2d.from_xy(0.0, 80.0)
    gizmos.rect_2d(frame, Vec2(120.0, 50.0), Color.srgb(0.4, 0.8, 1.0))
    gizmos.circle_2d(frame, 35.0, Color.srgb(1.0, 0.3, 0.7))
```

The remaining 2D counterparts are `line_gradient_2d`, `ray_gradient_2d`,
`linestrip_2d`, `lineloop_2d`, `cross_2d`, and `ellipse_2d`.

## Entity Bounds

Add `ShowAabbGizmo` to an entity whose renderer supplies an axis-aligned
bounding box, such as a mesh entity. The marker belongs on the same entity as
the render component:

```python
from pybevy.prelude import (
    Color,
    Commands,
    Entity,
    Mesh3d,
    Query,
    ShowAabbGizmo,
    Without,
)


def show_mesh_bounds(
    commands: Commands,
    meshes: Query[tuple[Entity, Mesh3d], Without[ShowAabbGizmo]],
) -> None:
    for entity, _mesh in meshes:
        commands.entity(entity).insert(
            ShowAabbGizmo(Color.srgb(0.1, 1.0, 0.3)),
        )
```

`ShowAabbGizmo()` uses the AABB gizmo group's default; without a configured
group color, Bevy derives a varied color from the entity. An entity without an
AABB has nothing to draw. For imported scenes, attach the marker to the mesh
child rather than only to an empty hierarchy root.

## Light Gizmos

Add `ShowLightGizmo` to the same entity as a point, spot, directional, or rect
light:

```python
from pybevy.prelude import (
    Color,
    Commands,
    LightGizmoColor,
    PointLight,
    ShowLightGizmo,
    Transform,
)


def spawn_debug_light(commands: Commands) -> None:
    commands.spawn(
        PointLight(intensity=80_000.0, range=8.0),
        Transform.from_xyz(0.0, 3.0, 0.0),
        ShowLightGizmo(LightGizmoColor.ByLightType()),
    )

    commands.spawn(
        PointLight(intensity=40_000.0, range=5.0),
        Transform.from_xyz(3.0, 2.0, 0.0),
        ShowLightGizmo(
            LightGizmoColor.Manual(Color.srgb(1.0, 0.2, 0.8)),
        ),
    )
```

Available color strategies are:

- `LightGizmoColor.Manual(color)`
- `LightGizmoColor.Varied()`
- `LightGizmoColor.MatchLightColor()`
- `LightGizmoColor.ByLightType()`

`ShowLightGizmo()` uses `MatchLightColor` by default.

## MCP Screenshots

The normal game window renders enabled gizmos. MCP captures hide them by
default so debug overlays do not contaminate scene-quality measurements. Opt in
when the gizmos are the subject of the capture:

```
capture_screenshot {"gizmos": true}
capture_stats {"gizmos": true, "grid": 4}
capture_timeline {"gizmos": true, "capture_count": 6}
```

The option includes existing gizmos; it does not generate labels or debug
geometry by itself. Your drawing system must run during the frames leading up
to capture. See `guide://scene-editing` for capture timing and deterministic
comparison workflows.

## Current Surface

PyBevy exposes immediate lines, rays, arrows, strips, loops, rectangles,
crosses, ellipses, circles, and spheres, plus AABB and light marker components.
Custom Python gizmo config groups, retained `GizmoAsset` values, grids, arcs,
and the rest of Bevy's builder API are not currently exposed. The standard
`LightGizmoConfigGroup` and its common `GizmoConfig` are available as a typed
pair through `GizmoConfigStore`. Compose unsupported shapes from lines or line
strips. Full hot reload preserves the plugin-populated store, including the
light gizmo group; scene entities are still rebuilt normally.
