from collections.abc import Callable, Iterable, Iterator
from typing import (
    Any,
    Generic,
    Protocol,
    TypeVar,
    TypeVarTuple,
    Unpack,
    overload,
    runtime_checkable,
)

import numpy as np

from pybevy.app import Stage, SystemFn
from pybevy.expr import Expr
from pybevy.light import PointLight
from pybevy.transform import Transform

@runtime_checkable
class Batchable(Protocol):
    """Protocol for batch component data returned by from_numpy() methods.

    Returned by built-in components (Transform, Visibility, etc.) and
    @component-decorated classes' from_numpy() (wrapper-storage only).
    Users should not implement this protocol directly.
    """

    def count(self) -> int: ...

class Message: ...

class F32List:
    """Borrowed list wrapper for Vec<f32> fields in components.

    Provides list-like access to Vec<f32> fields. Mutations persist back
    to the underlying component in ECS.

    Example:
        ```python
        def modify_bounds(query: Query[Mut[CascadeShadowConfig]]) -> None:
            for config in query:
                config.bounds[0] = 5.0  # Persists to ECS!
                config.bounds.append(100.0)
        ```
    """

    def __init__(self, values: list[float] = ...) -> None: ...
    def __len__(self) -> int: ...
    def __getitem__(self, index: int) -> float: ...
    def __setitem__(self, index: int, value: float) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def to_list(self) -> list[float]:
        """Convert to Python list."""
    def append(self, value: float) -> None:
        """Append a value to the end."""
    def pop(self, index: int = -1) -> float:
        """Remove and return item at index (default: last item)."""
    def insert(self, index: int, value: float) -> None:
        """Insert value at index."""
    def clear(self) -> None:
        """Clear all items."""
    def extend(self, values: list[float]) -> None:
        """Extend with items from list."""

class Resource:
    """Base class for ECS resources (global singleton data).

    Resources are global data accessible to all systems. Unlike components,
    which are attached to entities, resources exist once per type.

    IMPORTANT: Custom resources MUST use BOTH the @resource decorator AND
    inherit from Resource. Using only one will cause a runtime error.

    Example:
        ```python
        from pybevy.decorators import resource

        @resource  # Required decorator
        class GameState(Resource):  # MUST inherit from Resource
            score: int = 0
            level: int = 1

        # Access in systems via Res[T] (read-only) or ResMut[T] (mutable)
        def update_score(state: ResMut[GameState]) -> None:
            state.score += 10
        ```
    """

class Event:
    """Base class for ECS events.

    Events are used for communication between systems via observers or event readers.
    Unlike components and resources, events do NOT need a decorator - just inherit
    from Event.

    Example:
        ```python
        from dataclasses import dataclass

        @dataclass
        class PlayerDied(Event):  # Just inherit, no decorator needed
            player_id: int
            cause: str

        # Trigger events via Commands or World
        def kill_player(commands: Commands) -> None:
            commands.trigger(PlayerDied(player_id=1, cause="lava"))

        # Handle events via observers
        def on_player_died(trigger: On[PlayerDied]) -> None:
            event = trigger.event()
            print(f"Player {event.player_id} died from {event.cause}")
        ```
    """


type Res[T: Resource] = T
"""Read-only access to a Bevy resource.

Res[T] provides immutable access to a resource, allowing multiple systems
to read the same resource in parallel without conflicts.

Example:
    ```python
    def system(time: Res[Time]):
        elapsed = time.elapsed_secs()
    ```

See Also:
    - ResMut[T]: For mutable resource access
"""

type ResMut[T: Resource] = T
"""Mutable access to a Bevy resource.

ResMut[T] provides exclusive mutable access to a resource. Only one system
can have ResMut access to a resource at a time, preventing data races.

Example:
    ```python
    def system(state: ResMut[GameState]):
        state.score += 10
    ```

See Also:
    - Res[T]: For read-only resource access
"""

E = TypeVar("E")
OnTypes = TypeVarTuple("OnTypes")

M = TypeVar("M", bound=Message)
ComponentTypeVar = TypeVar("ComponentTypeVar", bound=Component)
ResourceType = TypeVar("ResourceType", bound=Resource)
MessageTypeVar = TypeVar("MessageTypeVar", bound=Message)

class MessageWriter(Generic[M]):
    """System parameter for writing messages to the ECS.

    Messages use double-buffering: messages written in frame N
    can be read by MessageReader in frame N+1.

    Example:
        def my_system(writer: MessageWriter[AppExit]) -> None:
            writer.write(AppExit.SUCCESS)
    """
    def write(self, message: M) -> MessageId:
        """Write a message to the message buffer."""
    def write_batch(self, messages: list[M]) -> list[MessageId]:
        """Write multiple messages at once."""
    def write_default(self) -> MessageId:
        """Write a default instance of the message type."""

class MessageReader(Generic[M]):
    """System parameter for reading messages from the ECS.

    Messages use double-buffering: messages written in frame N
    can be read in frame N+1.

    Example:
        def my_system(reader: MessageReader[AppExit]) -> None:
            for msg in reader:
                print(f"Received: {msg}")
    """
    def clear(self) -> None:
        """Clear all messages in the buffer."""
    def is_empty(self) -> bool:
        """Check if there are any messages."""
    def len(self) -> int:
        """Get the number of messages in the buffer."""
    def read(self) -> Iterator[M]:
        """Get an iterator over messages."""
    def __iter__(self) -> Iterator[M]:
        """Iterate over messages."""

class MessageReaderIter(Iterator[Any]):
    """Iterator over messages from MessageReader.

    Internal implementation class returned by MessageReader.__iter__().
    Users typically don't need to reference this type directly.
    """
    def __iter__(self) -> MessageReaderIter: ...
    def __next__(self) -> Any: ...

class Messages(Resource, Generic[M]):
    """Internal resource for message storage.

    Users typically don't need to reference this type directly - use
    MessageWriter and MessageReader for the public API.
    """
    def send(self, message: Any) -> MessageId: ...
    def clear(self) -> None: ...
    def is_empty(self) -> bool: ...
    def len(self) -> int: ...

class MessageType:
    """Internal type identifier for messages.

    Users typically don't need to reference this type directly.
    """

class MessageTypeParam:
    """Internal type parameter for message system parameters.

    Users typically don't need to reference this type directly.
    """

class On(Generic[Unpack[OnTypes]]):
    """System parameter for observers that provides access to triggered events.

    Use with type parameters to specify event type and optional bundle filter:
    - On[EventType] - Observe any event of this type
    - On[EventType, ComponentType] - Only trigger if entity has single component
    - On[EventType, tuple[CompA, CompB, ...]] - Only trigger if entity has all components in tuple
    - On[Add, ComponentType] - Observe component addition lifecycle events
    - On[Insert, ComponentType] - Observe component insertion lifecycle events
    - On[Remove, ComponentType] - Observe component removal lifecycle events
    - On[Replace, ComponentType] - Observe component replacement lifecycle events
    - On[Despawn, ComponentType] - Observe entity despawn lifecycle events

    Example:
        def on_player_died(trigger: On[PlayerDied]) -> None:
            event = trigger.event()
            print(f"Player {event.player_id} died")

        def on_transform_added(trigger: On[Add, Transform]) -> None:
            entity = trigger.entity()
            print(f"Transform added to {entity}")

        def on_damage_with_bundle(trigger: On[DamageEvent, tuple[Transform, Health]]) -> None:
            # Only triggers for entities that have BOTH Transform AND Health components
            event = trigger.event()
            entity = trigger.entity()
    """
    @overload
    def event(self: On[E]) -> E:
        """Get the event data.

        Returns the event object with its type determined by the first type parameter to On.
        """
    @overload
    def event(self: On[E, ComponentTypeVar]) -> E:
        """Get the event data.

        Returns the event object with its type determined by the first type parameter to On.
        """
    @overload
    def event(self: On[E, tuple[ComponentTypeVar, ...]]) -> E:
        """Get the event data.

        Returns the event object with its type determined by the first type parameter to On.
        """
    def entity(self) -> Entity | None:
        """Get the entity this event targets (for entity-targeted events)."""

class OnTypeParam:
    """Internal type parameter for On[...] subscript expressions.

    Created by On.__class_getitem__ when using On[EventType] or On[EventType, BundleType].
    Users typically don't need to reference this type directly.
    """

class Add:
    """Lifecycle event marker for component addition.

    Use with On[Add, ComponentType] to observe when components are added to entities
    via spawn() or the first insert() on an entity.
    """

