"""An empty application with default plugins.

Demonstrates adding DefaultPlugins which provides:
- Window management
- Rendering
- Input handling
- Asset loading
- Time management

The app will open a window and run until closed.
"""

from pybevy.prelude import *


@entrypoint
def main(app: App) -> App:
    return app.add_plugins(DefaultPlugins)


if __name__ == "__main__":
    main().run()
