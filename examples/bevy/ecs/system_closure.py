"""Shows how anonymous functions and closures can be used as systems.

In Python, system functions can capture variables from outer scopes.
"""

from pybevy.prelude import *


def simple_function() -> None:
    """A simple function that can be used as a system."""
    print("Hello from a simple function!")


def make_stateful_system(initial_value: str):
    """Create a closure that maintains state between calls."""
    state = {"value": initial_value}

    def system() -> None:
        print(f"Hello from a stateful closure! {state['value']}")
        # Modify the captured state - will persist between calls
        state["value"] = f"{state['value']} - updated"

    return system


@entrypoint
def main(app: App) -> App:
    outside_variable = "bar"

    # Create stateful system
    stateful_system = make_stateful_system("foo")

    # Inline lambda (limited to expressions only in Python)
    inline_lambda = lambda: print("Hello from an inline lambda!")

    # Closure that captures outside variable
    def closure_with_capture() -> None:
        print(
            f"Hello from a closure that captured 'outside_variable'! {outside_variable}"
        )

    return (
        app.add_systems(Update, simple_function)
        .add_systems(Update, stateful_system)
        .add_systems(Update, inline_lambda)
        .add_systems(Update, closure_with_capture)
    )


if __name__ == "__main__":
    main().run()