class Insert:
    """Lifecycle event marker for component insertion.

    Use with On[Insert, ComponentType] to observe when components are inserted
    via insert(), whether the entity already has the component or not.
    """

class Remove:
    """Lifecycle event marker for component removal.

    Use with On[Remove, ComponentType] to observe when components are removed
    from entities via remove().
    """

class Replace:
    """Lifecycle event marker for component replacement.

    Use with On[Replace, ComponentType] to observe when components are replaced
    (inserted when entity already has the component).
    """

class Despawn:
    """Lifecycle event marker for entity despawn.

    Use with On[Despawn, ComponentType] to observe when entities with the
    component are despawned.
    """

class ConditionalSystem:
    """Wrapper for a system with a run condition.

    Created by run_if(). Supports combinators for complex conditional logic:
    - .and_(condition): Both conditions must be true
    - .or_(condition): Either condition must be true
    - .not_(): Inverts the condition

    Example:
        ```python
        def should_run() -> bool:
            return True

        def other_condition() -> bool:
            return False

        # Run system only if both conditions are true
        app.add_systems(Update, run_if(my_system, should_run).and_(other_condition))

        # Run system if either condition is true
        app.add_systems(Update, run_if(my_system, should_run).or_(other_condition))

        # Run system when condition is false
        app.add_systems(Update, run_if(my_system, should_run).not_())
        ```
    """
    def __init__(self, system: Any, condition: Callable[..., bool]) -> None: ...

    def and_(self, condition: Callable[..., bool]) -> ConditionalSystem:
        """Combine with another condition using AND logic.

        Args:
            condition: Another condition function that returns bool

        Returns:
            New ConditionalSystem that runs only if both conditions are true
        """

    def or_(self, condition: Callable[..., bool]) -> ConditionalSystem:
        """Combine with another condition using OR logic.

        Args:
            condition: Another condition function that returns bool

        Returns:
            New ConditionalSystem that runs if either condition is true
        """

    def not_(self) -> ConditionalSystem:
        """Negate this condition using NOT logic.

        Returns:
            New ConditionalSystem that runs when the condition is false
        """

def run_if(system: SystemFn, condition: Callable[..., bool]) -> ConditionalSystem:
    """Create a conditional system that only runs when condition returns true.

    Args:
        system: The system function to run conditionally
        condition: A function that returns bool (can have system parameters)

    Returns:
        ConditionalSystem that can be added to schedules or chained with combinators

    Example:
        ```python
        def should_run() -> bool:
            return True

        def my_system() -> None:
            print("Running!")

        # System only runs when should_run() returns True
        app.add_systems(Update, run_if(my_system, should_run))

        # Chain conditions
        app.add_systems(Update, run_if(my_system, cond1).and_(cond2).or_(cond3))
        ```
    """

class Commands:
    @overload
    def spawn(self, *components: Component) -> EntityCommands: ...
    @overload
    def spawn(self, components: tuple[Component, ...]) -> EntityCommands: ...
    @overload
    def spawn_batch(
        self, *components: Component | Batchable, count: int | None = None
    ) -> list[Entity]:
        """Spawn entities from batch/uniform components (numpy fast path)."""
    @overload
    def spawn_batch(self, iterable: Iterable[tuple[Component, ...]], /) -> None:
        """Spawn entities from an iterable of component tuples (legacy path)."""
    def spawn_empty(self) -> EntityCommands: ...
    def entity(self, entity: Entity) -> EntityCommands: ...
    def get_entity(self, entity: Entity) -> EntityCommands | None: ...
    def insert_resource(self, resource: Resource) -> None: ...
    def remove_resource(self, resource_type: type[ResourceType]) -> None:
        """Remove a resource from the world.

        Args:
            resource_type: The resource type (class) to remove, not an instance.

        Example:
            commands.remove_resource(Time)
        """
    def despawn(self, entity: Entity) -> None: ...
    def trigger(self, event: Event) -> None:
        """Trigger an event (deferred until command flush).

        Events are queued and triggered during the command flush phase.
        Observers will run at that time.

        Example:
            commands.trigger(PlayerDied(player_id=1, cause="lava"))
        """

class Component:
    """Base class for ECS components.

    Components are data attached to entities. Each entity can have multiple
    components of different types.

    IMPORTANT: Custom components MUST use BOTH the @component decorator AND
    inherit from Component. Using only one will cause a runtime error.

    Example:
        ```python
        from pybevy.decorators import component

        @component  # Required decorator
        class Velocity(Component):  # MUST inherit from Component
            x: float = 0.0
            y: float = 0.0

        # With dataclass for additional features
        @component
        @dataclass
        class Health(Component):
            current: int
            max: int = 100

        # Marker component (no fields)
        @component
        class Player(Component):
            pass

        # Query components in systems
        def move_entities(query: Query[tuple[Mut[Transform], Velocity]]) -> None:
            for transform, velocity in query:
                transform.translation.x += velocity.x
        ```
    """

    @staticmethod
    def from_numpy(**kwargs: object) -> Batchable:
        """Create a batch of components from numpy arrays for spawn_batch().

        Added by the @component decorator for wrapper-storage components
        (not available for storage="python" components).
        """

class CustomComponent(Component):
    """Internal wrapper for custom Python components.

    This is the internal type used when a user-defined component (decorated
    with @component) is queried from the ECS. Users typically don't need to
    reference this type directly - use the user's class type instead.
    """

class LazyWrapperProxy:
    """Internal proxy for lazy component field access.

    Used internally by the wrapper-based custom component storage system.
    Users typically don't need to reference this type directly.
    """
    @property  # type: ignore[misc]
    def __class__(self) -> type: ...

# View column proxy types for IDE autocomplete
# These represent column accessors returned by view.column_mut(ComponentType)
# They provide field-level access for batch operations (no methods)

