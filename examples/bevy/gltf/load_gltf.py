"""Load and render a GLTF file as a scene.

This example demonstrates:
- Loading GLTF assets using GltfPlugin
- Spawning GLTF scenes using SceneRoot
- Setting up lighting and camera for 3D rendering
"""

from pybevy.app import App, DefaultPlugins, Startup
from pybevy.assets import AssetServer
from pybevy.camera import Camera3d
from pybevy.decorators import entrypoint
from pybevy.ecs import Commands, Res
from pybevy.light import DirectionalLight, DirectionalLightShadowMap
from pybevy.math import Vec3
from pybevy.scene import Scene, SceneRoot
from pybevy.transform import Transform


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(DirectionalLightShadowMap(size=4096))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
    )


def setup(commands: Commands, server: Res[AssetServer]) -> None:
    """Set up the scene with camera, lighting, and GLTF model."""
    # Spawn camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.7, 0.7, 1.0).looking_at(Vec3(0.0, 0.3, 0.0), Vec3.Y),
    )

    # Spawn directional light with shadows
    commands.spawn(DirectionalLight(shadows_enabled=True))

    # Load and spawn the GLTF scene
    # Note: Using FlightHelmet model from assets/models/FlightHelmet/
    scene_handle = server.load("bevy/models/FlightHelmet/FlightHelmet.gltf#Scene0", Scene)
    commands.spawn(SceneRoot(scene_handle))


if __name__ == "__main__":
    main().run()
