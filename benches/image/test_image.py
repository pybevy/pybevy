import numpy as np
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, MinimalPlugins, Startup, Update
from pybevy.assets import AssetPlugin, Assets
from pybevy.ecs import Commands, Query, ResMut
from pybevy.image import Image, ImagePlugin
from pybevy.sprite import Sprite
from pybevy.wgpu import Extent3d

# 4K RGBA image = 3840 * 2160 * 4 bytes = ~33MB
image_width = 3840
image_height = 2160
pixel_count = image_width * image_height * 4


def setup(images: ResMut[Assets[Image]], commands: Commands) -> None:
    data = np.random.randint(0, 255, size=pixel_count, dtype=np.uint8)
    image = Image(Extent3d(image_width, image_height, 1), data)
    commands.spawn(Sprite(images.add(image)))


def create_app() -> App:
    return (
        App()
        .add_plugins(MinimalPlugins, AssetPlugin, ImagePlugin)
        .add_systems(Startup, setup)
    )


def test_image_data_copy(benchmark: BenchmarkFixture) -> None:
    """Benchmark copying image data to owned NumPy array."""
    def update_system(assets: Assets[Image], query: Query[Sprite]) -> None:
        sprite = query.single()
        assert sprite is not None

        image = assets.get(sprite.image)
        assert image is not None

        pixels = image.data_copy()
        assert pixels.shape == (pixel_count,)

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_image_set_data(benchmark: BenchmarkFixture) -> None:
    """Benchmark copying data into image."""
    new_data = np.random.randint(0, 255, size=pixel_count, dtype=np.uint8)

    def update_system(assets: ResMut[Assets[Image]], query: Query[Sprite]) -> None:
        sprite = query.single()
        assert sprite is not None

        image = assets.get_mut(sprite.image)
        assert image is not None

        image.set_data(new_data)

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_image_data_readonly_context(benchmark: BenchmarkFixture) -> None:
    """Benchmark zero-copy readonly access via context manager."""
    def update_system(assets: Assets[Image], query: Query[Sprite]) -> None:
        sprite = query.single()
        assert sprite is not None

        image = assets.get(sprite.image)
        assert image is not None

        with image.data() as pixels:
            assert pixels.shape == (pixel_count,)

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_image_data_mutable_context(benchmark: BenchmarkFixture) -> None:
    """Benchmark zero-copy mutable access via context manager."""
    def update_system(assets: ResMut[Assets[Image]], query: Query[Sprite]) -> None:
        sprite = query.single()
        assert sprite is not None

        image = assets.get_mut(sprite.image)
        assert image is not None

        with image.data_mut() as pixels:
            assert pixels.shape == (pixel_count,)

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_image_data_readonly_with_computation(benchmark: BenchmarkFixture) -> None:
    """Benchmark readonly access with actual NumPy computation."""
    def update_system(assets: Assets[Image], query: Query[Sprite]) -> None:
        sprite = query.single()
        assert sprite is not None

        image = assets.get(sprite.image)
        assert image is not None

        with image.data() as pixels:
            # Simulate some computation
            mean = np.mean(pixels)
            assert mean >= 0

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_image_data_mutable_with_modification(benchmark: BenchmarkFixture) -> None:
    """Benchmark mutable access with actual pixel modification."""
    def update_system(assets: ResMut[Assets[Image]], query: Query[Sprite]) -> None:
        sprite = query.single()
        assert sprite is not None

        image = assets.get_mut(sprite.image)
        assert image is not None

        with image.data_mut() as pixels:
            # Simulate some modification (increase brightness)
            pixels[:] = np.clip(pixels + 10, 0, 255).astype(np.uint8)

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)
