"""Performance benchmarks for resource access with borrowed references.

Compares cloned vs borrowed resource access patterns to measure the impact
of zero-copy optimization.
"""

from pybevy.app import App, Update
from pybevy.audio import GlobalVolume, Volume
from pybevy.color import Color
from pybevy.ecs import Res
from pybevy.pbr import WireframeConfig
from pybevy.time import Time


def test_time_access_many_systems(benchmark):
    """Benchmark Time resource access across many systems (borrowed refs)."""

    def create_app_with_time_systems(num_systems: int) -> App:
        app = App()
        app.insert_resource(Time())

        # Create many systems that all access Time
        for i in range(num_systems):
            def make_system(index):
                def system(time: Res[Time]) -> None:
                    # Access multiple Time methods (realistic usage)
                    _ = time.delta_secs()
                    _ = time.elapsed_secs()
                    _ = time.elapsed()
                return system

            app.add_systems(Update, make_system(i))

        return app

    def run_systems():
        app = create_app_with_time_systems(100)
        app.update()

    benchmark(run_systems)


def test_wireframe_config_access_many_systems(benchmark):
    """Benchmark WireframeConfig resource access across many systems."""

    def create_app_with_wireframe_systems(num_systems: int) -> App:
        app = App()
        app.insert_resource(WireframeConfig(global_=True, default_color=Color.WHITE))

        for i in range(num_systems):
            def make_system(index):
                def system(config: Res[WireframeConfig]) -> None:
                    _ = config.global_
                    _ = config.default_color
                return system

            app.add_systems(Update, make_system(i))

        return app

    app = create_app_with_wireframe_systems(100)
    benchmark(app.update)


def test_global_volume_access_many_systems(benchmark):
    """Benchmark GlobalVolume resource access across many systems."""

    def create_app_with_volume_systems(num_systems: int) -> App:
        app = App()
        app.insert_resource(GlobalVolume(volume=Volume.Linear(0.8)))

        for i in range(num_systems):
            def make_system(index):
                def system(volume: Res[GlobalVolume]) -> None:
                    _ = volume.volume.to_linear()
                return system

            app.add_systems(Update, make_system(i))

        return app

    app = create_app_with_volume_systems(100)
    benchmark(app.update)


def test_multiple_resources_single_system(benchmark):
    """Benchmark accessing multiple resources in a single system."""

    results = []

    def system(
        time: Res[Time],
        wireframe: Res[WireframeConfig],
        volume: Res[GlobalVolume],
    ) -> None:
        # Access all resources
        dt = time.delta_secs()
        global_wireframe = wireframe.global_
        vol = volume.volume.to_linear()
        results.append((dt, global_wireframe, vol))

    app = App()
    app.insert_resource(Time())
    app.insert_resource(WireframeConfig(global_=True))
    app.insert_resource(GlobalVolume(volume=Volume.Linear(0.5)))
    app.add_systems(Update, system)
    app.update()

    benchmark(app.update)

    # Verify system ran
    assert len(results) > 0

