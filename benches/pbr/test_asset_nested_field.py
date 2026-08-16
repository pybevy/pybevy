"""Benchmarks for scalar and nested access through borrowed material assets."""

from collections.abc import Callable

from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, TaskPoolPlugin
from pybevy.assets import AssetPlugin, Assets
from pybevy.color import Color
from pybevy.core_pipeline import CorePipelinePlugin
from pybevy.ecs import World
from pybevy.gltf import GltfPlugin
from pybevy.image import ImagePlugin
from pybevy.mesh import MeshPlugin
from pybevy.pbr import PbrPlugin, StandardMaterial
from pybevy.render import RenderPlugin


def _with_material(callback: Callable[[StandardMaterial], None]) -> None:
    def access(world: World) -> None:
        materials = world.resource(Assets[StandardMaterial])
        handle = materials.add(StandardMaterial())
        material = materials.get_mut(handle)
        assert material is not None
        callback(material)

    App().add_plugins(
        TaskPoolPlugin,
        AssetPlugin,
        ImagePlugin,
        MeshPlugin,
        RenderPlugin,
        CorePipelinePlugin,
        GltfPlugin,
        PbrPlugin,
    ).world(access)


def test_asset_scalar_read(benchmark: BenchmarkFixture) -> None:
    def run(material: StandardMaterial) -> None:
        benchmark(lambda: material.perceptual_roughness)

    _with_material(run)


def test_asset_nested_read(benchmark: BenchmarkFixture) -> None:
    def run(material: StandardMaterial) -> None:
        transform = material.uv_transform
        benchmark(lambda: transform.translation.x)

    _with_material(run)


def test_asset_nested_write(benchmark: BenchmarkFixture) -> None:
    value = 1.0

    def run(material: StandardMaterial) -> None:
        transform = material.uv_transform

        def write() -> None:
            transform.translation.x = value

        benchmark(write)

    _with_material(run)


def test_asset_color_read(benchmark: BenchmarkFixture) -> None:
    def run(material: StandardMaterial) -> None:
        color = material.base_color
        benchmark(color.alpha)

    _with_material(run)


def test_asset_color_write(benchmark: BenchmarkFixture) -> None:
    def run(material: StandardMaterial) -> None:
        color = material.base_color
        benchmark(color.set_alpha, 0.5)

    _with_material(run)


def test_asset_color_channel_write(benchmark: BenchmarkFixture) -> None:
    def run(material: StandardMaterial) -> None:
        color = material.base_color
        assert isinstance(color, Color.LinearRgba)
        channels = color.value

        def write() -> None:
            channels.red = 0.5

        benchmark(write)

    _with_material(run)
