# Mesh Guide

Creating 3D and 2D meshes from primitives, custom vertices, and zero-copy NumPy access.

## Primitive Meshes

**Two patterns** for creating meshes:

### Direct Pass (Simple)
Pass shapes directly — PyBevy auto-converts to Mesh:
```python
cube_mesh = meshes.add(Cuboid(1.0, 1.0, 1.0))
sphere_mesh = meshes.add(Sphere(radius=0.5))
torus_mesh = meshes.add(Torus(0.3, 1.0))
```

### Builder Pattern (Custom Resolution)
Use `.mesh()` for resolution control. Some methods return `Mesh` directly, others need `.build()`:
```python
meshes.add(Sphere(0.5).mesh().uv(32, 18))             # .uv() returns Mesh directly
meshes.add(Sphere(0.5).mesh().ico(3))                  # .ico() returns Mesh directly
meshes.add(Torus(0.3, 1.0).mesh().minor_resolution(24).major_resolution(48).build())
meshes.add(Plane3d(Vec3.Y, Vec2(5.0, 5.0)).mesh().subdivisions(4).build())
```

**Use direct pass unless you need resolution control.**

### Available Primitives

| Primitive | Constructor | Notes |
|-----------|------------|-------|
| `Cuboid` | `Cuboid(x, y, z)` | Box with full sizes (x_length, y_length, z_length) |
| `Sphere` | `Sphere(radius)` | Ico or UV sphere |
| `Cylinder` | `Cylinder(radius, height)` | Y-axis aligned |
| `Cone` | `Cone(radius, height)` | Base at origin, points +Y |
| `Torus` | `Torus(inner, outer)` | Lies flat in XZ plane (hole faces +Y). To align hole along Z (tunnels/portals): `transform.rotation = Quat.from_euler(EulerRot.XYZ, math.pi / 2.0, 0.0, 0.0)` |
| `Capsule3d` | `Capsule3d(radius, length)` | Y-axis aligned, rounded ends |
| `Plane3d` | `Plane3d(normal)` | Infinite plane (mesh is finite) |
| `Circle` | `Circle(radius)` | 2D disc |
| `Rectangle` | `Rectangle(width, height)` | 2D quad |
| `Annulus` | `Annulus(inner, outer)` | 2D ring |
| `Ellipse` | `Ellipse(half_x, half_y)` | 2D ellipse |
| `RegularPolygon` | `RegularPolygon(circumradius, sides)` | |
| `Triangle2d` | `Triangle2d(a, b, c)` | From Vec2 points |
| `Triangle3d` | `Triangle3d(a, b, c)` | From Vec3 points |
| `Tetrahedron` | `Tetrahedron(a, b, c, d)` | From Vec3 points |

### Mesh Builder Options

Some builders have extra configuration:

```python
# Sphere variants — .ico() and .uv() return Mesh directly (no .build())
Sphere(0.5).mesh().ico(3)     # Icosphere with 3 subdivisions
Sphere(0.5).mesh().uv(32, 18) # UV sphere with sectors/stacks

# Plane with subdivisions
Plane3d(Vec3.Y, Vec2(5.0, 5.0)).mesh().subdivisions(4).build()

# Torus resolution
Torus(0.3, 1.0).mesh().minor_resolution(24).major_resolution(48).build()

# Capsule detail
Capsule3d(0.5, 2.0).mesh().rings(4).longitudes(32).latitudes(16).build()
```

## Custom Meshes from Vertices

Create meshes directly from vertex data using NumPy arrays or lists:

```python
import numpy as np
from pybevy.prelude import *

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Create a triangle
    mesh = Mesh(PrimitiveTopology.TriangleList)

    positions = np.array([
        [0.0, 1.0, 0.0],
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
    ], dtype=np.float32)

    normals = np.array([
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ], dtype=np.float32)

    uvs = np.array([
        [0.5, 0.0],
        [0.0, 1.0],
        [1.0, 1.0],
    ], dtype=np.float32)

    mesh.insert_attribute(Mesh.ATTRIBUTE_POSITION, positions)
    mesh.insert_attribute(Mesh.ATTRIBUTE_NORMAL, normals)
    mesh.insert_attribute(Mesh.ATTRIBUTE_UV_0, uvs)
    mesh.insert_indices(np.array([0, 1, 2], dtype=np.uint32))

    material = materials.add(StandardMaterial())
    commands.spawn(
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
    )
```

### Winding Order & Backface Culling

Bevy renders **counter-clockwise** triangles as front faces. `StandardMaterial` has backface culling enabled by default (`cull_mode=Face.Back`).

If your custom mesh is invisible but `get_bounding_box` confirms it exists:
- Your triangle winding order is likely clockwise (back faces showing)
- Quick debug: set `double_sided=True` or `cull_mode=None` on the material
- Fix: reverse your index order (swap every pair of vertices in each triangle)

**Ground overlays:** Flat meshes at Y < 0.1 are nearly invisible from eye-level cameras due to foreshortening. For paths, roads, or ground markings, use thin 3D geometry (e.g., `Cylinder(0.35, 0.08)` stepping stones) instead of flat triangle strips.

