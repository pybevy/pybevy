# 3D Models (GLB/GLTF) Guide

Loading, positioning, and troubleshooting GLB/GLTF models: scene hierarchy, origin offsets, scale, and animations.

`GltfMesh.primitives` is a read-only live sequence whose `GltfPrimitive`
elements remain linked to the loaded mesh. `list(mesh.primitives)` creates an
independent Python container, but its primitive elements remain live views.

## Asset Path

PyBevy loads assets from `./assets/` relative to the current working directory (not the Python interpreter location). Place your models in `assets/` and load with paths relative to that root:

```python
# File at: ./assets/models/character.glb
handle = asset_server.load("models/character.glb#Scene0", WorldAsset)
```

## Loading GLB Models

```python
from pybevy.prelude import *
from pybevy.world_serialization import WorldAsset, WorldAssetRoot

def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    # Load model - note the #Scene0 suffix
    model_handle = asset_server.load("models/rabbit.glb#Scene0", WorldAsset)
    commands.spawn(
        WorldAssetRoot(model_handle),
        Transform.from_xyz(0.0, 0.0, 0.0),
        Name("rabbit"),
    )

@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)

if __name__ == "__main__":
    main().run()
```

## WorldAssetRoot Hierarchy Structure

After spawning, a WorldAssetRoot entity becomes a **parent** with auto-generated mesh children:

```
"rabbit" (WorldAssetRoot, Transform, Name)      ← your named entity
  └─ "geometry_0.PBRMaterial" (Mesh3d, Aabb, GlobalTransform)  ← auto-generated
  └─ "geometry_1.PBRMaterial" (Mesh3d, Aabb, GlobalTransform)  ← auto-generated
```

**Key insight:** The parent entity has `Name` but **no `Aabb`**. The children have `Aabb` but generic names. Spatial tools (`get_bounding_box`, `check_overlaps`, `query_spatial`) automatically resolve through the hierarchy - querying the parent will merge child AABBs.

## Origin-at-Center Gotcha

Most 3D modeling tools export models with the origin at the geometric center. A rabbit that is 1.0 units tall will have its AABB from Y=-0.5 to Y=0.5. Spawning at `Transform.from_xyz(0, 0, 0)` means the bottom half is **below the ground plane** (Y=0).

**Fix: Apply a Y offset equal to half the model height.**

```python
# After loading, check the bounds:
# get_bounding_box(entity="rabbit") reports world.min_y = -0.5, world.max_y = 0.5
# height = 1.0, so y_offset = height / 2 = 0.5

commands.spawn(
    WorldAssetRoot(model_handle),
    Transform.from_xyz(0.0, 0.5, 0.0),  # Lift by half height
    Name("rabbit"),
)
```

**General formula:** `y_offset = -world.min_y` for a ground plane at Y=0.
Add that offset to the entity's current Y translation. The half-height shortcut
only applies when the model is centered on its origin.

## Model Scale

GLB model scale varies wildly between models. A fox might need `Vec3.splat(0.02)`, while a helmet needs `Vec3.splat(3.0)`. After first load, check the bounding box and adjust:

```python
# After loading, use MCP tools:
# get_bounding_box(entity="model") → check dimensions
# Adjust scale to fit your scene (common range: 0.01–10.0)
commands.spawn(
    WorldAssetRoot(model_handle),
    Transform.from_xyz(0.0, 0.0, 0.0).with_scale(Vec3.splat(0.02)),
    Name("model"),
)
```

## Animations

GLB models can contain skeletal animations. See `guide://animation` for the full pipeline: loading clips, building animation graphs, and controlling playback.

## Loader Settings

Use `GltfLoaderSettings` when a load needs different content, validation,
coordinate-conversion, or skinned-mesh bounds behavior:

```python
from pybevy.gltf import (
    Gltf,
    GltfConvertCoordinates,
    GltfLoaderSettings,
    GltfSkinnedMeshBoundsPolicy,
)

settings = GltfLoaderSettings(
    load_cameras=False,
    convert_coordinates=GltfConvertCoordinates(rotate_meshes=True),
    skinned_mesh_bounds_policy=GltfSkinnedMeshBoundsPolicy.Dynamic,
)
handle = asset_server.load_with_settings("models/character.glb", Gltf, settings)
```

Coordinate conversion is experimental in Bevy. Keep `validate=True` unless the
asset must bypass glTF validation deliberately.

## Verification Workflow

1. Spawn model at origin, reload
2. `get_bounding_box(entity="rabbit")` - check `world.min_y`
3. If `world.min_y < 0`: model is sunk, apply y_offset
4. `check_all_overlaps {"ground_y": 0.0}` - verify no sunken entities

**Note:** Sunken detection uses a minimum penetration threshold of 0.001 units. Very small negative `min_y` values below this threshold won't be flagged.

## Known Limitations

- `check_all_overlaps` without `ground_y` does **not** detect ground penetration - it only detects entity-vs-entity AABB overlaps and floating entities
- Use `ground_y` parameter to explicitly check for models sunk below a ground plane
- Generic or repeated child names (e.g., `geometry_0.PBRMaterial` or `cube`) are annotated with the nearest uniquely named ancestor as `[parent: ...]`
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
| "Could not find an asset loader matching" for a valid GLB loaded as `WorldAsset` | Missing scene suffix | Use `asset_server.load("model.glb#Scene0", WorldAsset)` |
| Model loads but invisible | Wrong scale, buried in ground, dark material, or a missing external texture | See troubleshooting below |

The loader message is not specific to a missing scene suffix. If `#Scene0` is
already present, also verify that the file exists, is a valid GLB, and that the
required asset plugins are installed.

## Troubleshooting: Model Not Visible After Spawn

1. `capture_depth` at expected position - check if entity name appears in samples
2. `query_entities` with `["WorldAssetRoot"]` - confirm entity exists
3. `get_component(entity="...", component="Transform")` - verify position/scale
4. Check mesh children: `query_entities` with `["Mesh3d", "Aabb"]`
5. If model is dark: temporarily increase `GlobalAmbientLight(brightness=1000.0)`
6. Use `check_all_overlaps {"ground_y": 0.0}` to verify the model isn't buried
7. Search `get_logs` for `Path not found`: a GLB referencing an external texture
   that was not bundled still loads with a correct entity count and bounding box,
   but its fallback material can render the model completely invisible rather
   than merely untextured. `get_last_error` stays null, so the logs are the only
   signal. Bundle every texture the GLB references.
