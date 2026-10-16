# Material binding examples

Run from the repository root with PyBevy installed, for example:

```bash
python examples/materials/structured_buffer.py
python examples/materials/cubemap.py
python examples/materials/array_texture.py
python examples/materials/volume_texture.py
python examples/materials/independent_samplers.py
```

All assets are generated in Python. Use `ShaderMaterialPlugin()` for all custom
material layouts; older examples using `StorageShaderMaterialPlugin()` remain
supported. Each example imports its generated material bindings in the matching
[WGSL shaders](../../assets/shaders/examples/).

| Scene | What to inspect |
|---|---|
| [Structured buffers](structured_buffer.py) | Animated tiles read a `Tile` struct array through a second `u32` index buffer. `@shader_struct` generates WGSL and packs the 32-byte stride: `vec3` offset, `f32` scale, `vec4` color. Both buffers update in place. |
| [Cubemap](cubemap.py) | A sphere reflects six procedural faces while the sampling direction rotates. Cube layer order is +X, -X, +Y, -Y, +Z, -Z. The image has a cube view and linear filtering. |
| [Texture array](array_texture.py) | Four checkerboard layers share one image and material. `MeshTag` selects each mesh's layer; a uniform offset cycles the layers. Nearest filtering keeps the checks crisp. |
| [Independent samplers](independent_samplers.py) | One image and material supply both panels. Left uses nearest filtering; right uses linear filtering through a separate sampler binding. |
| [Volume slices](volume_texture.py) | A 32³ D3 texture stores a colored wave. A uniform sweeps through Z; linear filtering interpolates adjacent slices. X is the fastest-varying upload coordinate. |

These are PyBevy teaching examples inspired by Bevy's material APIs. The array
example uses the same per-mesh layer technique as Bevy 0.19's `array_texture`;
it is not a complete port of that scene. For a direct Bevy port, see
[storage_buffer.py](../bevy/shaders/storage_buffer.py).

The shaders demonstrate forward rendering with direct color outputs. The
structured example's deformation needs matching prepass/deferred shaders if
extended to cast deformed shadows or produce motion vectors. Cubemap sampling
here has no roughness-prefiltered PBR lighting, and the volume scene displays
slices rather than ray marching. See the [shader guide](../../pybevy/mcp/guides/shaders.md)
for those extension points and binding limits.

## Verification

All five entrypoints pass the standard 60-frame smoke test on Vulkan with Xvfb.
Headless tests render their actual setup systems, check visible pixels, and
verify buffer selection, record edits, environment rotation, array-layer
selection and volume-slice changes. Shader and wgpu validation errors fail the
tests. CPU-authored depth comparisons have separate 2D/array/cube/cube-array
pixel tests, including sampler replacement and invalid-sampler recovery.

```bash
pytest -q tests/integration/test_material_examples.py tests/integration/test_material_textures.py tests/integration/test_material_samplers.py
xvfb-run -a pytest -q -m example_smoke tests/example_smoke/test_examples.py -k 'materials/'
```

The shared Rust validator also has synthetic reduced-limit boundary tests.
Those do not establish support on a physical lower-limit GPU or another
backend. RP2 material adapters remain pending.
