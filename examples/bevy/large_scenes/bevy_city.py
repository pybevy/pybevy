"""A procedurally generated city (port of bevy's `examples/large_scenes/bevy_city`).

A large-scene stress test: a grid of city blocks (roads, buildings, trees,
fences, moving cars) generated from noise-driven density, built from the CC0
Kenney city kits. Exercises bevy 0.19's big-scene rendering from Python.

On first run the Kenney assets (~50 small files) are downloaded from
https://github.com/bevyengine/bevy_asset_files into assets/kenney/ (the Rust
original streams the same files at runtime via bevy's web asset source).

Run from the repository root:
    poetry run python examples/bevy/large_scenes/bevy_city.py [--seed 42] [--size 30]

Controls:
    WASD        move camera
    Q / E       down / up
    arrow keys  look around
    K           toggle wireframe

Differences from the Rust original: no Feathers settings panel, contact
shadows, TAA, or per-car mesh merging (those APIs are not wrapped yet), and
no loading screen (assets stream in as they load).
"""

import argparse
import math
import random
import urllib.request
from dataclasses import dataclass
from pathlib import Path

from pybevy.camera import Exposure
from pybevy.decorators import component
from pybevy.ecs import Component
from pybevy.input import ButtonInput, KeyCode
from pybevy.light import (
    Atmosphere,
    AtmosphereEnvironmentMapLight,
    DirectionalLight,
    Falloff,
    PhaseFunction,
    ScatteringMedium,
    ScatteringTerm,
)
from pybevy.pbr import (
    AtmosphereSettings,
    StandardMaterial,
    WireframeConfig,
    WireframePlugin,
)
from pybevy.prelude import *

BASE_URL = "https://github.com/bevyengine/bevy_asset_files/raw/main/kenney"

CARS = [
    "hatchback-sports", "suv", "suv-luxury", "sedan", "sedan-sports",
    "truck", "truck-flat", "van", "delivery", "delivery-flat",
    "taxi", "garbage-truck", "ambulance", "police", "firetruck",
]

ASSET_FILES = (
    [f"car-kit/{c}.glb" for c in CARS]
    # the .glb files reference their kit's colormap textures externally
    + ["car-kit/Textures/colormap.png", "city-kit-roads/Textures/colormap.png"]
    + ["city-kit-roads/road-crossroad-path.glb", "city-kit-roads/road-straight.glb",
       "city-kit-roads/tile-low.glb"]
    + [f"city-kit-commercial/Textures/{v}.png" for v in ("colormap", "variation-a", "variation-b")]
    + [f"city-kit-commercial/building-skyscraper-{t}.glb" for t in "abcde"]
    + [f"city-kit-commercial/building-{t}.glb" for t in ("m", "l", "a", "b", "c", "d", "f", "g", "h")]
    + [f"city-kit-suburban/Textures/{v}.png"
       for v in ("colormap", "variation-a", "variation-b", "variation-c")]
    + [f"city-kit-suburban/building-type-{t}.glb" for t in "bcdefghiklou"]
    + ["city-kit-suburban/tree-small.glb", "city-kit-suburban/tree-large.glb",
       "city-kit-suburban/path-stones-long.glb", "city-kit-suburban/fence.glb"]
)


def ensure_assets() -> None:
    """Download any missing Kenney assets into assets/kenney/ (CC0 licensed)."""
    root = Path("assets/kenney")
    missing = [f for f in ASSET_FILES if not (root / f).exists()]
    if not missing:
        return
    print(f"Downloading {len(missing)} Kenney asset files to {root}/ (first run only)...")
    for i, rel in enumerate(missing, 1):
        dest = root / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        urllib.request.urlretrieve(f"{BASE_URL}/{rel}", dest)
        print(f"  [{i}/{len(missing)}] {rel}")


def value_noise_2d(x: float, z: float, seed: int) -> float:
    """Deterministic smooth value noise in [0, 1] (stands in for OpenSimplex)."""

    def lattice(ix: int, iz: int) -> float:
        h = (ix * 374761393 + iz * 668265263 + seed * 1442695041) & 0xFFFFFFFF
        h = (h ^ (h >> 13)) * 1274126177 & 0xFFFFFFFF
        return ((h ^ (h >> 16)) & 0xFFFF) / 0xFFFF

    ix, iz = math.floor(x), math.floor(z)
    fx, fz = x - ix, z - iz
    sx, sz = fx * fx * (3 - 2 * fx), fz * fz * (3 - 2 * fz)  # smoothstep
    top = lattice(ix, iz) + sx * (lattice(ix + 1, iz) - lattice(ix, iz))
    bot = lattice(ix, iz + 1) + sx * (lattice(ix + 1, iz + 1) - lattice(ix, iz + 1))
    return top + sz * (bot - top)