### Builder Pattern (Chaining)

Use `with_inserted_attribute` for a functional style:

```python
mesh = (
    Mesh(PrimitiveTopology.TriangleList)
    .with_inserted_attribute(Mesh.ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh.ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh.ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(indices)
)
```

### Vertex Attributes

| Attribute | Format | Per-Vertex Shape |
|-----------|--------|------------------|
| `Mesh.ATTRIBUTE_POSITION` | float32 | (N, 3) |
| `Mesh.ATTRIBUTE_NORMAL` | float32 | (N, 3) |
| `Mesh.ATTRIBUTE_UV_0` | float32 | (N, 2) |
| `Mesh.ATTRIBUTE_UV_1` | float32 | (N, 2) |
| `Mesh.ATTRIBUTE_TANGENT` | float32 | (N, 4) |
| `Mesh.ATTRIBUTE_COLOR` | float32 | (N, 4) — RGBA |
| `Mesh.ATTRIBUTE_JOINT_WEIGHT` | float32 | (N, 4) |
| `Mesh.ATTRIBUTE_JOINT_INDEX` | uint16 | (N, 4) |

### Vertex Colors

```python
colors = np.array([
    [1.0, 0.0, 0.0, 1.0],  # Red
    [0.0, 1.0, 0.0, 1.0],  # Green
    [0.0, 0.0, 1.0, 1.0],  # Blue
], dtype=np.float32)

mesh.insert_attribute(Mesh.ATTRIBUTE_COLOR, colors)
```

Use with a material that respects vertex colors (e.g., `StandardMaterial` with default settings).

## Zero-Copy Mesh Access

For high-performance mesh manipulation, use context managers that provide direct NumPy array views without copying.

### Read-Only Access

```python
def analyze_mesh(
    meshes: Res[Assets[Mesh]],
    query: Query[Mesh3d],
) -> None:
    for mesh3d in query:
        mesh = meshes.get(mesh3d.handle)
        if mesh:
            with mesh.positions() as positions:
                center = positions.mean(axis=0)
                max_y = positions[:, 1].max()
```

### Mutable Access

```python
def deform_mesh(
    time: Res[Time],
    meshes: ResMut[Assets[Mesh]],
    query: Query[Mesh3d],
) -> None:
    for mesh3d in query:
        mesh = meshes.get_mut(mesh3d.handle)
        if mesh:
            t = time.elapsed_secs()
            with mesh.positions_mut() as positions:
                positions[:, 1] += np.sin(positions[:, 0] + t) * 0.01
```

### Available Context Managers

| Method | Access | Shape |
|--------|--------|-------|
| `mesh.positions()` | read-only | (N, 3) |
| `mesh.positions_mut()` | mutable | (N, 3) |
| `mesh.normals()` | read-only | (N, 3) |
| `mesh.normals_mut()` | mutable | (N, 3) |
| `mesh.uvs()` | read-only | (N, 2) |
| `mesh.uvs_mut()` | mutable | (N, 2) |
| `mesh.attribute(attr)` | read-only | varies |
| `mesh.attribute_mut(attr)` | mutable | varies |

### Copy Methods (Safe Alternative)

When you don't need zero-copy performance:

```python
positions = mesh.positions_copy()     # Returns owned np.ndarray
mesh.set_positions(new_positions)     # Copies data into mesh

normals = mesh.normals_copy()
mesh.set_normals(new_normals)
```

## Tangent Generation

Auto-generate tangents (required for normal mapping):

```python
# Builder style
mesh = mesh.with_generated_tangents()

# In-place
mesh.generate_tangents()
```

## Mesh Topology Types

| Topology | Use Case |
|----------|----------|
| `PrimitiveTopology.TriangleList` | Standard meshes (most common) |
| `PrimitiveTopology.TriangleStrip` | Optimized triangle strips |
| `PrimitiveTopology.LineList` | Wireframe / debug lines |
| `PrimitiveTopology.LineStrip` | Connected line sequences |
| `PrimitiveTopology.PointList` | Point clouds |

## 2D Meshes

For 2D rendering, use `Mesh2d` with `MeshMaterial2d`:

```python
from pybevy.prelude import *

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ColorMaterial]],
) -> None:
    commands.spawn(
        Mesh2d(meshes.add(Circle(50.0))),
        MeshMaterial2d(materials.add(ColorMaterial(color=Color.srgb(0.2, 0.8, 0.2)))),
    )
```

## Asset Paths

Bevy's asset server loads files from the `assets/` directory relative to the working directory:

```
my_project/
├── assets/
│   ├── models/
│   │   └── character.glb
│   ├── textures/
│   │   └── ground.png
│   └── sounds/
│       └── music.ogg
├── main.py
```

```python
asset_server.load("models/character.glb")     # assets/models/character.glb
asset_server.load("bevy/textures/ground.png")       # assets/textures/ground.png
```

Paths are always relative to `assets/` — do not include `assets/` in the path string.