class ViewColumn:
    """Opaque column view for zero-copy access via Numba JIT and JAX interop.

    ViewColumn is an opaque handle that provides zero-copy access to Bevy ECS
    component data. It CANNOT be converted to numpy arrays. Access data through
    @numba.jit functions (zero-copy) or JAX (copy-based, supports GPU).

    Safety: The validity token is checked at the Numba call boundary. If the
    system that created this view has finished execution, accessing it will
    raise a RuntimeError instead of causing a segfault.

    Examples:
        import numba

        @numba.jit(nopython=True)
        def kernel(view: ViewColumn):
            for i in range(len(view)):
                view[i] = view[i] + 1.0

        def system(view: View[Mut[Transform]]):
            for batch in view.batch_iter():
                y = batch.col(Transform).translation.y
                kernel(y)  # Safety check at call boundary

    Do NOT:
        - Try to convert to numpy: np.asarray(view)  # RuntimeError!
        - Cache in global variables (will become stale)
        - Access directly in Python: view[0]  # TypeError!

    Debugging:
        - view.peek(index) -> float: Read single value (with safety check)
        - view.to_list() -> list: Convert to Python list (with copy)
        - view.is_valid -> bool: Check if view is still valid
    """

    @property
    def is_valid(self) -> bool:
        """Check if this view is still valid (system hasn't ended)."""

    @property
    def ptr(self) -> int:
        """Get raw pointer (for Numba unbox only). Checks validity."""

    @property
    def len(self) -> int:
        """Get number of elements."""

    @property
    def stride(self) -> int:
        """Get stride in bytes."""

    @property
    def dtype(self) -> str:
        """Get NumPy dtype string (e.g., 'f4' for float32)."""

    def at_offset(self, offset: int, dtype: str) -> ViewColumn:
        """Create a sub-column view at a byte offset (for field peeling)."""

    def peek(self, index: int) -> float:
        """Read a single value (with safety check). NOT zero-copy!"""

    def to_list(self) -> list[float]:
        """Convert to Python list (with copy). For debugging only!"""

    @property
    def __array__(self) -> None:
        """Explicitly refuse numpy conversion. Raises RuntimeError."""

    @property
    def __array_interface__(self) -> None:
        """Block array interface access. Raises RuntimeError."""

    def __len__(self) -> int:
        """Get number of elements (for Numba JIT)."""

    def __getitem__(self, index: int) -> float:
        """Get element by index (only works inside Numba JIT)."""

    def __setitem__(self, index: int, value: float) -> None:
        """Set element by index (only works inside Numba JIT)."""

    def __getattr__(self, name: str) -> Any:
        """Dynamic field access for custom component fields."""

    def __setattr__(self, name: str, value: Any) -> None:
        """Dynamic field assignment for custom component fields."""

    # Arithmetic operators (eager element-wise on batch ViewColumns)
    def __mul__(self, other: ViewColumn | float) -> ViewColumn: ...
    def __rmul__(self, other: ViewColumn | float) -> ViewColumn: ...
    def __add__(self, other: ViewColumn | float) -> ViewColumn: ...
    def __radd__(self, other: ViewColumn | float) -> ViewColumn: ...
    def __sub__(self, other: ViewColumn | float) -> ViewColumn: ...
    def __rsub__(self, other: float) -> ViewColumn: ...
    def __truediv__(self, other: ViewColumn | float) -> ViewColumn: ...
    def __rtruediv__(self, other: float) -> ViewColumn: ...
    def __pow__(self, other: ViewColumn | float, modulo: object = ...) -> ViewColumn: ...
    def __rpow__(self, other: float, modulo: object = ...) -> ViewColumn: ...
    def __mod__(self, other: ViewColumn | float) -> ViewColumn: ...
    def __rmod__(self, other: float) -> ViewColumn: ...
    def __neg__(self) -> ViewColumn: ...
    def __abs__(self) -> ViewColumn: ...

    # Math methods (eager element-wise)
    def sin(self) -> ViewColumn: ...
    def cos(self) -> ViewColumn: ...
    def tan(self) -> ViewColumn: ...
    def asin(self) -> ViewColumn: ...
    def acos(self) -> ViewColumn: ...
    def atan(self) -> ViewColumn: ...
    def sqrt(self) -> ViewColumn: ...
    def abs(self) -> ViewColumn: ...
    def floor(self) -> ViewColumn: ...
    def ceil(self) -> ViewColumn: ...
    def round(self) -> ViewColumn: ...
    def exp(self) -> ViewColumn: ...
    def ln(self) -> ViewColumn: ...
    def log10(self) -> ViewColumn: ...
    def log2(self) -> ViewColumn: ...
    def sign(self) -> ViewColumn: ...
    def fract(self) -> ViewColumn: ...

    def min(self, other: ViewColumn | float) -> ViewColumn: ...
    def max(self, other: ViewColumn | float) -> ViewColumn: ...
    def clamp(self, min_val: float, max_val: float) -> ViewColumn: ...
    def lerp(self, other: ViewColumn | float, t: float) -> ViewColumn: ...

    def set(self, value: ViewColumn | float) -> None:
        """Assign values from another ViewColumn or a scalar into this column."""

    def to_contiguous_bytes(self) -> bytes:
        """Copy column data into contiguous bytes in native dtype (f4/f8/i4/i8).

        The output is tightly packed (no stride gaps), suitable for
        numpy.frombuffer() or JAX array construction.
        """

    def write_from_buffer(self, data: bytes) -> None:
        """Bulk write from bytes into ECS storage (stride-aware).

        Input must be tightly packed data in the column's native dtype.
        Raises RuntimeError on size mismatch or stale ViewColumn.
        """

    def to_jax(self) -> Any:
        """Convert to a JAX array (copy). Requires `import pybevy.ecs.jax_ext`."""

    def from_jax(self, arr: Any) -> None:
        """Write JAX array back into ECS storage. Requires `import pybevy.ecs.jax_ext`."""