@component(storage="python")
@dataclass
class CarState(Component):
    """Naive traffic state: the car shuttles along a world-space road segment."""

    start_x: float = 0.0
    start_z: float = 0.0
    end_x: float = 0.0
    end_z: float = 0.0
    dir: float = 1.0
    distance: float = 0.0


@component(storage="python")
@dataclass
class FlyCam(Component):
    yaw: float = 0.0
    pitch: float = 0.0


class CityAssets:
    """Handles for everything the generator spawns (loaded once at startup)."""

    def __init__(self, server: AssetServer, materials: Assets) -> None:
        def scene(path: str) -> Handle:
            return server.load(f"kenney/{path}#Scene0", WorldAsset)

        def mesh(path: str) -> Handle:
            return server.load(f"kenney/{path}#Mesh0/Primitive0", Mesh)

        def kit_materials(kit: str, variations: list[str]) -> list[Handle]:
            return [
                materials.add(StandardMaterial(
                    base_color_texture=server.load(f"kenney/{kit}/Textures/{v}.png", Image)))
                for v in variations
            ]

        self.cars = [scene(f"car-kit/{c}.glb") for c in CARS]
        self.crossroad = scene("city-kit-roads/road-crossroad-path.glb")
        self.road_straight = scene("city-kit-roads/road-straight.glb")

        commercial = kit_materials("city-kit-commercial", ["colormap", "variation-a", "variation-b"])
        self.high_density = (
            [mesh(f"city-kit-commercial/building-skyscraper-{t}.glb") for t in "abcde"]
            + [mesh(f"city-kit-commercial/building-{t}.glb") for t in ("m", "l")],
            commercial,
        )
        self.medium_density = (
            [mesh(f"city-kit-commercial/building-{t}.glb") for t in ("a", "b", "c", "d", "f", "g", "h")],
            commercial,
        )
        self.low_density = (
            [mesh(f"city-kit-suburban/building-type-{t}.glb") for t in "bcdefghiklou"],
            kit_materials("city-kit-suburban",
                          ["colormap", "variation-a", "variation-b", "variation-c"]),
        )

        self.ground_mesh = mesh("city-kit-roads/tile-low.glb")
        self.ground_road_mat = server.load(
            "kenney/city-kit-roads/tile-low.glb#DefaultMaterial/std", StandardMaterial)
        self.ground_grass_mat = materials.add(
            StandardMaterial(base_color=Color.srgb(97 / 255, 203 / 255, 139 / 255)))

        self.tree_small = scene("city-kit-suburban/tree-small.glb")
        self.tree_large = scene("city-kit-suburban/tree-large.glb")
        self.path_stones_long = scene("city-kit-suburban/path-stones-long.glb")
        self.fence = scene("city-kit-suburban/fence.glb")

    def random_building(self, rng: random.Random, kind: tuple) -> tuple:
        meshes, mats = kind
        return Mesh3d(rng.choice(meshes)), MeshMaterial3d(rng.choice(mats))


ARGS = argparse.Namespace(seed=42, size=30)
FRAC_PI_2 = math.pi / 2.0


@entrypoint
def main(app: App) -> App:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--size", type=int, default=30, help="city grid size (size x size blocks)")
    parser.parse_args(namespace=ARGS)

    ensure_assets()

    window = Window()
    window.title = "bevy_city"
    return (
        app.add_plugins(DefaultPlugins().set(WindowPlugin(primary_window=window)))
        .add_plugins(WireframePlugin())
        .insert_resource(ClearColor(Color.BLACK))
        .insert_resource(WireframeConfig(global_=False, default_color=Color.WHITE))
        .add_systems(Startup, setup)
        .add_systems(Update, simulate_cars)
        .add_systems(Update, fly_camera)
        .add_systems(Update, toggle_wireframe)
    )


