# Wireframe Rendering

Debug wireframe overlays on 3D meshes. Requires explicit `WireframePlugin` registration:

```python
from pybevy.pbr import Wireframe, WireframeColor, NoWireframe, WireframeConfig, WireframePlugin

@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_plugins(WireframePlugin())
```

## Per-Object Wireframe

Add `Wireframe()` to any mesh entity:

```python
# Default wireframe color (from WireframeConfig)
commands.spawn(
    Mesh3d(mesh), MeshMaterial3d(mat), Transform.from_xyz(0, 0, 0),
    Wireframe(),
)

# Custom wireframe color
commands.spawn(
    Mesh3d(mesh), MeshMaterial3d(mat), Transform.from_xyz(3, 0, 0),
    Wireframe(),
    WireframeColor(Color.srgb(1.0, 0.0, 0.0)),
)
```

## Global Wireframe Toggle

Use `WireframeConfig` resource to toggle wireframe on all meshes:

```python
# In Startup system
commands.insert_resource(WireframeConfig(
    global_=False,
    default_color=Color.WHITE,
))

# Toggle at runtime
def toggle_wireframe(keys: Res[ButtonInput], config: ResMut[WireframeConfig]) -> None:
    if keys.just_pressed(KeyCode.KeyW):
        config.global_ = not config.global_
```

## Excluding Objects

Add `NoWireframe()` to exclude an entity from global wireframe:

```python
commands.spawn(
    Mesh3d(ground_mesh), MeshMaterial3d(ground_mat),
    NoWireframe(),  # Never shows wireframe even with global_=True
)
```

## Line Width & Topology

```python
from pybevy.pbr import (
    WireframeConfig,
    WireframeLineWidth,
    WireframeTopology,
)

# Global defaults
commands.insert_resource(WireframeConfig(
    global_=True,
    default_line_width=2.0,
    default_topology=WireframeTopology.Quads,
))

# Per-entity overrides
commands.spawn(
    Mesh3d(mesh), MeshMaterial3d(mat), Wireframe(),
    WireframeLineWidth(3.0),
    WireframeTopology.Quads,
)
```

`WireframeTopology.Quads` draws quad edges (best-effort detection from
triangles); `Triangles` shows every triangle edge. Both topology values are
components and can override the global default per entity.

## Visual Tips

Wireframe lines are thin and hard to see with bright lighting. For best visibility:
- Use dark scenes or `unlit=True` materials
- Set a bright `default_color` in `WireframeConfig`
- Lower ambient light intensity when debugging geometry
