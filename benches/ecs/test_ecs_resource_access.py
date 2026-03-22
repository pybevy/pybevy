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

    def run_systems():
        app = create_app_with_wireframe_systems(100)
        app.update()

    benchmark(run_systems)


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

    def run_systems():
        app = create_app_with_volume_systems(100)
        app.update()

    benchmark(run_systems)


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

    def run_system():
        app = App()
        app.insert_resource(Time())
        app.insert_resource(WireframeConfig(global_=True))
        app.insert_resource(GlobalVolume(volume=Volume.Linear(0.5)))
        app.add_systems(Update, system)
        app.update()

    benchmark(run_system)

    # Verify system ran
    assert len(results) > 0


def test_resource_access_scaling(benchmark):
    """Test how resource access scales with number of systems."""
    import time as pytime

    def measure_app_with_n_systems(num_systems: int) -> float:
        """Measure average time per update for app with N systems."""
        app = App()
        app.insert_resource(Time())

        for i in range(num_systems):
            def make_system(index):
                def system(time: Res[Time]) -> None:
                    _ = time.delta_secs()
                return system
            app.add_systems(Update, make_system(i))

        # Warmup
        app.update()

        # Measure 50 iterations
        start = pytime.perf_counter()
        for _ in range(50):
            app.update()
        end = pytime.perf_counter()

        return (end - start) / 50

    benchmark(lambda: measure_app_with_n_systems(10))

    system_counts = [10, 50, 100]
    timings = []

    for count in system_counts:
        avg_time = measure_app_with_n_systems(count)
        timings.append((count, avg_time))

    ratio_10_to_100 = timings[2][1] / timings[0][1]
    assert ratio_10_to_100 < 15.0, f"Resource access scales poorly: {ratio_10_to_100:.2f}x (expected <15x)"