def setup(
    commands: Commands,
    server: Res[AssetServer],
    materials: ResMut[Assets[StandardMaterial]],
    mediums: ResMut[Assets[ScatteringMedium]],
) -> None:
    assets = CityAssets(server, materials)

    commands.spawn(
        Camera3d(),
        Transform.from_xyz(15.0, 10.0, 20.0).looking_at(Vec3.ZERO, Vec3.Y),
        FlyCam(yaw=math.atan2(15.0, 20.0), pitch=-0.4),
        # Fit the aerial-view LUT to the city (~16 km) for less banding
        AtmosphereSettings(aerial_view_lut_max_distance=1.6e4),
        Exposure.OVERCAST,  # the raw-sunlight sun is bright
        Bloom.NATURAL,  # gives the sun a natural glow
        AtmosphereEnvironmentMapLight(),  # atmosphere drives ambient light
        Msaa.Off,
    )

    # Sun at raw sunlight illuminance (bevy's light_consts::lux::RAW_SUNLIGHT)
    commands.spawn(
        DirectionalLight(shadow_maps_enabled=True, illuminance=130_000.0),
        Transform.from_xyz(1.0, 0.15, 1.0).looking_at(Vec3.ZERO, Vec3.Y),
    )

    spawn_atmosphere(commands, mediums)
    spawn_city(commands, assets, ARGS.seed, ARGS.size)


def spawn_atmosphere(commands: Commands, mediums: Assets) -> None:
    """Earth atmosphere plus a low-lying haze term (see the Rust original)."""
    medium = ScatteringMedium()  # defaults to earth's terms

    haze_scale_height_km = 0.1  # 100 m fog layer
    atmosphere_ref_height_km = 60.0
    albedo = 0.99  # high albedo, low absorption: white fog
    visibility_km = 12.0
    beta_ext = (3.912 / visibility_km) * 1e-3  # Koschmieder extinction, m^-1

    medium.terms = [
        *medium.terms,
        ScatteringTerm(
            absorption=Vec3.splat(beta_ext * (1.0 - albedo)),
            scattering=Vec3.splat(beta_ext * albedo),
            falloff=Falloff.exponential(haze_scale_height_km / atmosphere_ref_height_km),
            phase=PhaseFunction.Mie(0.76),
        ),
    ]
    atmosphere = Atmosphere.earth(mediums.add(medium))

    # 1 city block ~ 100 m relative to the atmosphere
    scale = 1.0 / 20.0
    commands.spawn(
        atmosphere,
        Transform.from_translation(Vec3(0.0, -atmosphere.inner_radius * scale, 0.0)).with_scale(
            Vec3.splat(scale)
        ),
    )


def spawn_city(commands: Commands, assets: CityAssets, seed: int, size: int) -> None:
    """A grid of city blocks: crossroad + roads per block, density picks the fill.

    Each block is 5.5 x 4.0 units; density comes from smooth noise so
    forests, suburbs, and skyscraper clusters form contiguous areas.
    """
    rng = random.Random(seed)
    noise_seed = rng.getrandbits(31)
    noise_scale = 0.025

    half = size // 2
    for gx in range(-half, half):
        for gz in range(-half, half):
            off_x, off_z = gx * 5.5, gz * 4.0
            spawn_roads_and_cars(commands, assets, rng, off_x, off_z)

            density = value_noise_2d(off_x * noise_scale, off_z * noise_scale, noise_seed)

            grass = density < 0.6  # forest and suburban blocks get a grass tile
            tile_scale = Vec3(4.5, 1.0, 3.0)
            commands.spawn(
                Mesh3d(assets.ground_mesh),
                MeshMaterial3d(assets.ground_grass_mat if grass else assets.ground_road_mat),
                Transform.from_translation(
                    Vec3(0.5 + 4.5 / 2 + off_x, -0.5005 + 0.5, 0.5 + 3.0 / 2 + off_z)
                ).with_scale(tile_scale),
            )

            if density < 0.45:
                spawn_forest(commands, assets, rng, off_x, off_z)
            elif density < 0.6:
                spawn_low_density(commands, assets, rng, off_x, off_z)
            elif density < 0.7:
                spawn_medium_density(commands, assets, rng, off_x, off_z)
            else:
                spawn_high_density(commands, assets, rng, off_x, off_z)


