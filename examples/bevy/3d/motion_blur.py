"""Per-object motion blur, configured per camera with the MotionBlur component.

Controls:
- 1/2: decrease/increase the shutter angle (blur amount)
- 3/4: decrease/increase the sample count (blur quality)
- Space: switch between the chase and trackside cameras
"""

import math

from pybevy.assets import RenderAssetUsages
from pybevy.image import (
    ImageAddressMode,
    ImageFilterMode,
    ImageSampler,
    ImageSamplerDescriptor,
)
from pybevy.post_process import MotionBlur
from pybevy.prelude import *
from pybevy.render import Extent3d, TextureDimension, TextureFormat

N_CARS = 20
N_CONES = 100
N_TREES = 30
TEXTURE_SIZE = 7


@resource
class CameraMode(Resource):
    """Whether the camera chases the tracked car or watches it from the track side."""

    def __init__(self, chase: bool) -> None:
        self.chase = chase


@component
class Moves(Component):
    def __init__(self, offset: float) -> None:
        self.offset = offset


@component
class CameraTracked(Component):
    """Marks the car the camera follows."""


@component
class Rotates(Component):
    """Marks a spinning wheel."""


@component
class ShutterAngleText(Component):
    """Marks the span showing the shutter angle."""


@component
class SamplesText(Component):
    """Marks the span showing the sample count."""


@system
def setup_camera(commands: Commands) -> None:
    commands.spawn(
        Camera3d(),
        # MotionBlur requires the motion vector prepass, which Bevy inserts automatically.
        MotionBlur(shutter_angle=1.0, samples=2),
    )


def race_track_pos(offset: float, t: float) -> Vec2:
    """Point on a looping race track, offset perpendicular to the track."""
    x_tweak = 2.0
    y_tweak = 3.0
    scale = 8.0
    x0 = math.sin(x_tweak * t)
    y0 = math.cos(y_tweak * t)
    dx = x_tweak * math.cos(x_tweak * t)
    dy = y_tweak * -math.sin(y_tweak * t)
    dl = math.hypot(dx, dy)
    x = x0 + offset * dy / dl
    y = y0 - offset * dx / dl
    return Vec2(x * scale, y * scale)


def uv_debug_texture() -> Image:
    palette = [
        164, 164, 164, 255, 168, 168, 168, 255, 153, 153, 153, 255, 139, 139, 139, 255,
        153, 153, 153, 255, 177, 177, 177, 255, 159, 159, 159, 255,
    ]
    texture_data: list[int] = []
    for _ in range(TEXTURE_SIZE):
        texture_data.extend(palette)
        palette = palette[-12:] + palette[:-12]

    image = Image(
        Extent3d(width=TEXTURE_SIZE, height=TEXTURE_SIZE, depth_or_array_layers=1),
        TextureDimension.D2,
        texture_data,
        TextureFormat.Rgba8UnormSrgb,
        RenderAssetUsages.RENDER_WORLD,
    )
    linear = ImageSamplerDescriptor.linear()
    image.sampler = ImageSampler.Descriptor(
        ImageSamplerDescriptor(
            address_mode_u=ImageAddressMode.Repeat,
            address_mode_v=ImageAddressMode.MirrorRepeat,
            mag_filter=ImageFilterMode.Nearest,
            min_filter=linear.min_filter,
            mipmap_filter=linear.mipmap_filter,
        )
    )
    return image


@system
def setup_scene(
    commands: Commands,
    asset_server: Res[AssetServer],
    images: ResMut[Assets[Image]],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    commands.insert_resource(GlobalAmbientLight(color=Color.WHITE, brightness=300.0))
    commands.insert_resource(CameraMode(chase=True))
    commands.spawn(
        DirectionalLight(illuminance=3_000.0, shadow_maps_enabled=True),
        Transform().looking_to(Vec3(-1.0, -0.7, -1.0), Vec3.X),
    )
    # Sky
    commands.spawn(
        Mesh3d(meshes.add(Sphere())),
        MeshMaterial3d(
            materials.add(StandardMaterial(unlit=True, base_color=Color.linear_rgb(0.1, 0.6, 1.0)))
        ),
        Transform().with_scale(Vec3.splat(-4000.0)),
    )
    # Ground
    plane = Plane3d().mesh().build()
    uv_size = 4000.0
    plane.insert_attribute(
        Mesh.ATTRIBUTE_UV_0, [[uv_size, 0.0], [0.0, 0.0], [0.0, uv_size], [uv_size, uv_size]]
    )
    commands.spawn(
        Mesh3d(meshes.add(plane)),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.WHITE,
                    perceptual_roughness=1.0,
                    base_color_texture=images.add(uv_debug_texture()),
                )
            )
        ),
        Transform.from_xyz(0.0, -0.65, 0.0).with_scale(Vec3.splat(80.0)),
    )

    spawn_cars(commands, asset_server, meshes, materials)
    spawn_trees(commands, meshes, materials)
    spawn_barriers(commands, meshes, materials)