class FieldExpr(ViewColumn, Expr):
    """Represents a scalar field in a View expression (e.g., intensity, range).

    Inherits from ViewColumn (batch path: to_jax, from_jax, to_list, etc.)
    and Expr (expression path: arithmetic, comparisons, where, etc.).
    """
    # Binary operators
    def __add__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __radd__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __sub__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __rsub__(self, other: Expr | float | int) -> FieldExpr: ...
    def __mul__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __rmul__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __truediv__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __rtruediv__(self, other: Expr | float | int) -> FieldExpr: ...
    def __pow__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __rpow__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __mod__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __rmod__(self, other: Expr | float | int) -> FieldExpr: ...

    # Unary operators
    def __neg__(self) -> FieldExpr: ...
    def __abs__(self) -> FieldExpr: ...

    # In-place operators (trigger immediate assignment)
    def __iadd__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __isub__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __imul__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __itruediv__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __ipow__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]

    # Comparison operators
    def __eq__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __ne__(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def __lt__(self, other: Expr | float | int) -> FieldExpr: ...
    def __le__(self, other: Expr | float | int) -> FieldExpr: ...
    def __gt__(self, other: Expr | float | int) -> FieldExpr: ...
    def __ge__(self, other: Expr | float | int) -> FieldExpr: ...

    # Logical operators
    def __and__(self, other: Expr | float | int) -> FieldExpr: ...
    def __or__(self, other: Expr | float | int) -> FieldExpr: ...
    def __invert__(self) -> FieldExpr: ...

    # Basic math functions
    def sqrt(self) -> FieldExpr: ...
    def abs(self) -> FieldExpr: ...
    def min(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def max(self, other: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]
    def clamp(self, min_val: Expr | float | int, max_val: Expr | float | int) -> FieldExpr: ...

    # Trigonometric functions
    def sin(self) -> FieldExpr: ...
    def cos(self) -> FieldExpr: ...
    def tan(self) -> FieldExpr: ...
    def asin(self) -> FieldExpr: ...
    def acos(self) -> FieldExpr: ...
    def atan(self) -> FieldExpr: ...

    # Rounding functions
    def floor(self) -> FieldExpr: ...
    def ceil(self) -> FieldExpr: ...
    def round(self) -> FieldExpr: ...

    # Exponential and logarithmic functions
    def exp(self) -> FieldExpr: ...
    def ln(self) -> FieldExpr: ...
    def log10(self) -> FieldExpr: ...
    def log2(self) -> FieldExpr: ...

    # Additional math operations
    def sign(self) -> FieldExpr: ...
    def fract(self) -> FieldExpr: ...
    def mod(self, other: Expr | float | int) -> FieldExpr: ...
    def lerp(self, other: Expr | float | int, t: Expr | float | int) -> FieldExpr: ...  # type: ignore[override]

    # Random functions
    def random(self) -> FieldExpr: ...
    def random_range(self, min: Expr | float | int, max: Expr | float | int) -> FieldExpr: ...

    # Conditional selection
    def where(self, true_value: Expr | float | int, false_value: Expr | float | int) -> FieldExpr: ...

    # Assignment
    def set(self, value: Expr | float | int) -> None: ...  # type: ignore[override]

    # NumPy conversion (requires batch context)
    def to_numpy(self) -> np.ndarray:
        """Convert this field to a NumPy array.

        Only valid in batch iteration context (within `for batch in view.iter_batches()`).
        Returns a zero-copy view of the underlying archetype storage for this field.

        Returns:
            NumPy array of shape (N,) for scalar fields

        Raises:
            RuntimeError: If called outside batch iteration context

        Example:
            ```python
            for batch in view.iter_batches():
                state = batch.column_mut(MoleState)
                # Convert to NumPy for Numba processing
                state_np = state.value.to_numpy()  # (N,) int32 array

                @numba.jit(nopython=True, cache=True)
                def process(s):
                    for i in range(len(s)):
                        s[i] += 1

                process(state_np)
            ```

        Note:
            For repr(transparent) components, this provides zero-copy access
            to the underlying contiguous array storage.
        """

    # Indexing operations (for Numba JIT)
    def __len__(self) -> int: ...
    def __getitem__(self, index: int) -> float: ...
    def __setitem__(self, index: int, value: float) -> None: ...
    def peek(self, index: int) -> float: ...

class Vec3Expr:
    """Represents a Vec3 field in a View expression (e.g., translation, scale).

    Provides x, y, z component access with assignment support.
    """
    @property
    def x(self) -> FieldExpr: ...
    @x.setter
    def x(self, value: Expr | FieldExpr | float | int) -> None: ...
    @property
    def y(self) -> FieldExpr: ...
    @y.setter
    def y(self, value: Expr | FieldExpr | float | int) -> None: ...
    @property
    def z(self) -> FieldExpr: ...
    @z.setter
    def z(self, value: Expr | FieldExpr | float | int) -> None: ...

    # Binary operators (Vec3 operations)
    def __add__(self, other: Vec3Expr) -> Vec3Expr: ...
    def __radd__(self, other: Vec3Expr) -> Vec3Expr: ...
    def __sub__(self, other: Vec3Expr) -> Vec3Expr: ...
    def __rsub__(self, other: Vec3Expr) -> Vec3Expr: ...

    # Scalar operations
    def __mul__(self, other: Expr | float | int) -> Vec3Expr: ...
    def __rmul__(self, other: Expr | float | int) -> Vec3Expr: ...
    def __truediv__(self, other: Expr | float | int) -> Vec3Expr: ...
    def __rtruediv__(self, other: Expr | float | int) -> Vec3Expr: ...

    # Unary operators
    def __neg__(self) -> Vec3Expr: ...

    # Assignment method
    def set(self, value: Vec3Expr) -> None: ...

    # NumPy conversion (requires batch context)
    def to_numpy(self) -> np.ndarray:
        """Convert this Vec3 field to NumPy array.

        Only valid in batch iteration context. Returns a zero-copy view
        of the underlying archetype storage for this Vec3 field.

        Returns:
            NumPy array of shape (N, 3) for Vec3 fields

        Raises:
            RuntimeError: If called outside batch iteration context

        Example:
            ```python
            for batch in view.iter_batches():
                transform = batch.column_mut(Transform)
                # Convert translation to NumPy
                trans_np = transform.translation.to_numpy()  # (N, 3) array

                @numba.jit(nopython=True, cache=True)
                def wave(trans, t):
                    for i in range(len(trans)):
                        trans[i, 1] = np.sin(trans[i, 0] * 0.5 + t * 3.0)

                wave(trans_np, time.elapsed_secs())
            ```
        """

    @overload
    def from_jax(self, obj: Any) -> None:
        """Write back from object with .x, .y, .z attributes. Requires `import pybevy.ecs.jax_ext`."""
    @overload
    def from_jax(self, x: Any, y: Any, z: Any) -> None:
        """Write back from 3 separate JAX arrays. Requires `import pybevy.ecs.jax_ext`."""
    def from_jax(self, x_or_obj: Any, y: Any = ..., z: Any = ...) -> None: ...  # type: ignore[misc]

class QuatExpr:
    """Represents a Quat field in a View expression (e.g., rotation).

    Provides x, y, z, w component access with assignment support.
    """
    @property
    def x(self) -> FieldExpr: ...
    @x.setter
    def x(self, value: Expr | FieldExpr | float | int) -> None: ...
    @property
    def y(self) -> FieldExpr: ...
    @y.setter
    def y(self, value: Expr | FieldExpr | float | int) -> None: ...
    @property
    def z(self) -> FieldExpr: ...
    @z.setter
    def z(self, value: Expr | FieldExpr | float | int) -> None: ...
    @property
    def w(self) -> FieldExpr: ...
    @w.setter
    def w(self, value: Expr | FieldExpr | float | int) -> None: ...

    # NumPy conversion (requires batch context)
    def to_numpy(self) -> np.ndarray:
        """Convert this Quat field to NumPy array.

        Only valid in batch iteration context. Returns a zero-copy view
        of the underlying archetype storage for this Quat field.

        Returns:
            NumPy array of shape (N, 4) for Quat fields (x, y, z, w)

        Raises:
            RuntimeError: If called outside batch iteration context

        Example:
            ```python
            for batch in view.iter_batches():
                transform = batch.column_mut(Transform)
                # Convert rotation to NumPy
                rot_np = transform.rotation.to_numpy()  # (N, 4) array

                @numba.jit(nopython=True, cache=True)
                def wiggle(rot, t):
                    for i in range(len(rot)):
                        angle = np.sin(t * 8.0 + float(i)) * 0.15
                        half = angle * 0.5
                        rot[i, 1] = np.sin(half)  # y
                        rot[i, 3] = np.cos(half)  # w

                wiggle(rot_np, time.elapsed_secs())
            ```
        """

    @overload
    def from_jax(self, obj: Any) -> None:
        """Write back from object with .x, .y, .z, .w attributes. Requires `import pybevy.ecs.jax_ext`."""
    @overload
    def from_jax(self, x: Any, y: Any, z: Any, w: Any) -> None:
        """Write back from 4 separate JAX arrays. Requires `import pybevy.ecs.jax_ext`."""
    def from_jax(self, x_or_obj: Any, y: Any = ..., z: Any = ..., w: Any = ...) -> None: ...  # type: ignore[misc]

class TransformViewColumn(ViewColumn):
    """View column accessor for Transform component.

    Provides field-level access for batch operations.
    For method access (look_at, rotate, etc.), use Query iteration instead.
    """

    translation: Vec3Expr
    rotation: QuatExpr
    scale: Vec3Expr

class PointLightViewColumn(ViewColumn):
    """View column accessor for PointLight component.

    Provides field-level access for batch operations on light properties.
    """

    intensity: FieldExpr
    range: FieldExpr
    radius: FieldExpr
    shadow_depth_bias: FieldExpr
    shadow_normal_bias: FieldExpr
    shadow_map_near_z: FieldExpr

# View-related internal classes
class ViewParam:
    """Type parameter for View system parameters.

    Internal implementation class created when View[...] is used in type hints.
    Users typically don't need to reference this type directly.
    """
    def _data_len(self) -> int: ...
    @property
    def _data_names(self) -> list[str]: ...
    @property
    def _filter_names(self) -> list[str]: ...

class ViewCol:
    """Read-only column proxy for View component fields.

    Internal implementation class returned by View.column(). Users typically
    access fields directly through the column, not this class itself.
    """
    component_id: int

class ViewColMut:
    """Mutable column proxy for View component fields.

    Internal implementation class returned by View.column_mut(). Users typically
    access fields directly through the column, not this class itself.
    """
    component_id: int

    def _trigger_assignment(self, field_name: str, value: Any) -> None:
        """Internal: Trigger field assignment with expression compilation."""

class BatchIterator(Iterator[Any]):
    """Iterator over batches (archetypes) in a View.

    Internal implementation class returned by View.__iter__(). Users typically
    don't need to reference this type directly.
    """
    def __iter__(self) -> BatchIterator: ...
    def __next__(self) -> Any: ...

class View(Generic[QueryParam_T, *Qs]):
    """
    High-performance batch operations on components.

    View compiles Python expressions to bytecode and executes them in parallel
    on all matching entities, achieving 30-50x speedup over Query iteration.

    IMPORTANT:
    - Use Mut[T] for mutable access, plain T for read-only (just like Query)
    - Filters must be explicit (With[T], Without[T], etc.), not bare component types

    Examples:
        # Modify all entities with Transform (requires Mut[])
        def system(view: View[Mut[Transform]]):
            transform = view.column_mut(Transform)
            transform.translation.x = transform.translation.x + 1.0

        # Read-only access (no Mut[])
        def system(view: View[Transform]):
            transform = view.column(Transform)  # read-only
            total = view.reduce_sum(transform.translation.x)

        # Filter to only entities with specific components (requires With[])
        def system(view: View[Mut[Transform], With[Cube]]):  # Only Transform+Cube entities
            transform = view.column_mut(Transform)
            transform.translation.y = 0.5

        # Read-only with filter
        def system(view: View[Transform, With[Marker]]):
            transform = view.column(Transform)  # read-only
            sum_x = view.reduce_sum(transform.translation.x)

        # Mixed access (mutable + read-only)
        def system(view: View[tuple[Mut[Transform], Velocity]]):
            transform = view.column_mut(Transform)
            velocity = view.column(Velocity)  # read-only
            transform.translation.x += velocity.x

        # Change detection: only affect entities whose Transform changed
        def system(view: View[Mut[Transform], Changed[Transform]]):
            transform = view.column_mut(Transform)
            transform.translation.y = transform.translation.y + 1.0

        # Added detection: only affect newly spawned entities
        def system(view: View[Mut[Transform], Added[Transform]]):
            transform = view.column_mut(Transform)
            transform.translation.y = 0.0
    """

    # Type-safe column accessors with overloads
    @overload
    def column_mut(self, component_type: type[Transform]) -> TransformViewColumn: ...  # type: ignore
    @overload
    def column_mut(self, component_type: type[PointLight]) -> PointLightViewColumn: ...  # type: ignore
    @overload
    def column_mut(self, component_type: type[ComponentTypeVar]) -> ViewColumn: ...  # type: ignore  # Generic fallback for unknown component types
    @overload
    def column(self, component_type: type[Transform]) -> TransformViewColumn: ...  # type: ignore
    @overload
    def column(self, component_type: type[PointLight]) -> PointLightViewColumn: ...  # type: ignore
    @overload
    def column(self, component_type: type[ComponentTypeVar]) -> ViewColumn: ...  # type: ignore  # Generic fallback for unknown component types
    def reduce_sum(self, expr: Expr) -> float:
        """
        Compute the sum of an expression across all entities.

        Args:
            expr: Expression to evaluate (e.g., view.column(Health).hp)

        Returns:
            Sum of all values

        Example:
            total_health = view.reduce_sum(view.column(Health).hp)
        """

    def reduce_mean(self, expr: Expr) -> float:
        """
        Compute the average of an expression across all entities.

        Args:
            expr: Expression to evaluate

        Returns:
            Average value

        Example:
            avg_health = view.reduce_mean(view.column(Health).hp)
        """

    def reduce_max(self, expr: Expr) -> float:
        """
        Find the maximum value of an expression across all entities.

        Args:
            expr: Expression to evaluate

        Returns:
            Maximum value

        Example:
            strongest = view.reduce_max(view.column(Health).hp)
        """

    def reduce_min(self, expr: Expr) -> float:
        """
        Find the minimum value of an expression across all entities.

        Args:
            expr: Expression to evaluate

        Returns:
            Minimum value

        Example:
            weakest = view.reduce_min(view.column(Health).hp)
        """

    def reduce_count(self, expr: Expr | None = None) -> int:
        """
        Count entities matching a condition (or all entities if no condition).

        Args:
            expr: Optional condition to filter entities

        Returns:
            Count of matching entities

        Example:
            total_players = view.reduce_count()
            critical = view.reduce_count(view.column(Health).hp < 20)
        """

    def iter_batches(self) -> Iterator[Batch]:
        """
        Iterate over archetype-sized batches (PyArrow-style chunked iteration).

        This provides a PyArrow-style chunked API where data is processed in
        archetype-sized batches. Each batch represents entities from a single
        archetype with contiguous component storage.

        Returns:
            Iterator of Batch objects, one per archetype

        Example:
            ```python
            import numba
            import numpy as np

            def wiggle_system(view: View[Mut[Transform], With[Marker]], time: Time) -> None:
                # Process each archetype batch separately
                for batch in view.iter_batches():
                    # Get numpy arrays (zero-copy views)
                    translations = batch.column_numpy(Transform, "translation")  # (N, 3)
                    rotations = batch.column_numpy_mut(Transform, "rotation")   # (N, 4)

                    # Define inline JIT kernel
                    @numba.jit(nopython=True, cache=True)
                    def process(trans, rot, t):
                        for i in range(len(trans)):
                            if trans[i, 1] > 0.5:  # Only visible entities
                                angle = np.sin(t * 8.0 + float(i)) * 0.15
                                half = angle * 0.5
                                rot[i, 1] = np.sin(half)
                                rot[i, 3] = np.cos(half)

                    # Execute on this batch
                    process(translations, rotations, time.elapsed_secs())
            ```

        Performance:
            - Each batch is processed in native code via Numba
            - Better cache locality (archetypes have similar components)
            - Can parallelize across batches in the future
            - Typical batch sizes: 100-10,000 entities per archetype

        Note:
            This API is designed to match PyArrow's batching pattern, familiar
            to data scientists. Each batch represents a contiguous chunk of
            component data from Bevy's Table storage.
        """

class Batch:
    """
    A batch of entities from a single archetype (PyArrow-style chunk).

    Represents a contiguous slice of component data from one archetype.
    Provides numpy array views for zero-copy access to ECS data.

    This is the ECS equivalent of PyArrow's RecordBatch - a chunk of
    columnar data that can be processed efficiently.
    """

    # ViewColumn accessors (unified API with .to_numpy() support)
    @overload
    def column(self, component_type: type[Transform]) -> TransformViewColumn: ...
    @overload
    def column(self, component_type: type[PointLight]) -> PointLightViewColumn: ...
    @overload
    def column(self, component_type: type[ComponentTypeVar]) -> ViewColumn: ...

    @overload
    def column_mut(self, component_type: type[Transform]) -> TransformViewColumn: ...
    @overload
    def column_mut(self, component_type: type[PointLight]) -> PointLightViewColumn: ...
    @overload
    def column_mut(self, component_type: type[ComponentTypeVar]) -> ViewColumn: ...

    def entities(self) -> list[Entity]:
        """Get entity IDs for this batch, in same order as column data.

        Returns a list of Entity objects corresponding to the entities in this
        batch. The order matches the column data indices, so ``entities()[i]``
        is the entity whose component data is at index ``i`` in any column.

        Example::

            for batch in view.iter_batches():
                entities = batch.entities()
                col = batch.column(Transform)
                # entities[i] corresponds to col data at index i
        """

    def __len__(self) -> int:
        """Get the number of entities in this batch (Python len() support)."""

class Entity:
    @staticmethod
    def from_raw(raw: int) -> Entity | None: ...
    def to_bits(self) -> int: ...
    @staticmethod
    def from_bits(bits: int) -> Entity: ...

class EntityCommands:
    def add_child(self, child: Entity) -> EntityCommands: ...
    def set_parent(self, parent: Entity) -> EntityCommands:
        """Set the parent of this entity.

        Creates a parent-child relationship by adding a ChildOf component to this entity.

        Args:
            parent: The entity to set as parent

        Returns:
            EntityCommands for method chaining

        Example:
            child = commands.spawn(Transform()).set_parent(parent_entity)
        """
    def remove_parent(self) -> EntityCommands:
        """Remove the parent relationship from this entity.

        Removes the ChildOf component, making this entity parentless.

        Returns:
            EntityCommands for method chaining

        Example:
            entity_commands.remove_parent()
        """
    def remove_children(self, *children: Entity) -> EntityCommands: ...
    def clear_children(self) -> EntityCommands: ...
    def id(self) -> Entity: ...
    def insert(self, *components: Component) -> EntityCommands: ...
    def remove(self, *components: type[Component]) -> EntityCommands: ...
    def despawn(self) -> None:
        """Despawn this entity.

        Removes the entity and all its components from the world.
        This is a deferred operation that will be applied when Commands are flushed.

        Example:
            entity_commands.despawn()
        """
    def with_children(
        self,
        func: Callable[[RelatedSpawnerCommands], Any],
    ) -> EntityCommands:
        """Spawn child entities in a hierarchical relationship.

        The callback receives a RelatedSpawnerCommands object for spawning children.
        Children automatically get a ChildOf component pointing to the parent.

        IMPORTANT: Python lambdas can only contain a single expression.
        For multiple children, return a tuple of spawn calls:

        Example - Single child:
            ```python
            commands.spawn(Transform()).with_children(lambda parent:
                parent.spawn(Mesh3d(mesh), MeshMaterial3d(material))
            )
            ```

        Example - Multiple children (tuple pattern):
            ```python
            commands.spawn(Transform()).with_children(lambda parent: (
                parent.spawn(Mesh3d(head_mesh), Transform.from_xyz(0, 1, 0)),
                parent.spawn(Mesh3d(body_mesh), Transform.from_xyz(0, 0, 0)),
                parent.spawn(Mesh3d(leg_mesh), Transform.from_xyz(0, -1, 0)),
            ))
            ```

        Example - Nested hierarchy:
            ```python
            root = commands.spawn(Transform()).id()
            commands.entity(root).with_children(lambda parent: (
                (mid := parent.spawn(Transform.from_xyz(1, 0, 0)).id()),
                parent.entity(mid).with_children(lambda p2:
                    p2.spawn(Mesh3d(leaf_mesh))
                ),
            ))
            ```

        Args:
            func: Callback that spawns children via RelatedSpawnerCommands

        Returns:
            Self for method chaining
        """
    def observe(self, observer: SystemFn) -> EntityCommands:
        """Register an observer for this specific entity.

        The observer will only trigger when events target this entity.

        Example:
            def on_damage(trigger: On[TakeDamage]) -> None:
                print(f"Entity {trigger.entity()} took damage")

            commands.spawn(Player()).observe(on_damage)
        """

class Name(Component):
    """Component for giving entities human-readable names.

    Names are not unique - multiple entities can have the same name.
    Use Entity for unique identification.

    Example:
        ```python
        # Spawn a named entity
        commands.spawn(Name("Player"), Transform())

        # Query named entities
        def system(query: Query[Name]) -> None:
            for name in query:
                print(f"Entity: {name}")
        ```
    """

    def __init__(self, name: str = "") -> None: ...
    @property
    def name(self) -> str:
        """Get the entity's name."""
    @name.setter
    def name(self, value: str) -> None:
        """Set the entity's name."""
    def as_str(self) -> str:
        """Get the name as a string (alias for name property)."""
    def __eq__(self, other: object) -> bool: ...

class ChildrenIterator(Iterator[Entity]):
    """Iterator over child entities.

    Returned by iterating over a Children component. Users typically don't
    need to reference this type directly.
    """
    def __iter__(self) -> ChildrenIterator: ...
    def __next__(self) -> Entity: ...

class Disabled(Component):
    """Marker component that disables an entity.

    Disabled entities are excluded from queries by default.
    """

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class ComponentId: ...

class World:
    def __init__(self) -> None: ...
    def spawn_empty(self) -> EntityCommands: ...
    def spawn_batch(self, batch: Iterable[Component | tuple[Component, ...]]) -> None: ...
    def spawn(self, *components: Component) -> EntityCommands: ...
    def commands(self) -> Commands: ...
    def resource(self, resource: type[ResourceType]) -> ResourceType: ...
    def register_resource(self, resource: type[ResourceType]) -> ComponentId: ...
    def init_resource(self, resource: type[ResourceType]) -> ComponentId: ...
    def insert_resource(self, resource: Resource) -> None: ...
    def component_id(self, component: type[Component]) -> ComponentId | None: ...
    def contains_resource(self, resource: type[ResourceType]) -> bool: ...
    def _get_last_error(self) -> tuple[str, str | None] | None:
        """Get the last system error, if any (PyBevy internal API).

        Returns a tuple of (error_message, traceback) or None if no error.
        """
    def despawn(self, entity: Entity) -> None: ...
    def register_component(self, component: type[Component]) -> ComponentId: ...
    def run_system_once(self, func: SystemFn) -> None:
        """Run a system function once immediately."""
    def trigger(self, event: Event) -> None:
        """Trigger an event immediately.

        Observers watching for this event will execute immediately
        before this function returns.

        Example:
            world.trigger(PlayerDied(player_id=1, cause="explosion"))
        """
    def add_observer(self, observer: SystemFn) -> Entity:
        """Register an observer and return its entity ID for lifecycle management.

        Use this method when you need to manage the observer's lifecycle
        (e.g., despawn it later). For simple observer registration during
        app setup, use app.add_observer() instead.

        Example:
            def on_player_died(trigger: On[PlayerDied]) -> None:
                print(f"Player died")

            def setup(world: World) -> None:
                observer_id = world.add_observer(on_player_died)
                # Later can despawn with: world.despawn_observer(observer_id)
        """
    def despawn_observer(self, observer_entity: Entity) -> None:
        """Despawn an observer entity.

        Removes the observer from the registry and despawns its entity.
        The observer will no longer trigger for events.

        Example:
            observer_id = world.add_observer(on_event)
            # Later...
            world.despawn_observer(observer_id)
        """
    def entity(self, entity: Entity) -> EntityCommands:
        """Get EntityCommands for an existing entity.

        Raises ValueError if the entity does not exist in the world.

        Example:
            entity_cmd = world.entity(entity_id)
            entity_cmd.insert(Transform())
        """
    def get(self, entity: Entity, component_type: type[ComponentTypeVar]) -> ComponentTypeVar | None:
        """Get a read-only reference to a component on an entity.

        Returns None if the entity doesn't have the component or doesn't exist.

        Example:
            transform = world.get(entity, Transform)
            if transform is not None:
                print(f"Position: {transform.translation}")
        """
    def get_mut(self, entity: Entity, component_type: type[ComponentTypeVar]) -> ComponentTypeVar | None:
        """Get a mutable reference to a component on an entity.

        Returns None if the entity doesn't have the component or doesn't exist.
        The returned component reference can be modified and changes persist to the ECS.

        Example:
            transform = world.get_mut(entity, Transform)
            if transform is not None:
                transform.translation.x += 10.0
        """
    def get_assets_resource(self, asset_type: Any) -> Any:
        """Internal method to get the Assets resource for a specific asset type."""
    def run_schedule(self, label: Stage | Any) -> None:
        """Run a specific schedule on this World.

        Accepts Stage values (SimTick, Update, etc.) or state-based schedule
        labels (OnEnter, OnExit, OnTransition).

        When called from within an exclusive system (one that takes World),
        the GIL is released so inner Python systems can execute without
        deadlock.

        Note: any references obtained from this World before calling
        run_schedule() may be stale afterward. Re-query after the call.

        Example:
            ```python
            def train_system(world: World) -> None:
                for _ in range(steps_per_frame):
                    world.run_schedule(SimTick)
            ```
        """

class QueryParam:
    """Internal type parameter for Query system parameters."""
    def _data_len(self) -> int: ...
    def _filter_len(self) -> int: ...
    @property
    def _data_names(self) -> list[str]: ...

Q = TypeVarTuple("Q")

class With(Generic[Unpack[Q]]):
    """Filter: Only match entities that have all specified components."""


class Without(Generic[Unpack[Q]]):
    """Filter: Only match entities that don't have any of the specified components."""


class Changed(Generic[T]):
    """Filter: Only match entities where the component has changed since the last run.

    Detects when a component is modified (write access occurred) since the last time
    this system ran. Useful for optimization - only process entities when data changes.

    Example:
        # Only update UI when health changes
        def update_health_ui(query: Query[tuple[Health, UIElement], Changed[Health]]) -> None:
            for health, ui in query:
                ui.text = f"HP: {health.value}"
    """

class Added(Generic[T]):
    """Filter: Only match entities where the component was added since the last run.

    Detects when a component is first added to an entity (via spawn or insert) since
    the last time this system ran. Useful for initialization logic.

    Example:
        # Initialize new entities
        def init_player(query: Query[Mut[Player], Added[Player]]) -> None:
            for player in query:
                player.health = player.max_health
    """

class Has(Generic[T]):
    """Filter: Check if entities have a component without fetching it.

    Returns a boolean indicating component presence. Useful when you need to check
    for a component but don't need to access its data.

    Note: Unlike With/Without, Has doesn't filter - it adds a boolean to query results.

    Example:
        # Check if entity has a component
        def check_armor(query: Query[tuple[Entity, Has[Armor]]]) -> None:
            for entity, has_armor in query:
                if has_armor:
                    print(f"Entity {entity} has armor")
    """

class AnyOf(Generic[Unpack[Q]]):
    """Filter: Match entities that have ANY of the specified components.

    This is a disjunction filter - entities match if they have at least one
    of the specified component types.

    Note: This filter is currently not fully implemented in the query runtime.
    Use multiple queries with different With filters as a workaround.

    Example:
        # Match entities with Sprite OR Mesh
        def render_visuals(query: Query[Entity, AnyOf[Sprite, Mesh]]) -> None:
            for entity in query:
                print(f"Entity {entity} has visual component")
    """

T = TypeVar("T", bound="Component | Entity")
Qs = TypeVarTuple("Qs")

class Mut(Generic[T]):
    """
    Marker type for mutable access in ECS queries.

    Use Mut[Component] to indicate mutable access to a component in a query.
    The Mut wrapper is only used for runtime access control - type checkers
    will see the unwrapped component type in query iteration.

    Examples:
        Query[Mut[Transform]] - mutable access, iterates over Transform
        Query[Transform] - read-only access, iterates over Transform
        Query[tuple[Mut[Transform], PointLight]] - mutable Transform, read-only PointLight
    """

    @property
    def inner_type(self) -> type[T]: ...
    @property
    def value(self) -> T: ...
    def get(self) -> T: ...

# Type variables for tuple unwrapping
T1 = TypeVar("T1", bound="Component | Entity")
T2 = TypeVar("T2", bound="Component | Entity")
T3 = TypeVar("T3", bound="Component | Entity")
T4 = TypeVar("T4", bound="Component | Entity")
T5 = TypeVar("T5", bound="Component | Entity")

# Type alias for query parameters - can be Component, Entity, or Mut-wrapped
QueryParam_T = TypeVar("QueryParam_T")

class QueryIter:
    """Iterator for query results.

    Internal implementation class returned by Query.__iter__(). Users typically
    don't need to reference this type directly.
    """
    def __iter__(self) -> QueryIter: ...
    def __next__(self) -> Any: ...
    def single(self) -> Any:
        """Get exactly one entity from the query.

        Returns an error if there are 0 or 2+ entities matching.
        """
    def is_empty(self) -> bool:
        """Check if the query has no matching entities."""
    def get(self, entity: Entity) -> Any | None:
        """Get a specific entity's components if it matches the query."""
    def iter_many(self, entities: Iterable[Entity]) -> list[Any]:
        """Iterate over specific entities that match the query."""

class SingleQuery:
    """Wrapper for Single queries (exactly one matching entity).

    Internal implementation class. Users typically don't need to reference
    this type directly - use Single[T] type hints instead.
    """
    def __iter__(self) -> SingleQuery: ...
    def __next__(self) -> Any: ...

# https://peps.python.org/pep-0646/#variance-type-constraints-and-type-bounds-not-yet-supported

class Query(Generic[QueryParam_T, *Qs]):
    """
    ECS Query for iterating over entities with specific components.

    Examples:
        Query[Transform] - read-only Transform access
        Query[Mut[Transform]] - mutable Transform access
        Query[Mut[Transform], With[Rotate]] - mutable with single filter
        Query[Mut[Transform], tuple[With[Rotate], Without[Player]]] - multiple filters
        Query[tuple[Visibility, Mut[Transform]]] - tuple components
        Query[tuple[Visibility, Mut[Transform]], tuple[With[Rotate]]] - tuple components + filters
        Query[Optional[Transform]] - optional component (None if absent)
        Query[tuple[Transform, Optional[Visibility]]] - mixed required + optional

    The iterator unwraps Mut[T] to T automatically.

    ⚠️ MULTIPLE COMPONENTS SYNTAX
    ═══════════════════════════════════════════════════════════════════════════════
    For multiple components, you MUST use tuple[...] NOT parentheses:

        ✅ Query[tuple[Transform, Velocity]]      - CORRECT
        ❌ Query[(Transform, Velocity)]           - WRONG (raises TypeError)

    ⚠️ CONFLICTING ACCESS
    ═══════════════════════════════════════════════════════════════════════════════
    Within a single system, no two queries may conflict on the same component type.
    This follows Rust's borrowing rules enforced by Bevy.

    What causes conflicts:
        ❌ Query[Transform] + Query[Mut[Transform]]       → read/write conflict
        ❌ Query[Mut[Transform]] + Query[Mut[Transform]]  → write/write conflict
        ✅ Query[Transform] + Query[Transform]            → OK (both read-only)

    IMPORTANT: Having different With[] filters does NOT automatically make queries
    disjoint! PyBevy can only prove disjointness when one query has With[X] and the
    other has Without[X] (or vice versa).

        ❌ WRONG - These still conflict despite different marker components:
            def system(
                robot: Query[Transform, With[Robot]],
                spotlight: Query[Mut[Transform], With[Spotlight]]
            ): ...
            # RuntimeError: conflicting component access to Transform

        ✅ CORRECT - Use Without[] to prove disjointness:
            def system(
                robot: Query[Transform, tuple[With[Robot], Without[Spotlight]]],
                spotlight: Query[Mut[Transform], tuple[With[Spotlight], Without[Robot]]]
            ): ...
            # OK! PyBevy proves these never overlap

    This matches Bevy Rust behavior - see FilteredAccess::is_compatible() in Bevy.

    Filter style: Use tuple[With[...], Without[...]] for multiple filters.
    Matches Bevy Rust: Query<&mut T, (With<A>, Without<B>)>
    """

    @overload
    def __iter__(self: Query[Mut[T]]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Query[T]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Query[Mut[T], With]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Query[T, With]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Query[Mut[T], *Qs]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Query[T, *Qs]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Query[Mut[T], tuple[*Qs]]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Query[T, tuple[*Qs]]) -> Iterator[T]: ...

    @overload
    def __iter__(self: Query[T | None]) -> Iterator[T | None]: ...  # type: ignore[overload-overlap]
    @overload
    def __iter__(self: Query[T | None, *Qs]) -> Iterator[T | None]: ...

    @overload
    def __iter__(self: Query[tuple[T]]) -> Iterator[tuple[T]]: ...
    @overload
    def __iter__(self: Query[tuple[Mut[T]]]) -> Iterator[tuple[T]]: ...

    @overload
    def __iter__(self: Query[tuple[T1, T2]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[Mut[T1], T2]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[T1, Mut[T2]]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[Mut[T1], Mut[T2]]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[T1, T2], With]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[Mut[T1], T2], With]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[T1, Mut[T2]], With]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2]], With],
    ) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[T1, T2], With[*Qs]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2], With[*Qs]],
    ) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2]], With[*Qs]],
    ) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2]], With[*Qs]],
    ) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[T1, T2], *Qs]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[Mut[T1], T2], *Qs]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[T1, Mut[T2]], *Qs]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2]], *Qs],
    ) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Query[tuple[T1, T2], tuple[*Qs]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2], tuple[*Qs]],
    ) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2]], tuple[*Qs]],
    ) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2]], tuple[*Qs]],
    ) -> Iterator[tuple[T1, T2]]: ...

    @overload
    def __iter__(
        self: Query[tuple[T1, T2, T3], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2, T3], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2], T3], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, T2, Mut[T3]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2], T3], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2, Mut[T3]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2], Mut[T3]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2], Mut[T3]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3]]: ...


    @overload
    def __iter__(
        self: Query[tuple[T1, T2, T3, T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2, T3, T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2], T3, T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, T2, Mut[T3], T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, T2, T3, Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...

    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2], Mut[T3], Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...


    @overload
    def get(self: Query[Mut[T], *Qs], entity: Entity) -> T | None: ...
    @overload
    def get(self: Query[T, *Qs], entity: Entity) -> T | None: ...
    @overload
    def get(self: Query[T | None, *Qs], entity: Entity) -> T | None: ...
    @overload
    def get(
        self: Query[tuple[T1, Mut[T2]], *Qs], entity: Entity
    ) -> tuple[T1, T2] | None: ...
    @overload
    def get(
        self: Query[tuple[Mut[T1], T2], *Qs], entity: Entity
    ) -> tuple[T1, T2] | None: ...


    @overload
    def get_mut(self: Query[Mut[T], *Qs], entity: Entity) -> T | None: ...
    @overload
    def get_mut(self: Query[T, *Qs], entity: Entity) -> T | None: ...


    @overload
    def single(self: Query[Mut[T], *Qs]) -> T: ...
    @overload
    def single(self: Query[T, *Qs]) -> T: ...
    @overload
    def single(self: Query[T | None, *Qs]) -> T | None: ...
    @overload
    def single(self: Query[tuple[T1, Mut[T2]], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def single(self: Query[tuple[Mut[T1], T2], *Qs]) -> tuple[T1, T2]: ...

    def __len__(self) -> int:
        """Get the number of entities matching this query."""

    def is_empty(self) -> bool:
        """Check if the query has no matching entities.

        Returns:
            True if there are no entities matching the query filters, False otherwise.

        Example:
            if player_query.is_empty():
                print("No players found")
        """

    @overload
    def iter_many(self: Query[Mut[T], *Qs], entities: Iterable[Entity]) -> list[T]:
        """Iterate over query results for a specific list of entities.

        Entities that don't match the query filters are skipped.

        Args:
            entities: An iterable of Entity objects to query

        Returns:
            A list of query results for matching entities (in same order, skipping non-matching)

        Example:
            entities = [entity1, entity2, entity3]
            transforms = query.iter_many(entities)
            for t in transforms:
                t.translation.x += 1.0
        """
    @overload
    def iter_many(self: Query[T, *Qs], entities: Iterable[Entity]) -> list[T]:
        """Iterate over query results for a specific list of entities.

        Entities that don't match the query filters are skipped.

        Args:
            entities: An iterable of Entity objects to query

        Returns:
            A list of query results for matching entities (in same order, skipping non-matching)
        """
    @overload
    def iter_many(self: Query[tuple[T1, Mut[T2]], *Qs], entities: Iterable[Entity]) -> list[tuple[T1, T2]]:
        """Iterate over query results for a specific list of entities.

        Entities that don't match the query filters are skipped.

        Args:
            entities: An iterable of Entity objects to query

        Returns:
            A list of tuples containing query results for matching entities
        """
    @overload
    def iter_many(self: Query[tuple[Mut[T1], T2], *Qs], entities: Iterable[Entity]) -> list[tuple[T1, T2]]:
        """Iterate over query results for a specific list of entities.

        Entities that don't match the query filters are skipped.

        Args:
            entities: An iterable of Entity objects to query

        Returns:
            A list of tuples containing query results for matching entities
        """

class Single(Generic[QueryParam_T, *Qs]):
    """
    Single entity query that enforces exactly one entity matches.

    Panics if zero or multiple entities match the query filter.

    Examples:
        Single[Player] - single Player entity (read-only)
        Single[Mut[Transform], With[Player]] - single Player Transform (mutable)
        Single[tuple[Mut[Transform], Player]] - single entity with both components

    Usage:
        def system(player: Single[tuple[Mut[Transform], Player]]) -> None:
            # player is directly the tuple, not an iterator
            transform, player_data = player
            transform.translation.x += 10.0

    Note: Unlike Query, Single does not return an iterator. It returns the
    single result directly or panics.
    """

    # Single component overloads
    @overload
    def __iter__(self: Single[Mut[T]]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Single[T]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Single[Mut[T], *Qs]) -> Iterator[T]: ...
    @overload
    def __iter__(self: Single[T, *Qs]) -> Iterator[T]: ...

    # Single-element tuple overloads
    @overload
    def __iter__(self: Single[tuple[T1]]) -> Iterator[tuple[T1]]: ...
    @overload
    def __iter__(self: Single[tuple[Mut[T1]]]) -> Iterator[tuple[T1]]: ...

    # Two-element tuple component overloads
    @overload
    def __iter__(self: Single[tuple[T1, T2]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Single[tuple[Mut[T1], T2]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Single[tuple[T1, Mut[T2]]]) -> Iterator[tuple[T1, T2]]: ...
    @overload
    def __iter__(self: Single[tuple[Mut[T1], Mut[T2]]]) -> Iterator[tuple[T1, T2]]: ...

    # Fallback
    @overload
    def __iter__(self) -> Iterator[Any]: ...
    def __next__(self) -> Any: ...

V = TypeVar("V")  # , bound=Callable)

type Local[T] = T
"""Per-system local state, persisted across system invocations.

Local[T] provides per-system state that is initialized to T() on first run
and persisted between system calls. Each system gets its own independent copy.

Example:
    ```python
    def fps_system(time: Res[Time], counter: Local[FPSCounter]) -> None:
        counter.frame_count += 1
    ```
"""

class RelatedSpawnerCommands:
    """Commands for spawning child entities within with_children().

    Provides spawn methods that automatically set up parent-child relationships.
    The spawned children will have ChildOf component pointing to the parent.

    IMPORTANT: When using with_children() with a lambda, remember that Python
    lambdas can only contain a single expression. For multiple children, wrap
    the spawn calls in a tuple:

    Example - Multiple children:
        ```python
        commands.spawn(Transform()).with_children(lambda parent: (
            parent.spawn(Mesh3d(mesh1), Transform.from_xyz(-1, 0, 0)),
            parent.spawn(Mesh3d(mesh2), Transform.from_xyz(1, 0, 0)),
        ))
        ```

    Example - Nested hierarchy with entity():
        ```python
        commands.spawn(Transform()).with_children(lambda parent: (
            (child := parent.spawn(Transform()).id()),
            parent.entity(child).with_children(lambda p2:
                p2.spawn(Mesh3d(grandchild_mesh))
            ),
        ))
        ```
    """

    def __init__(self, commands: Commands, target: Entity) -> None: ...

    def spawn(self, *components: Component) -> EntityCommands:
        """Spawn a child entity with given components.

        The child automatically gets a ChildOf component pointing to parent.

        Example:
            parent.spawn(Mesh3d(mesh), Transform.from_xyz(0, 1, 0))
        """

    def spawn_empty(self) -> EntityCommands:
        """Spawn an empty child entity (for later component insertion)."""

    def target_entity(self) -> Entity:
        """Get the parent entity ID."""

# State management
StateType = TypeVar("StateType")

class State(Generic[StateType], Resource):
    """Current state resource.

    Holds the current state value for a state machine.
    Created automatically by app.init_state() or app.insert_state().

    Example:
        def check_state(current: Res[State]) -> None:
            if current.get() == GameState.MENU:
                print("In menu")
    """
    def __init__(self, initial_state: StateType) -> None:
        """Create a State resource with an initial state value.

        Usually created via app.init_state() or app.insert_state().
        """

    def get(self) -> StateType:
        """Get the current state value."""

class NextState(Generic[StateType], Resource):
    """Pending state transition resource.

    Queue state transitions using set(). The transition will be applied
    by the StateTransition schedule (or manually via Commands).

    Example:
        def start_game(next_state: ResMut[NextState]) -> None:
            next_state.set(GameState.IN_GAME)
    """
    def set(self, state: StateType) -> None:
        """Queue a state transition."""

    def is_pending(self) -> bool:
        """Check if a transition is pending."""

    def peek_pending(self) -> StateType | None:
        """Get the pending state without removing it."""

    def reset(self) -> None:
        """Clear the pending transition."""

def state(cls: type[StateType]) -> type[StateType]:
    """Decorator to mark an Enum as a valid state type.

    Example:
        @state
        class GameState(Enum):
            MENU = auto()
            IN_GAME = auto()
    """

def in_state(state: StateType) -> Callable[[Res[State]], bool]:
    """Create a run condition that checks if current state matches target state.

    Args:
        state: The target state to check for

    Returns:
        A run condition function

    Example:
        app.add_systems(Update, menu_system, run_if=in_state(GameState.MENU))
    """

# Schedule label types (internal - returned by OnEnter/OnExit/OnTransition functions)
class OnEnterSchedule:
    """Schedule label for systems that run when entering a state.

    This is the internal type returned by OnEnter(). Users typically don't
    need to reference this type directly.
    """

class OnExitSchedule:
    """Schedule label for systems that run when exiting a state.

    This is the internal type returned by OnExit(). Users typically don't
    need to reference this type directly.
    """

class OnTransitionSchedule:
    """Schedule label for systems that run during state transitions.

    This is the internal type returned by OnTransition(). Users typically don't
    need to reference this type directly.
    """

# Schedule labels for state transitions (functions that return schedule labels)
def OnEnter(state: StateType) -> OnEnterSchedule:
    """Create a schedule label for systems that run when entering a state.

    Example:
        app.add_systems(OnEnter(GameState.MENU), setup_menu)
    """

def OnExit(state: StateType) -> OnExitSchedule:
    """Create a schedule label for systems that run when exiting a state.

    Example:
        app.add_systems(OnExit(GameState.MENU), cleanup_menu)
    """

def OnTransition(from_state: StateType, to_state: StateType) -> OnTransitionSchedule:
    """Create a schedule label for systems that run on a specific state transition.

    Example:
        app.add_systems(OnTransition(GameState.MENU, GameState.IN_GAME), start_game)
    """

# Components for automatic entity lifecycle management
class DespawnOnExit(Component, Generic[StateType]):
    """Component that marks an entity to be despawned when exiting a state.

    Example:
        commands.spawn(Sprite(), DespawnOnExit(GameState.MENU))
    """
    def __init__(self, state: StateType) -> None: ...
    def state_value(self) -> StateType:
        """Get the state value this component is associated with."""

class DespawnOnEnter(Component, Generic[StateType]):
    """Component that marks an entity to be despawned when entering a state.

    Example:
        commands.spawn(Sprite(), DespawnOnEnter(GameState.PAUSE_MENU))
    """
    def __init__(self, state: StateType) -> None: ...
    def state_value(self) -> StateType:
        """Get the state value this component is associated with."""

class ChildOf(Component):
    """Relationship component indicating the parent entity.

    Used to build entity hierarchies. Spawn with ChildOf(parent_entity)
    to make an entity a child of the parent.
    """
    def __init__(self, parent: Entity) -> None: ...
    def parent(self) -> Entity:
        """Get the parent entity."""
    def __eq__(self, other: object) -> bool: ...

class Children(Component):
    """Auto-managed list of child entities (read-only).

    Cannot be created from Python — it is automatically managed by Bevy
    when ChildOf components are added. Query it to iterate over children.
    """
    def entities(self) -> list[Entity]:
        """Get all child entities as a list."""
    def __len__(self) -> int: ...
    def is_empty(self) -> bool: ...
    def __iter__(self) -> Iterator[Entity]: ...
    def __getitem__(self, index: int) -> Entity: ...

class MessageId:
    """A unique identifier for a sent message.

    Returned by MessageWriter.write() and can be used to track message delivery.
    """