def spawn_roads_and_cars(
    commands: Commands, assets: CityAssets, rng: random.Random, x: float, z: float
) -> None:
    commands.spawn(WorldAssetRoot(assets.crossroad), Transform.from_xyz(x, 0.0, z))

    max_car_density = 0.4

    # Horizontal road: one stretched segment instead of many tiles
    commands.spawn(
        WorldAssetRoot(assets.road_straight),
        Transform.from_xyz(x + 2.75, 0.0, z).with_scale(Vec3(4.5, 1.0, 1.0)),
    )
    # Cars drive along the road in both directions (world-space segment in CarState)
    for i in range(9):
        for lane, direction in ((-0.15, -1.0), (0.15, 1.0)):
            if rng.random() < max_car_density:
                spawn_car(
                    commands, assets, rng,
                    start=(x + 0.75, z + lane), end=(x + 0.75 + 0.5 * 9, z + lane),
                    direction=direction, progress=i * 0.5,
                    rotation=Quat.from_axis_angle(Vec3.Y, FRAC_PI_2 * (3.0 if direction < 0 else 1.0)),
                )

    # Vertical road
    commands.spawn(
        WorldAssetRoot(assets.road_straight),
        Transform.from_xyz(x, 0.0, z + 2.0)
        .with_scale(Vec3(3.0, 1.0, 1.0))
        .with_rotation(Quat.from_axis_angle(Vec3.Y, FRAC_PI_2)),
    )
    for i in range(6):
        for lane, direction in ((0.15, 1.0), (-0.15, -1.0)):
            if rng.random() < max_car_density:
                spawn_car(
                    commands, assets, rng,
                    start=(x + lane, z + 0.75), end=(x + lane, z + 0.75 + 0.5 * 6),
                    direction=direction, progress=i * 0.5,
                    rotation=Quat.from_axis_angle(Vec3.Y, math.pi if direction < 0 else 0.0),
                )


def spawn_car(
    commands: Commands,
    assets: CityAssets,
    rng: random.Random,
    start: tuple[float, float],
    end: tuple[float, float],
    direction: float,
    progress: float,
    rotation: Quat,
) -> None:
    commands.spawn(
        WorldAssetRoot(rng.choice(assets.cars)),
        Transform.from_xyz(start[0], 0.0, start[1]).with_scale(Vec3.splat(0.15)).with_rotation(rotation),
        CarState(
            start_x=start[0], start_z=start[1], end_x=end[0], end_z=end[1],
            dir=direction, distance=progress,
        ),
    )


def spawn_low_density(
    commands: Commands, assets: CityAssets, rng: random.Random, x: float, z: float
) -> None:
    for i in (1, 2):
        bx = x + i * 1.8
        commands.spawn(
            *assets.random_building(rng, assets.low_density),
            Transform.from_xyz(bx, 0.0, z + 1.25),
        )
        commands.spawn(
            *assets.random_building(rng, assets.low_density),
            Transform.from_xyz(bx, 0.0, z + 2.75).with_rotation(
                Quat.from_axis_angle(Vec3.Y, math.pi)
            ),
        )
    for i in range(7):
        commands.spawn(
            WorldAssetRoot(assets.fence),
            Transform.from_xyz(x + 2.75, 0.0, z + 0.75 + i * 0.4).with_rotation(
                Quat.from_axis_angle(Vec3.Y, FRAC_PI_2)
            ),
        )
    for i in range(9):
        for tx in (0.75, 4.75):
            commands.spawn(
                WorldAssetRoot(assets.tree_small),
                Transform.from_xyz(x + tx, 0.0, z + 0.75 + i * 0.3),
            )


def spawn_medium_density(
    commands: Commands, assets: CityAssets, rng: random.Random, x: float, z: float
) -> None:
    for i in range(1, 6):
        bx = x + i * 0.9
        commands.spawn(
            *assets.random_building(rng, assets.medium_density),
            Transform.from_xyz(bx, 0.0, z + 1.0),
        )
        for tx in (0.0, 0.5):
            if i == 5 and tx == 0.5:
                break
            for tz in (1.75, 2.25):
                commands.spawn(
                    WorldAssetRoot(assets.tree_large),
                    Transform.from_xyz(bx + tx, 0.0, z + tz),
                )
        commands.spawn(
            *assets.random_building(rng, assets.medium_density),
            Transform.from_xyz(bx, 0.0, z + 3.0).with_rotation(
                Quat.from_axis_angle(Vec3.Y, math.pi)
            ),
        )
    for i in range(11):
        px = x + 0.75 + i * 0.4
        commands.spawn(
            WorldAssetRoot(assets.path_stones_long),
            Transform.from_xyz(px, 0.02, z + 2.0)
            .with_scale(Vec3(1.0, 2.0, 1.0))
            .with_rotation(Quat.from_axis_angle(Vec3.Y, FRAC_PI_2)),
        )
        for fz in (1.85, 2.15):
            commands.spawn(WorldAssetRoot(assets.fence), Transform.from_xyz(px, 0.02, z + fz))


