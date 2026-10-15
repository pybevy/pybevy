"""App helpers shared by the benches."""

from pybevy.app import App


def initialize(app: App) -> None:
    app.finish()
    app.cleanup()
