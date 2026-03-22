"""An empty application (does nothing).

The absolute minimal PyBevy application. Demonstrates the minimum code needed
to create and run a PyBevy app.
"""

from pybevy.prelude import *


@entrypoint
def main(app: App) -> App:
    return app


if __name__ == "__main__":
    main().run()
