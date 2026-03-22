"""Demonstrates a startup system (one that runs once when the app starts up).

Demonstrates:
- Startup schedule vs Update schedule
- System execution order
- Basic system definition

Startup systems run exactly once when the app starts up, right before normal systems run.
"""

from pybevy.prelude import *


def startup_system() -> None:
    """Startup system - runs once at app start."""
    print("startup system ran first")


def normal_system() -> None:
    """Normal system - runs every frame."""
    print("normal system ran second")


@entrypoint
def main(app: App) -> App:
    return (
        app.add_systems(Startup, startup_system)
        .add_systems(Update, normal_system)
    )


if __name__ == "__main__":
    main().run()
