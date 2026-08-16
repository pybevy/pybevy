from collections.abc import Callable
from typing import Protocol, TypeVar, overload

from .app import App, Plugin
from .ecs import Component, Event, Message, Resource

CT = TypeVar("CT", bound=Component)
RT = TypeVar("RT", bound=Resource)
PL = TypeVar("PL", bound=Plugin)
MT = TypeVar("MT", bound=Message)
ET = TypeVar("ET", bound=Event)

def message(cls: type[MT]) -> type[MT]:
    """Validate and mark a custom Message subclass.

    The message channel must still be registered with ``app.add_message(cls)``.
    """

def event(cls: type[ET]) -> type[ET]:
    """Validate and mark a custom Event subclass used by observers."""

@overload
def component(cls: type[CT]) -> type[CT]: ...
@overload
def component(*, storage: str) -> Callable[[type[CT]], type[CT]]: ...
def component(  # type: ignore[misc]
    cls: type[CT] | None = None,
    *,
    storage: str | None = None,
) -> type[CT] | Callable[[type[CT]], type[CT]]:
    """Decorator to register a class as an ECS component.

    IMPORTANT: The class MUST also inherit from Component. Both the decorator
    AND the inheritance are required - using only one will cause a runtime error.

    Args:
        storage: Explicit storage mode. Use ``"python"`` to opt into PyObject
            storage. When omitted, wrapper storage is required; if the
            component has non-primitive fields a ``TypeError`` is raised.
            Read queries over Python storage use a shallow read proxy and
            declare exclusive scheduler access. Nested objects remain live and
            do not automatically mark the component changed.

    Example:
        ```python
        @component
        class Velocity(Component):
            x: float = 0.0
            y: float = 0.0

        @component(storage="python")
        @dataclass
        class Inventory(Component):
            items: list[str]
        ```

    Raises:
        TypeError: If the class does not inherit from Component.
        ValueError: If storage is not ``"python"`` or None.
    """

def resource(cls: type[RT]) -> type[RT]:
    """Decorator to register a class as a global ECS resource.

    IMPORTANT: The class MUST also inherit from Resource. Both the decorator
    AND the inheritance are required - using only one will cause a runtime error.

    Resources are singleton data accessible to all systems via Res[T] or ResMut[T].

    Example:
        ```python
        @resource
        class GameSettings(Resource):  # MUST inherit from Resource
            difficulty: int = 1
            music_volume: float = 0.8

        # With dataclass
        @resource
        @dataclass
        class Score(Resource):
            value: int = 0
        ```

    Raises:
        TypeError: If the class does not inherit from Resource.
    """

def plugin(cls: type[PL]) -> type[PL]:
    """Decorator to register a class as a Bevy plugin.

    IMPORTANT: The class MUST also inherit from Plugin. Both the decorator
    AND the inheritance are required.

    Plugins encapsulate reusable app configuration including systems, resources,
    and other plugins.

    Example:
        ```python
        @plugin
        class MyGamePlugin(Plugin):  # MUST inherit from Plugin
            def build(self, app: App) -> None:
                app.add_systems(Startup, setup)
                app.add_systems(Update, game_loop)
        ```

    Raises:
        TypeError: If the class does not inherit from Plugin.
    """
def clear_component_cache(verbose: bool = False) -> None:
    """Clear the component cache. Called before full reload to allow fresh component definitions."""

def enable_component_caching(enabled: bool, verbose: bool = False) -> None:
    """Enable or disable component caching for hot reload support."""

def is_component_decorated(cls: type) -> bool:
    """Check if a class has been properly decorated with @component.

    Returns:
        True if the class has the @component decorator, False otherwise.
    """

class EntrypointDecoratorResult(Protocol):
    """Protocol for functions decorated with @entrypoint."""
    __name__: str
    __doc__: str | None
    @overload
    def __call__(self) -> App: ...
    @overload
    def __call__(self, app: App) -> App: ...

from .material import material as material

def entrypoint(func: Callable[[App], App]) -> EntrypointDecoratorResult:
    """Decorator for the main app entry point function.

    Automatically injects an App instance when called with no arguments,
    while still allowing explicit App instances for testing.

    Unlike @component/@resource/@plugin, this decorator is applied to a FUNCTION,
    not a class. No inheritance is required.

    Example:
        ```python
        @entrypoint
        def main(app: App) -> App:
            return (
                app.add_plugins(DefaultPlugins)
                .add_systems(Startup, setup)
                .add_systems(Update, game_loop)
            )

        if __name__ == "__main__":
            main().run()  # App() is automatically created
        ```

    For testing:
        ```python
        test_app = App()
        test_app.add_plugins(ScheduleRunnerPlugin(RunMode.Once()))
        main(test_app).run()  # Pass explicit app instance
        ```
    """
