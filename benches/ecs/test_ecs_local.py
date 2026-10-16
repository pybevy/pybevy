from pytest_benchmark.fixture import BenchmarkFixture

from benches._helpers import initialize
from pybevy.app import App, Update
from pybevy.ecs import system


def test_local_system(benchmark: BenchmarkFixture) -> None:
    counter = [0]

    @system
    def update_system() -> None:
        counter[0] += 1

    app = App().add_systems(Update, update_system)

    initialize(app)
    benchmark(app.update)


def test_local_update(benchmark: BenchmarkFixture) -> None:
    counter = [0]

    @system
    def update_system() -> None:
        counter[0] += 1

    app = App().add_systems(Update, update_system)

    initialize(app)
    benchmark(app.update)
