"""Depth of field: the blur a real camera lens gives out-of-focus objects.

Controls:
- Up/Down: move the focal distance
- Left/Right: change the aperture f-number
- Space: cycle between bokeh, Gaussian, and off
"""

import math

from pybevy.camera import PhysicalCameraParameters
from pybevy.core_pipeline import Tonemapping
from pybevy.gltf import GltfMeshName
from pybevy.pbr import Lightmap
from pybevy.post_process import Bloom, DepthOfField, DepthOfFieldMode
from pybevy.prelude import *

SCENE_PATH = "bevy/models/DepthOfFieldExample/DepthOfFieldExample.glb"
LIGHTMAP_PATH = "bevy/models/DepthOfFieldExample/CircuitBoardLightmap.hdr"

# Adjustment speeds, per frame.
FOCAL_DISTANCE_SPEED = 0.05
APERTURE_F_STOP_SPEED = 0.01

MIN_FOCAL_DISTANCE = 0.01
MIN_APERTURE_F_STOPS = 0.05


@resource
class AppSettings(Resource):
    """User-adjustable depth of field settings; mode None turns the effect off."""

    def __init__(self) -> None:
        # Objects 7 meters away are in full focus.
        self.focal_distance = 7.0
        # A very low f-number, so the effect is easy to see.
        self.aperture_f_stops = 1.0 / 8.0
        self.mode: DepthOfFieldMode | None = DepthOfFieldMode.Bokeh

    def depth_of_field(self) -> DepthOfField | None:
        if self.mode is None:
            return None
        return DepthOfField(
            mode=self.mode,
            focal_distance=self.focal_distance,
            aperture_f_stops=self.aperture_f_stops,
            max_depth=14.0,
        )

    def help_text(self) -> str:
        if self.mode is None:
            return "Mode: Off (Press Space to change)"
        # Sensor height and FOV stay at their defaults.
        sensor_height = PhysicalCameraParameters().sensor_height
        fov = PerspectiveProjection().fov
        focal_length = 0.5 * sensor_height / math.tan(0.5 * fov)
        mode = "Bokeh" if self.mode == DepthOfFieldMode.Bokeh else "Gaussian"
        return (
            f"Focal distance: {self.focal_distance:.2f} m (Press Up/Down to change)\n"
            f"Aperture F-stops: f/{self.aperture_f_stops:.3f} (Press Left/Right to change)\n"
            f"Sensor height: {sensor_height * 1000.0:.2f}mm\n"
            f"Focal length: {focal_length * 1000.0:.2f}mm\n"
            f"Mode: {mode} (Press Space to change)"
        )


@system
def setup(commands: Commands, asset_server: Res[AssetServer], app_settings: Res[AppSettings]) -> None:
    # HDR bloom highlights the depth of field effect.
    camera = commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 4.5, 8.25).looking_at(Vec3.ZERO, Vec3.Y),
        Tonemapping.TonyMcMapface,
        Bloom.NATURAL,
    )
    depth_of_field = app_settings.depth_of_field()
    if depth_of_field is not None:
        camera.insert(depth_of_field)

    commands.spawn(
        WorldAssetRoot(
            asset_server.load(GltfAssetLabel.Scene(0).from_asset(SCENE_PATH), asset_type=WorldAsset)
        )
    )

    commands.spawn(
        Text(app_settings.help_text()),
        Node(position_type=PositionType.Absolute, bottom=px(12), left=px(12)),
    )


@system
def adjust_focus(input: Res[ButtonInput[KeyCode]], app_settings: ResMut[AppSettings]) -> None:
    distance_delta = 0.0
    if input.pressed(KeyCode.ArrowDown):
        distance_delta = -FOCAL_DISTANCE_SPEED
    elif input.pressed(KeyCode.ArrowUp):
        distance_delta = FOCAL_DISTANCE_SPEED

    f_stop_delta = 0.0
    if input.pressed(KeyCode.ArrowLeft):
        f_stop_delta = -APERTURE_F_STOP_SPEED
    elif input.pressed(KeyCode.ArrowRight):
        f_stop_delta = APERTURE_F_STOP_SPEED

    app_settings.focal_distance = max(app_settings.focal_distance + distance_delta, MIN_FOCAL_DISTANCE)
    app_settings.aperture_f_stops = max(app_settings.aperture_f_stops + f_stop_delta, MIN_APERTURE_F_STOPS)


@system
def change_mode(input: Res[ButtonInput[KeyCode]], app_settings: ResMut[AppSettings]) -> None:
    if not input.just_pressed(KeyCode.Space):
        return
    if app_settings.mode is None:
        app_settings.mode = DepthOfFieldMode.Bokeh
    elif app_settings.mode == DepthOfFieldMode.Bokeh:
        app_settings.mode = DepthOfFieldMode.Gaussian
    else:
        app_settings.mode = None


@system
def update_dof_settings(
    commands: Commands,
    view_targets: Query[Entity, With[Camera]],
    app_settings: Res[AppSettings],
) -> None:
    depth_of_field = app_settings.depth_of_field()
    for view in view_targets:
        if depth_of_field is None:
            commands.entity(view).remove(DepthOfField)
        else:
            commands.entity(view).insert(depth_of_field)


@system
def tweak_scene(
    commands: Commands,
    asset_server: Res[AssetServer],
    materials: ResMut[Assets[StandardMaterial]],
    lights: Query[Mut[DirectionalLight], Changed[DirectionalLight]],
    named_entities: Query[
        tuple[Entity, GltfMeshName, MeshMaterial3d],
        tuple[With[Mesh3d], Without[Lightmap]],
    ],
) -> None:
    """Makes one-time adjustments to the scene that can't be encoded in glTF."""
    for light in lights:
        light.shadow_maps_enabled = True

    # Add a lightmap to the circuit board.
    for entity, name, material in named_entities:
        if name.name == "CircuitBoard":
            circuit_board = materials.get_mut(material.handle)
            if circuit_board is not None:
                circuit_board.lightmap_exposure = 10000.0
            commands.entity(entity).insert(Lightmap(image=asset_server.load_image(LIGHTMAP_PATH)))


@system
def update_text(texts: Query[Mut[Text]], app_settings: Res[AppSettings]) -> None:
    for text in texts:
        text.text = app_settings.help_text()


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(AppSettings())
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, tweak_scene)
        .add_systems(Update, chain(adjust_focus, change_mode, update_dof_settings, update_text))
    )


if __name__ == "__main__":
    main().run()
