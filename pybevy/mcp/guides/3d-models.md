# 3D Models (GLB/GLTF) Guide

Loading, positioning, and troubleshooting GLB/GLTF models: scene hierarchy, origin offsets, scale, and animations.

## Asset Path

PyBevy loads assets from `./assets/` relative to the current working directory (not the Python interpreter location). Place your models in `assets/` and load with paths relative to that root:

```python
# File at: ./assets/models/character.glb
handle = asset_server.load("models/character.glb#Scene0", Scene)
```

## Loading GLB Models

```python
from pybevy.prelude import *
from pybevy.scene import SceneRoot, Scene

@entrypoint
def main(app):
    app.add_plugins(DefaultPlugins)

    @app.main_system
    def setup(commands: Commands, asset_server: Res[AssetServer]):
        # Load model — note the #Scene0 suffix
        model_handle = asset_server.load("models/rabbit.glb#Scene0", Scene)
        commands.spawn(
            SceneRoot(model_handle),
            Transform.from_xyz(0.0, 0.0, 0.0),
            Name("rabbit"),
        )
```

## SceneRoot Hierarchy Structure

After spawning, a SceneRoot entity becomes a **parent** with auto-generated mesh children:

```
"rabbit" (SceneRoot, Transform, Name)      ← your named entity
  └─ "geometry_0.PBRMaterial" (Mesh3d, Aabb, GlobalTransform)  ← auto-generated
  └─ "geometry_1.PBRMaterial" (Mesh3d, Aabb, GlobalTransform)  ← auto-generated
```

**Key insight:** The parent entity has `Name` but **no `Aabb`**. The children have `Aabb` but generic names. Spatial tools (`get_bounding_box`, `check_overlaps`, `query_spatial`) automatically resolve through the hierarchy — querying the parent will merge child AABBs.

## Origin-at-Center Gotcha

Most 3D modeling tools export models with the origin at the geometric center. A rabbit that is 1.0 units tall will have its AABB from Y=-0.5 to Y=0.5. Spawning at `Transform.from_xyz(0, 0, 0)` means the bottom half is **below the ground plane** (Y=0).

**Fix: Apply a Y offset equal to half the model height.**

```python
# After loading, check the bounds:
# get_bounding_box(name="rabbit") → world.min_y = -0.5, world.max_y = 0.5
# height = 1.0, so y_offset = height / 2 = 0.5

commands.spawn(
    SceneRoot(model_handle),
    Transform.from_xyz(0.0, 0.5, 0.0),  # Lift by half height
    Name("rabbit"),
)
```

**Formula:** `y_offset = (world.max_y - world.min_y) / 2 - world.min_y` when spawned at origin.

## Model Scale

GLB model scale varies wildly between models. A fox might need `Vec3.splat(0.02)`, while a helmet needs `Vec3.splat(3.0)`. After first load, check the bounding box and adjust:

```python
# After loading, use MCP tools:
# get_bounding_box(name="model") → check dimensions
# Adjust scale to fit your scene (common range: 0.01–10.0)
commands.spawn(
    SceneRoot(model_handle),
    Transform.from_xyz(0.0, 0.0, 0.0).with_scale(Vec3.splat(0.02)),
    Name("model"),
)
```

## Animations

GLB models can contain skeletal animations. See `guide://animation` for the full pipeline: loading clips, building animation graphs, and controlling playback.

## Verification Workflow

1. Spawn model at origin, reload
2. `get_bounding_box(name="rabbit")` — check `world.min_y`
3. If `world.min_y < 0`: model is sunk, apply y_offset
4. `check_overlaps(ground_y=0.0)` — verify no sunken entities

**Note:** Sunken detection uses a minimum penetration threshold of 0.001 units. Very small negative `min_y` values below this threshold won't be flagged.

## Known Limitations

- `check_overlaps` without `ground_y` does **not** detect ground penetration — it only detects entity-vs-entity AABB overlaps and floating entities
- Use `ground_y` parameter to explicitly check for models sunk below a ground plane
- Generic child names (e.g., `geometry_0.PBRMaterial`) are annotated with `[parent: ...]` in spatial tool output for easier identification
- `get_bounding_box` requires child mesh entities to have rendered at least one frame (Aabb is computed by the render pipeline). If it returns 404 immediately after spawn, increase `delay_frames` or wait for a reload cycle.

## Model Facing Direction

GLB models have no guaranteed forward direction. Bevy's default forward is −Z,
but models may face +Z, +X, or any other direction.

**Diagnosing:** Spawn the model with `Quat.IDENTITY` rotation and capture a screenshot.
The visible front face reveals the model's native forward direction.

**Fixing:** If the model faces +Z instead of −Z:
- Flip yaw calculation: `atan2(dx, dz)` instead of `atan2(-dx, -dz)`
- Or apply 180° base rotation: `.with_rotation(Quat.from_euler(EulerRot.XYZ, 0, math.pi, 0))`

AI-generated models (Ludo, Meshy, Tripo) commonly face +Z.

## Common GLB Loading Errors

| Error | Cause | Fix |
|-------|-------|-----|
| "expected value at line 1 column 1" | File is gzip-compressed | Run `file model.glb` to check, `gunzip` if needed |
| "no default scene" | Missing scene suffix | Use `asset_server.load("model.glb#Scene0", Scene)` |
| Model loads but invisible | Wrong scale, buried in ground, or dark material | See troubleshooting below |

## Troubleshooting: Model Not Visible After Spawn

1. `capture_depth` at expected position — check if entity name appears in samples
2. `query_entities` with `["SceneRoot"]` — confirm entity exists
3. `get_component(name="...", component="Transform")` — verify position/scale
4. Check mesh children: `query_entities` with `["Mesh3d", "Aabb"]`
5. If model is dark: temporarily increase `GlobalAmbientLight(brightness=1000.0)`
6. Use `check_overlaps(ground_y=0.0)` to verify model isn't buried