def spawn_cars(
    commands: Commands,
    asset_server: Res[AssetServer],
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    box_mesh = meshes.add(Cuboid(0.3, 0.15, 0.55))
    cylinder = meshes.add(Cylinder())
    logo = asset_server.load("bevy/branding/icon.png", asset_type=Image)
    wheel_matl = materials.add(StandardMaterial(base_color=Color.WHITE, base_color_texture=logo))

    colors = [
        materials.add(StandardMaterial(base_color=color))
        for color in [
            Color.linear_rgb(1.0, 0.0, 0.0),
            Color.linear_rgb(1.0, 1.0, 0.0),
            Color.BLACK,
            Color.linear_rgb(0.0, 0.0, 1.0),
            Color.linear_rgb(0.0, 1.0, 0.0),
            Color.linear_rgb(1.0, 0.0, 1.0),
            Color.linear_rgb(0.5, 0.5, 0.0),
            Color.linear_rgb(1.0, 0.5, 0.0),
        ]
    ]

    for i in range(N_CARS):
        color = colors[i % len(colors)]
        car = commands.spawn(
            Mesh3d(box_mesh),
            MeshMaterial3d(color),
            Transform.from_scale(Vec3.splat(0.5)),
            Moves(i * 2.0),
        )
        if i == 0:
            car.insert(CameraTracked())
        car_id = car.id()
        commands.spawn(
            Mesh3d(box_mesh),
            MeshMaterial3d(color),
            Transform.from_xyz(0.0, 0.08, 0.03).with_scale(Vec3(1.0, 1.0, 0.5)),
            ChildOf(car_id),
        )
        for x, z in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]:
            commands.spawn(
                Mesh3d(cylinder),
                MeshMaterial3d(wheel_matl),
                Transform.from_xyz(0.14 * x, -0.045, 0.15 * z)
                .with_scale(Vec3(0.15, 0.04, 0.15))
                .with_rotation(Quat.from_rotation_z(math.pi / 2.0)),
                Rotates(),
                ChildOf(car_id),
            )


