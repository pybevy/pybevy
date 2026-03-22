from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, Update


def test_local_system(benchmark: BenchmarkFixture) -> None:
    counter = [0]

    def update_system() -> None:
        counter[0] += 1

    app = App().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_local_update(benchmark: BenchmarkFixture) -> None:
    counter = [0]

    def update_system() -> None:
        counter[0] += 1

    app = App().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)
