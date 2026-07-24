"""GPU particles via pybevy.contrib.hanabi (bevy_hanabi).

A continuous fountain in the center plus two one-shot bursts, all simulated
and rendered entirely on the GPU:
- EffectAsset.fountain() / EffectAsset.burst() presets
- ParticleEffect component spawned at a Transform
- HanabiPlugin
"""

import math

from pybevy.contrib.hanabi import EffectAsset, HanabiPlugin, ParticleEffect
from pybevy.prelude import *


def setup(
    commands: Commands,
    effects: ResMut[Assets[EffectAsset]],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Ground disc for spatial reference
    commands.spawn(
        Mesh3d(meshes.add(Circle(radius=4.0).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial.from_color(Color.srgb(0.1, 0.1, 0.12)))),
        Transform.from_rotation(Quat.from_rotation_x(-math.pi / 2)),
    )

    # Continuous fountain in the center
    fountain = effects.add(EffectAsset.fountain(rate=300.0, speed=(4.0, 6.0)))
    commands.spawn(ParticleEffect(fountain), Transform.from_xyz(0.0, 0.0, 0.0))

    # Two one-shot bursts, different colors
    boom = effects.add(
        EffectAsset.burst(count=800.0, colors=[(0.3, 0.7, 1.0, 1.0), (0.0, 0.2, 1.0, 0.0)])
    )
    commands.spawn(ParticleEffect(boom), Transform.from_xyz(-3.0, 1.0, 0.0))
    commands.spawn(ParticleEffect(boom), Transform.from_xyz(3.0, 1.0, 0.0))

    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3(0.0, 1.5, 0.0), Vec3.Y),
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(HanabiPlugin())
        .add_systems(Startup, setup)
    )


if __name__ == "__main__":
    main().run()