def spawn_barriers(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    capsule = meshes.add(Capsule3d())
    matl = materials.add(StandardMaterial(base_color=Color.srgb_u8(255, 87, 51), reflectance=1.0))
    for offset in (0.04, -0.04):
        for i in range(N_CONES):
            pos = race_track_pos(offset, i / N_CONES * math.pi * 2.0)
            commands.spawn(
                Mesh3d(capsule),
                MeshMaterial3d(matl),
                Transform.from_xyz(pos.x, -0.65, pos.y).with_scale(Vec3.splat(0.07)),
            )


def spawn_trees(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    capsule = meshes.add(Capsule3d())
    sphere = meshes.add(Sphere())
    leaves = materials.add(StandardMaterial(base_color=Color.linear_rgb(0.0, 1.0, 0.0)))
    trunk = materials.add(StandardMaterial(base_color=Color.linear_rgb(0.4, 0.2, 0.2)))
    for offset in (0.07, -0.07):
        for i in range(N_TREES):
            pos = race_track_pos(offset, i / N_TREES * math.pi * 2.0)
            commands.spawn(
                Mesh3d(sphere),
                MeshMaterial3d(leaves),
                Transform.from_xyz(pos.x, -0.3, pos.y).with_scale(Vec3.splat(0.3)),
            )
            commands.spawn(
                Mesh3d(capsule),
                MeshMaterial3d(trunk),
                Transform.from_xyz(pos.x, -0.5, pos.y).with_scale(Vec3(0.05, 0.3, 0.05)),
            )


@system
def setup_ui(commands: Commands) -> None:
    root = commands.spawn(
        Text(),
        Node(position_type=PositionType.Absolute, top=px(12), left=px(12)),
    ).id()
    commands.spawn(TextSpan(), ShutterAngleText(), ChildOf(root))
    commands.spawn(TextSpan(), SamplesText(), ChildOf(root))
    commands.spawn(TextSpan("1/2: -/+ shutter angle (blur amount)\n"), ChildOf(root))
    commands.spawn(TextSpan("3/4: -/+ sample count (blur quality)\n"), ChildOf(root))
    commands.spawn(TextSpan("Spacebar: cycle camera\n"), ChildOf(root))


@system
def keyboard_inputs(
    motion_blur_row: Single[Mut[MotionBlur]],
    presses: Res[ButtonInput[KeyCode]],
    shutter_row: Single[Mut[TextSpan], With[ShutterAngleText]],
    samples_row: Single[Mut[TextSpan], tuple[With[SamplesText], Without[ShutterAngleText]]],
    camera: ResMut[CameraMode],
) -> None:
    motion_blur = motion_blur_row.into_inner()
    if presses.just_pressed(KeyCode.Digit1):
        motion_blur.shutter_angle -= 0.25
    elif presses.just_pressed(KeyCode.Digit2):
        motion_blur.shutter_angle += 0.25
    elif presses.just_pressed(KeyCode.Digit3):
        motion_blur.samples = max(motion_blur.samples - 1, 0)
    elif presses.just_pressed(KeyCode.Digit4):
        motion_blur.samples += 1
    elif presses.just_pressed(KeyCode.Space):
        camera.chase = not camera.chase
    motion_blur.shutter_angle = min(max(motion_blur.shutter_angle, 0.0), 1.0)
    motion_blur.samples = min(motion_blur.samples, 64)
    shutter_row.into_inner().text = f"Shutter angle: {motion_blur.shutter_angle:.2f}\n"
    samples_row.into_inner().text = f"Samples: {motion_blur.samples}\n"


@system
def move_cars(
    time: Res[Time],
    movables: Query[tuple[Mut[Transform], Moves, Children]],
    spins: Query[Mut[Transform], tuple[Without[Moves], With[Rotates]]],
) -> None:
    for transform, moves, children in movables:
        t = time.elapsed_secs() * 0.25 + 0.5 * moves.offset
        dx = math.cos(t)
        dz = -math.sin(3.0 * t)
        t += math.sqrt(dx * dx + dz * dz) * 0.15
        prev = Vec3(transform.translation.x, transform.translation.y, transform.translation.z)
        pos = race_track_pos(0.0, t)
        transform.translation = Vec3(pos.x, -0.59, pos.y)
        delta = transform.translation - prev
        if delta.length_squared() > 0.0:
            transform.look_to(delta, Vec3.Y)
        for child in children:
            wheel = spins.get(child)
            if wheel is None:
                continue
            circumference = 2.0 * math.pi * wheel.scale.x
            wheel.rotate_local_y(delta.length() / circumference * math.pi * 2.0)


@system
def move_camera(
    camera: Single[tuple[Mut[Transform], Mut[Projection]], Without[CameraTracked]],
    tracked_row: Single[Transform, With[CameraTracked]],
    mode: Res[CameraMode],
) -> None:
    transform, projection = camera.into_inner()
    tracked = tracked_row.into_inner()
    if mode.chase:
        transform.translation = tracked.translation + Vec3(0.0, 0.15, 0.0) + tracked.back() * 0.6
        transform.look_to(tracked.forward(), Vec3.Y)
        fov = 1.0
    else:
        transform.look_at(tracked.translation, Vec3.Y)
        transform.translation = Vec3(15.0, -0.5, 0.0)
        fov = 0.05
    match projection:
        case Projection.Perspective(perspective):
            perspective.fov = fov
        case _:
            pass


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup_camera, setup_scene, setup_ui))
        .add_systems(Update, chain(keyboard_inputs, move_cars, move_camera))
    )


if __name__ == "__main__":
    main().run()