def spawn_high_density(
    commands: Commands, assets: CityAssets, rng: random.Random, x: float, z: float
) -> None:
    for i in range(3):
        bx = x + 1.25 + i * 1.5
        commands.spawn(
            *assets.random_building(rng, assets.high_density),
            Transform.from_xyz(bx, 0.0, z + 1.25),
        )
        commands.spawn(
            *assets.random_building(rng, assets.high_density),
            Transform.from_xyz(bx, 0.0, z + 2.75).with_rotation(
                Quat.from_axis_angle(Vec3.Y, math.pi)
            ),
        )


def spawn_forest(
    commands: Commands, assets: CityAssets, rng: random.Random, x: float, z: float
) -> None:
    for i in range(13):
        for j in range(9):
            kind = rng.randrange(3)
            if kind == 0:
                continue
            tree = assets.tree_small if kind == 1 else assets.tree_large
            commands.spawn(
                WorldAssetRoot(tree),
                Transform.from_xyz(x + 0.75 + i * 0.325, 0.0, z + 0.85 + j * 0.3),
            )


def simulate_cars(query: Query[tuple[Mut[Transform], Mut[CarState]]], time: Res[Time]) -> None:
    """Move each car along its road segment, wrapping at the end."""
    speed = 1.5
    delta = time.delta_secs()
    for transform, car in query:
        dx, dz = car.end_x - car.start_x, car.end_z - car.start_z
        road_len = math.hypot(dx, dz)
        car.distance += speed * delta
        if car.distance > road_len:
            car.distance = 0.0
        t = car.distance / road_len * car.dir
        transform.translation = Vec3(car.start_x + dx * t, 0.0, car.start_z + dz * t)


def fly_camera(
    query: Query[tuple[Mut[Transform], Mut[FlyCam]], With[Camera3d]],
    keys: Res[ButtonInput],
    time: Res[Time],
) -> None:
    move_speed = 8.0 * time.delta_secs()
    look_speed = 1.5 * time.delta_secs()

    for transform, cam in query:
        if keys.pressed(KeyCode.ArrowLeft):
            cam.yaw += look_speed
        if keys.pressed(KeyCode.ArrowRight):
            cam.yaw -= look_speed
        if keys.pressed(KeyCode.ArrowUp):
            cam.pitch = min(cam.pitch + look_speed, 1.5)
        if keys.pressed(KeyCode.ArrowDown):
            cam.pitch = max(cam.pitch - look_speed, -1.5)

        rotation = Quat.from_axis_angle(Vec3.Y, cam.yaw) * Quat.from_axis_angle(
            Vec3.X, cam.pitch
        )
        transform.rotation = rotation

        forward = rotation * Vec3(0.0, 0.0, -1.0)
        right = rotation * Vec3(1.0, 0.0, 0.0)
        motion = Vec3.ZERO
        if keys.pressed(KeyCode.KeyW):
            motion = motion + forward
        if keys.pressed(KeyCode.KeyS):
            motion = motion - forward
        if keys.pressed(KeyCode.KeyA):
            motion = motion - right
        if keys.pressed(KeyCode.KeyD):
            motion = motion + right
        if keys.pressed(KeyCode.KeyE):
            motion = motion + Vec3.Y
        if keys.pressed(KeyCode.KeyQ):
            motion = motion - Vec3.Y
        transform.translation = transform.translation + motion * move_speed


def toggle_wireframe(keys: Res[ButtonInput], config: ResMut[WireframeConfig]) -> None:
    if keys.just_pressed(KeyCode.KeyK):
        config.global_ = not config.global_


if __name__ == "__main__":
    main().run()
