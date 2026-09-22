from collections.abc import Callable, Iterable, Iterator
from enum import Enum
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
from typing import (
    Optional as Optional,
)

from pybevy.app import (
    ChainedSystems,
    ChainedSystemSets,
    Stage,
    SystemChainItem,
    SystemFn,
    SystemSetChainItem,
)
from pybevy.expr import Expr
from pybevy.expr import FieldExpr as _FieldExpr
from pybevy.light import PointLight
from pybevy.transform import Transform

# Bound so View[...] and bare component types are rejected. The whole Query is
# one variable rather than (data, *filters): tying the return to reconstructed
# parts lets an enclosing context back-infer through Query's invariant
# parameter, which rejects valid calls like iter(world.query(Query[Mut[T]])).
_WorldQueryT = TypeVar("_WorldQueryT", bound=Query[Any, *tuple[Any, ...]])

@runtime_checkable
class Batchable(Protocol):
    """Protocol for batch component data returned by batch() methods.

    Returned by built-in components (Transform, Visibility, etc.) and
    @component-decorated classes' batch() (wrapper-storage only).
    Users should not implement this protocol directly.
    """

    def count(self) -> int:
        """The number of entities the batch will spawn."""

    def __len__(self) -> int:
        """The same number as `count()`."""

class Message: ...

class Resource(Component):
    """Base class for ECS resources (global singleton data).

    Each resource type is a component stored on one stable resource entity.
    Prefer Res[T] or ResMut[T] for ordinary system access; native resources can
    also be read through Query[T] on that entity.

    IMPORTANT: Custom resources MUST use BOTH the @resource decorator AND
    inherit from Resource. Using only one will cause a runtime error.

    Example:
        ```python
        from dataclasses import dataclass
        from pybevy.decorators import resource

        @resource  # Required decorator
        @dataclass
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

    Events are used for immediate communication through observers. The optional
    ``@event`` decorator validates this inheritance and documents intent; inheriting
    from Event remains sufficient for compatibility.

    Example:
        ```python
        from dataclasses import dataclass

        @event
        @dataclass
        class PlayerDied(Event):
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
World custom-resource proxies preserve value equality and inequality. They are
unhashable, and comparison through expired World handles raises RuntimeError.

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
World custom-resource proxies preserve value equality and inequality. They are
unhashable, and comparison through expired World handles raises RuntimeError.

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

    Messages written before a reader runs are visible in the same schedule pass.
    Buffered messages are retained across two admitted message-update cycles.
    A system cannot contain a writer and another reader or writer for the same
    message channel; split that work into ordered systems or distinct message types.

    Example:
        def my_system(writer: MessageWriter[AppExit]) -> None:
            writer.write(AppExit.Success())
    """
    def write(self, message: M) -> MessageId:
        """Write a message to the message buffer."""
    def write_batch(self, messages: list[M]) -> list[MessageId]:
        """Validate and write a batch atomically, returning contiguous message IDs."""
    def write_default(self) -> MessageId:
        """Write a default instance of the message type."""

class MessageReader(Generic[M]):
    """System parameter for reading messages from the ECS.

    Each reader parameter owns an independent cursor. Inspection methods report
    unread values without consuming them; iteration consumes values as yielded.
    Dropping a partially consumed iterator leaves its remaining values unread.

    Example:
        def my_system(reader: MessageReader[AppExit]) -> None:
            for msg in reader:
                print(f"Received: {msg}")
    """
    def clear(self) -> None:
        """Mark all currently retained messages read for this reader only."""
    def is_empty(self) -> bool:
        """Check if there are any messages."""
    def len(self) -> int:
        """Get this reader's unread message count without consuming it."""
    def read(self) -> Iterator[M]:
        """Get an iterator over messages."""
    def __iter__(self) -> Iterator[M]:
        """Iterate over messages."""

    def __len__(self) -> int: ...

class MessageMutator(Generic[M]):
    """Combined reader/writer for a custom Python message channel.

    Each parameter owns an independent cursor. Messages yielded by ``read()``
    are the retained Python objects, so field mutations are visible to later
    readers. A write before ``read()`` is visible immediately; a write after it
    remains unread by this mutator until its next run.

    This initial surface supports Python-defined custom messages only. Native
    Bevy message wrappers are snapshots and are rejected rather than pretending
    that field mutations persist.
    """
    def write(self, message: M) -> MessageId:
        """Write a message and return its ID."""
    def write_batch(self, messages: list[M]) -> list[MessageId]:
        """Validate and write a batch atomically."""
    def write_default(self) -> MessageId:
        """Write a default instance of the custom message type."""
    def clear(self) -> None:
        """Mark all currently retained messages read for this mutator only."""
    def is_empty(self) -> bool:
        """Return whether this mutator has no unread messages."""
    def len(self) -> int:
        """Return this mutator's unread count without consuming it."""
    def read(self) -> Iterator[M]:
        """Iterate over unread retained messages mutably."""
    def __iter__(self) -> Iterator[M]:
        """Iterate over unread retained messages mutably."""

    def __len__(self) -> int: ...

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

    def __len__(self) -> int: ...

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
    - On[Discard, ComponentType] - Observe component discard lifecycle events
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

class Discard:
    """Lifecycle event marker for component discard.

    Use with On[Discard, ComponentType] to observe when a component value is
    discarded because it is replaced, removed, or despawned. Fires before the
    value is dropped, so observers can still read the original component data.
    Named after bevy's Discard event (formerly Replace).
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
    def __init__(self, system: Any, condition: Callable[..., object]) -> None: ...

    def and_(self, condition: Callable[..., object]) -> ConditionalSystem:
        """Combine with another condition using AND logic.

        Args:
            condition: Another function whose result is evaluated for truthiness

        Returns:
            New ConditionalSystem that runs only if both conditions are true
        """

    def or_(self, condition: Callable[..., object]) -> ConditionalSystem:
        """Combine with another condition using OR logic.

        Args:
            condition: Another function whose result is evaluated for truthiness

        Returns:
            New ConditionalSystem that runs if either condition is true
        """

    def not_(self) -> ConditionalSystem:
        """Negate this condition using NOT logic.

        Returns:
            New ConditionalSystem that runs when the condition is false
        """

class SystemSet:
    """Stable Bevy system-set identity.

    Prefer ``@system_set`` for user-defined sets so the identity is derived
    from the class's fully qualified name and remains stable across hot reload.
    """
    def __init__(self, name: str) -> None: ...
    @property
    def name(self) -> str: ...
    def in_set(self, parent: SystemSet | SystemSetEnum) -> SystemSetConfig: ...
    def before(self, target: SystemSet | SystemSetEnum | SystemFn) -> SystemSetConfig: ...
    def after(self, target: SystemSet | SystemSetEnum | SystemFn) -> SystemSetConfig: ...
    def run_if(self, condition: Callable[..., object]) -> SystemSetConfig: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class SystemSetEnum(Enum):  # type: ignore[misc]
    """Typed base for an enum whose members are distinct Bevy system sets."""
    @property
    def name(self) -> str: ...
    @property
    def value(self) -> object: ...
    def in_set(self, parent: SystemSet | SystemSetEnum) -> SystemSetConfig: ...
    def before(self, target: SystemSet | SystemSetEnum | SystemFn) -> SystemSetConfig: ...
    def after(self, target: SystemSet | SystemSetEnum | SystemFn) -> SystemSetConfig: ...
    def run_if(self, condition: Callable[..., object]) -> SystemSetConfig: ...

class SystemConfig:
    """Immutable fluent scheduling configuration for one system callable."""
    def __init__(self, system: SystemFn) -> None: ...
    def in_set(self, set: SystemSet | SystemSetEnum) -> SystemConfig: ...
    def before(self, target: SystemSet | SystemSetEnum | SystemFn | SystemConfig) -> SystemConfig: ...
    def after(self, target: SystemSet | SystemSetEnum | SystemFn | SystemConfig) -> SystemConfig: ...
    def run_if(self, condition: Callable[..., object]) -> SystemConfig: ...
    def pipe(self, target: SystemFn) -> SystemConfig:
        """Pass this system's return value into a downstream ``In[T]`` system."""

class SystemSetConfig:
    """Immutable fluent scheduling configuration for one system set."""
    def in_set(self, parent: SystemSet | SystemSetEnum) -> SystemSetConfig: ...
    def before(self, target: SystemSet | SystemSetEnum | SystemFn) -> SystemSetConfig: ...
    def after(self, target: SystemSet | SystemSetEnum | SystemFn) -> SystemSetConfig: ...
    def run_if(self, condition: Callable[..., object]) -> SystemSetConfig: ...

def system(system: SystemFn, /) -> SystemConfig:
    """Wrap a callable for ``in_set``, ``before``, ``after``, or ``run_if``."""

def pipe(source: SystemFn, target: SystemFn, /) -> SystemConfig:
    """Compose two systems, passing ``source``'s output to ``target``'s ``In[T]``."""

_SystemSetEnumT = TypeVar("_SystemSetEnumT", bound=SystemSetEnum)

@overload
def system_set(cls: type[_SystemSetEnumT], /) -> type[_SystemSetEnumT]: ...  # type: ignore[overload-overlap]
@overload
def system_set(cls: type[object], /) -> SystemSet:
    """Declare a system set using the class's stable fully qualified name."""

def run_if(system: SystemFn, condition: Callable[..., object]) -> ConditionalSystem:
    """Create a conditional system that only runs when condition returns true.

    Args:
        system: The system function to run conditionally
        condition: A function whose result is evaluated for truthiness (can have system parameters)

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
    ) -> None:
        """Queue entities from batch/uniform components until command flush."""
    @overload
    def spawn_batch(self, iterable: Iterable[tuple[Component, ...]], /) -> None:
        """Spawn entities from an iterable of component tuples."""
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
    def batch(**kwargs: object) -> Batchable:
        """Create a batch of components from numpy arrays for spawn_batch().

        Added by the @component decorator. Calling it on a storage="python"
        component raises TypeError because Python-object storage is not batchable.
        """

class IsResource(Component):
    """Engine-managed marker attached to each Bevy resource entity."""

    @property
    def resource_component_id(self) -> ComponentId: ...

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

class ViewColumn:
    """Opaque concrete column buffer for zero-copy access via Numba JIT and JAX interop.

    ViewColumn is an opaque handle that provides zero-copy access to Bevy ECS
    component data. It CANNOT be converted to numpy arrays. Access data through
    @numba.jit functions (zero-copy) or JAX (copy-based, supports GPU).

    Obtain a ViewColumn with Batch.column()/column_mut() inside
    View.iter_batches(). View.column()/column_mut() yield lazy expressions.

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
            for batch in view.iter_batches():
                y = batch.column_mut(Transform).translation.y
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
        """Get number of elements. Checks validity."""

    @property
    def stride(self) -> int:
        """Get stride in bytes. Checks validity."""

    @property
    def writable(self) -> bool:
        """Whether writes are allowed by the originating View declaration. Checks validity."""

    @property
    def dtype(self) -> str:
        """Get NumPy dtype string (e.g., 'f4' for float32). Checks validity."""

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

_FieldT = TypeVar("_FieldT", default=_FieldExpr)

class Vec3Expr(Generic[_FieldT]):
    """Represents a Vec3 field in a View expression (e.g., translation, scale).

    Provides x, y, z component access with assignment support.
    """
    @property
    def x(self) -> _FieldT: ...
    @x.setter
    def x(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def y(self) -> _FieldT: ...
    @y.setter
    def y(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def z(self) -> _FieldT: ...
    @z.setter
    def z(self, value: Expr | ViewColumn | float | int) -> None: ...

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

    @overload
    def from_jax(self, obj: Any) -> None:
        """Write back from object with .x, .y, .z attributes. Requires `import pybevy.ecs.jax_ext`."""
    @overload
    def from_jax(self, x: Any, y: Any, z: Any) -> None:
        """Write back from 3 separate JAX arrays. Requires `import pybevy.ecs.jax_ext`."""
    def from_jax(self, x_or_obj: Any, y: Any = ..., z: Any = ...) -> None: ...  # type: ignore[misc]

class QuatExpr(Generic[_FieldT]):
    """Represents a Quat field in a View expression (e.g., rotation).

    Provides x, y, z, w component access with assignment support.
    """
    @property
    def x(self) -> _FieldT: ...
    @x.setter
    def x(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def y(self) -> _FieldT: ...
    @y.setter
    def y(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def z(self) -> _FieldT: ...
    @z.setter
    def z(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def w(self) -> _FieldT: ...
    @w.setter
    def w(self, value: Expr | ViewColumn | float | int) -> None: ...

    @overload
    def from_jax(self, obj: Any) -> None:
        """Write back from object with .x, .y, .z, .w attributes. Requires `import pybevy.ecs.jax_ext`."""
    @overload
    def from_jax(self, x: Any, y: Any, z: Any, w: Any) -> None:
        """Write back from 4 separate JAX arrays. Requires `import pybevy.ecs.jax_ext`."""
    def from_jax(self, x_or_obj: Any, y: Any = ..., z: Any = ..., w: Any = ...) -> None: ...  # type: ignore[misc]

class TransformViewColumn(ViewColumn, Generic[_FieldT]):
    """Static typing facade for Transform columns returned by View and Batch.

    The runtime object resolves these structured fields dynamically. As
    returned by Batch.column()/column_mut() it is a concrete ViewColumn buffer
    (len/dtype/stride, Numba zero-copy indexing, optional JAX copy conversion).
    View.column()/column_mut() return a lazy expression column.
    """

    translation: Vec3Expr[_FieldT]
    rotation: QuatExpr[_FieldT]
    scale: Vec3Expr[_FieldT]

class PointLightViewColumn(ViewColumn, Generic[_FieldT]):
    """View column accessor for PointLight component.

    Provides field-level access for batch operations on light properties.
    Concrete ViewColumn buffer when returned by Batch.column()/column_mut();
    lazy expression column when returned by View.column()/column_mut().
    """

    @property
    def intensity(self) -> _FieldT: ...
    @intensity.setter
    def intensity(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def range(self) -> _FieldT: ...
    @range.setter
    def range(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def radius(self) -> _FieldT: ...
    @radius.setter
    def radius(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def shadow_depth_bias(self) -> _FieldT: ...
    @shadow_depth_bias.setter
    def shadow_depth_bias(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def shadow_normal_bias(self) -> _FieldT: ...
    @shadow_normal_bias.setter
    def shadow_normal_bias(self, value: Expr | ViewColumn | float | int) -> None: ...
    @property
    def shadow_map_near_z(self) -> _FieldT: ...
    @shadow_map_near_z.setter
    def shadow_map_near_z(self, value: Expr | ViewColumn | float | int) -> None: ...

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
    """Read-only lazy expression column returned by View.column().

    Field access returns expressions over one component of an injected View.
    Assignments raise a RuntimeError that names column_mut().
    For a concrete ViewColumn buffer, use View.iter_batches() and Batch.column().
    """

    @property
    def component_id(self) -> int: ...

class ViewColMut:
    """Mutable lazy expression column returned by View.column_mut().

    A proxy over one mutable component of an injected View. Assigning to a
    field compiles and runs the expression across all matching entities.
    For a concrete ViewColumn buffer, use View.iter_batches() and Batch.column_mut().
    """

    @property
    def component_id(self) -> int: ...

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
    on all matching entities. Measured workloads commonly range from about
    5-7x faster for conditional work to 20-25x for pure column math; measure
    the actual system on target hardware.

    IMPORTANT:
    - Use Mut[T] for mutable access, plain T for read-only (just like Query)
    - Filters must be explicit (With[T], Without[T], etc.), not bare component types

    Column access has two models:
    - column(T) and column_mut(T) return a lazy expression column. Field
      access yields expressions; field assignment compiles and runs the
      expression across all matching entities in one step.
    - For a concrete ViewColumn buffer, iterate with iter_batches() and call
      batch.column(T) or batch.column_mut(T). It has len/dtype/stride, Numba
      zero-copy indexing, and optional JAX copy conversion (to_jax()/
      from_jax()), with a batch-scoped lifetime that expires when the batch
      ends. It refuses direct NumPy conversion (__array__ raises); a NumPy
      array is a copy built with to_contiguous_bytes() and numpy.frombuffer().

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
        Iterate over contiguous filtered batches (PyArrow-style chunked iteration).

        This is the route to concrete ViewColumn buffers: batch.column(T) and
        batch.column_mut(T) return real ViewColumn objects with len/dtype/stride,
        Numba zero-copy indexing, and optional JAX copy conversion (to_jax()/
        from_jax()). A ViewColumn refuses direct NumPy conversion (__array__
        raises); a NumPy array is a copy built with to_contiguous_bytes() and
        numpy.frombuffer(). These buffers are scoped to the batch and expire
        when the batch ends, so use them within the loop.

        This provides a PyArrow-style chunked API where data is processed in
        maximal contiguous row runs from one ECS table. Changed/Added filters
        may split a table so filtered-out rows are never exposed.

        Returns:
            Iterator of Batch objects, one per contiguous passing row run

        Example:
            ```python
            import numba

            @numba.jit(nopython=True, cache=True)
            def process(x, y, t):
                for i in range(len(x)):
                    if y[i] > 0.5:  # Only visible entities
                        x[i] = x[i] * 0.9

            def wiggle_system(view: View[Mut[Transform], With[Marker]], time: Time) -> None:
                # Concrete ViewColumn buffers, scoped to this batch.
                for batch in view.iter_batches():
                    pos = batch.column_mut(Transform)
                    process(pos.translation.x, pos.translation.y, time.elapsed_secs())
            ```

        Performance:
            - Each batch is processed in native code via Numba
            - Better cache locality (archetypes have similar components)
            - Can parallelize across batches in the future
            - Typical batch sizes: 100-10,000 entities per table

        Note:
            This API is designed to match PyArrow's batching pattern, familiar
            to data scientists. Each batch represents a contiguous chunk of
            component data from Bevy's Table storage.
        """

class Batch:
    """
    A contiguous batch of selected entities from one ECS table.

    Represents a contiguous slice of selected component data from one table.
    Its columns are concrete ViewColumn buffers with len/dtype/stride: Numba
    zero-copy indexing, optional JAX copy conversion (to_jax()/from_jax()), and
    a copied NumPy array via to_contiguous_bytes() and numpy.frombuffer(). A
    ViewColumn refuses direct NumPy conversion (__array__ raises).

    This is the ECS equivalent of PyArrow's RecordBatch - a chunk of
    columnar data that can be processed efficiently.
    """

    @overload
    def column(self, component_type: type[Transform]) -> TransformViewColumn[ViewColumn]: ...
    @overload
    def column(self, component_type: type[PointLight]) -> PointLightViewColumn[ViewColumn]: ...
    @overload
    def column(self, component_type: type[ComponentTypeVar]) -> ViewColumn: ...

    @overload
    def column_mut(self, component_type: type[Transform]) -> TransformViewColumn[ViewColumn]: ...
    @overload
    def column_mut(self, component_type: type[PointLight]) -> PointLightViewColumn[ViewColumn]: ...
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

class EntityIndex:
    """The transient index portion of a Bevy entity identity.

    An index is unique only among currently active entities and can be reused
    after despawn. It is not an MCP/HTTP entity ID.
    """

    def __copy__(self) -> EntityIndex: ...
    def __deepcopy__(self, memo: dict[int, object]) -> EntityIndex: ...

    @staticmethod
    def from_raw(raw: int) -> EntityIndex | None:
        """Construct an index, or return ``None`` for ``2**32 - 1``."""
    def index(self) -> int:
        """Return the raw integer index used in entity diagnostics."""
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Entity:
    """A generation-bearing Bevy entity identity.

    ``index()`` returns the typed transient index shown in ``repr(entity)``. It
    is useful for correlating diagnostics, but is not an MCP/HTTP entity ID.
    Use the packed ``to_bits()`` value at wire boundaries.
    """
    @staticmethod
    def from_raw(raw: int) -> Entity | None:
        """Construct a generation-zero identity from an index.

        This does not recover an arbitrary live entity that currently uses the
        index. Use ``from_bits()`` for a packed identity returned by MCP.
        """
    def index(self) -> EntityIndex:
        """Return the typed transient index shown in the entity repr."""
    def to_bits(self) -> int:
        """Return the packed generation-safe identity used by MCP and HTTP."""
    @staticmethod
    def from_bits(bits: int) -> Entity:
        """Reconstruct a packed identity previously returned by ``to_bits()``."""
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

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
    def trigger(self, event: Event) -> EntityCommands:
        """Trigger an event for this entity."""
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
            commands.spawn(Transform()).with_children(lambda parent:
                parent.spawn(Transform.from_xyz(1, 0, 0)).with_children(lambda p2:
                    p2.spawn(Mesh3d(leaf_mesh))
                )
            )
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
    def spawn_empty(self) -> EntityCommands:
        """Spawn an entity without components and return fluent commands."""
    @overload
    def spawn_batch(
        self,
        *components: Component | Batchable,
        count: int | None = None,
        batch: None = None,
    ) -> list[Entity]:
        """Immediately spawn batch/uniform components and return entity handles.

        Uniform-only inputs require count, including components that are iterable.
        """
    @overload
    def spawn_batch(
        self, batch: Iterable[Component | tuple[Component, ...]]
    ) -> list[Entity]:
        """Spawn component bundles and return their entity handles in input order."""
    @overload
    def spawn(self, *components: Component) -> EntityCommands:
        """Spawn components and return fluent commands; call `.id()` for the entity."""
    @overload
    def spawn(self, components: tuple[Component, ...]) -> EntityCommands: ...
    def commands(self) -> Commands:
        """Queue deferred operations on this World; spawned IDs are reserved now."""
    def flush(self) -> None:
        """Apply pending World commands and raise their application errors.

        Exclusive systems and App.world callbacks also flush on exit, including
        error exits. Direct World mutations remain immediate.
        """
    def resource(self, resource: type[ResourceType]) -> ResourceType: ...
    def register_resource(self, resource: type[ResourceType]) -> ComponentId: ...
    def init_resource(self, resource: type[ResourceType]) -> ComponentId: ...
    def insert_resource(self, resource: Resource) -> None: ...
    def remove_resource(self, resource_type: type[ResourceType]) -> ResourceType | None: ...
    def component_id(self, component: type[Component]) -> ComponentId | None:
        """Return or register a component ID, including parameterized ``Assets[T]``."""
    def contains_resource(self, resource: type[ResourceType]) -> bool:
        """Test resource presence, including parameterized ``Assets[T]`` collections."""
    def resource_entity(self, resource: type[ResourceType]) -> Entity | None:
        """Return a resource's stable entity, including ``Assets[T]``, if present."""
    def resource_entities(self) -> Iterator[tuple[ComponentId, Entity]]:
        """Iterate over resource component IDs with allocated Bevy entities."""
    def _get_last_error(self) -> tuple[str, str | None] | None:
        """Get the last system error, if any (PyBevy internal API).

        Returns a tuple of (error_message, traceback) or None if no error.
        """
    def despawn(self, entity: Entity) -> bool: ...
    def register_component(self, component: type[Component]) -> ComponentId: ...
    def run_system_once(self, func: SystemFn) -> None:
        """Run a system function once immediately.

        Outer World/Commands handles and borrowed children raise RuntimeError
        during the call. Use the inner system's injected parameters. Outer access
        resumes after the inner call returns, including when it raises.

        A required Single parameter with zero or multiple matches raises
        RuntimeError; scheduled systems preserve Bevy's silent skip instead.
        """
    def trigger(self, event: Event) -> None:
        """Trigger an event immediately.

        Observers watching for this event will execute immediately
        before this function returns.

        Observers must use their injected parameters; captured outer handles
        raise RuntimeError while an observer is executing on the same World.

        Example:
            world.trigger(PlayerDied(player_id=1, cause="explosion"))
        """
    def write_message(self, message: MessageTypeVar) -> MessageId | None:
        """Write a message, returning None if its channel is not registered."""
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

        The argument must be the Entity returned by add_observer(). A live
        entity that is not a registered observer raises ValueError, and an
        already despawned observer is a no-op.
        Protected resource entities raise TypeError and remain unchanged.

        Example:
            observer_id = world.add_observer(on_event)
            # Later...
            world.despawn_observer(observer_id)
        """
    def iter_entities(self) -> list[Entity]:
        """Return a snapshot of every currently allocated entity."""
    def entity(self, entity: Entity) -> EntityCommands:
        """Get EntityCommands for an existing entity.

        Raises RuntimeError if the entity does not exist in the world.

        Example:
            entity_cmd = world.entity(entity_id)
            entity_cmd.insert(Transform())
        """
    def query(self, param: type[_WorldQueryT]) -> _WorldQueryT:
        """Create an ad-hoc query bound to this World.

        Only accepts Query[...]; View[...] is a system parameter and is
        rejected at runtime.

        Example:
            transforms = world.query(Query[tuple[Entity, Transform]])
            for entity, transform in transforms:
                print(entity, transform.translation)
        """
    def get(self, entity: Entity, component_type: type[ComponentTypeVar]) -> ComponentTypeVar | None:
        """Get a read-only reference to a component on an entity.

        Returns None if the entity doesn't have the component or doesn't exist.
        An entity ID reserved by deferred Commands is not spawned yet, so this
        also returns None until the commands are flushed. This method does not
        flush them.

        Example:
            transform = world.get(entity, Transform)
            if transform is not None:
                print(f"Position: {transform.translation}")
        """
    def get_mut(self, entity: Entity, component_type: type[ComponentTypeVar]) -> ComponentTypeVar | None:
        """Get a mutable reference to a component on an entity.

        Returns None if the entity doesn't have the component or doesn't exist.
        An entity ID reserved by deferred Commands is not spawned yet, so this
        also returns None until the commands are flushed. This method does not
        flush them.
        The returned component reference can be modified and changes persist to the ECS.

        Example:
            transform = world.get_mut(entity, Transform)
            if transform is not None:
                transform.translation.x += 10.0
        """
    def run_schedule(self, label: Stage | Any) -> None:
        """Run a specific schedule on this World.

        Accepts Stage values (SimTick, Update, etc.) or state-based schedule
        labels (OnEnter, OnExit, OnTransition).

        When called from within an exclusive system (one that takes World),
        the GIL is released so inner Python systems can execute without
        deadlock.

        Captured outer World/Commands handles and borrowed children raise
        RuntimeError while the nested schedule runs. Use its systems' injected
        parameters. Outer access resumes when the call returns, including errors.

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
    """Query data: Check if entities have a component without fetching it.

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

class AnyOf(Generic[QueryParam_T]):
    """Query data fetching optional values for components present on an entity.

    The query matches only entities with at least one requested component. Each
    result is a tuple whose corresponding item is the component or ``None``.

    Example:
        def inspect(
            query: Query[AnyOf[tuple[Transform, Visibility]]],
        ) -> None:
            for transform, visibility in query:
                ...
    """

class Or(Generic[QueryParam_T]):
    """Filter matching entities that satisfy any nested query filter.

    Example:
        # Match entities with Sprite OR Mesh
        def render_visuals(
            query: Query[Entity, Or[tuple[With[Sprite], With[Mesh]]]],
        ) -> None:
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
    def __len__(self) -> int: ...
    def __iter__(self) -> Iterator[object]: ...
    def unwrap(self) -> T: ...

# Type variables for tuple unwrapping
T1 = TypeVar("T1", bound="Component | Entity")
T2 = TypeVar("T2", bound="Component | Entity")
T3 = TypeVar("T3", bound="Component | Entity")
T4 = TypeVar("T4", bound="Component | Entity")
T5 = TypeVar("T5", bound="Component | Entity")

# Type alias for query parameters - can be Component, Entity, or Mut-wrapped
QueryParam_T = TypeVar("QueryParam_T")

class QueryIter:
    """Runtime Query object injected into systems.

    Iterating this object returns a fresh QueryIterator. Users typically don't
    need to reference either implementation type directly.
    """
    def __iter__(self) -> QueryIterator: ...
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

    def __len__(self) -> int: ...

class QueryIterator(Iterator[Any]):
    """Iterator for one fresh Query traversal."""
    def __iter__(self) -> QueryIterator: ...
    def __next__(self) -> Any: ...

class SingleQuery:
    """Wrapper for Single queries (exactly one matching entity).

    Internal implementation class. Users typically don't need to reference
    this type directly - use Single[T] type hints instead.
    """
    def into_inner(self) -> object: ...


# https://peps.python.org/pep-0646/#variance-type-constraints-and-type-bounds-not-yet-supported

class Query(Generic[QueryParam_T, *Qs]):
    """Iterate over matching entities as components or tuples.

    Use Query[data, filters], with optional filters in the second position.
    Group multiple data items or filters with tuple[...], not parentheses.
    Extra Query filter type arguments raise TypeError and show the canonical
    Query[Data, tuple[With[A], Without[B]]] form.
    Nested data tuples preserve their nesting in every returned row, matching
    Bevy's recursive tuple QueryData semantics.
    For nested Mut/Has/AnyOf markers, use typing.cast on the returned row when
    the type checker cannot infer the concrete nested result type.
    Mut[T] enables writes and yields T; Optional[T] yields None when absent.

    Examples:
        Query[Transform] - read-only components
        Query[Mut[Transform], With[Player]] - writable components, filtered
        Query[tuple[Transform, Optional[Visibility]]] - tuples of components
        Query[Transform, tuple[With[Player], Without[Enemy]]] - multiple filters
        Query[Entity, With[Player]] - entity IDs

    Filter-only forms such as Query[With[Player]] raise TypeError. Use Entity
    as data when only entity IDs are needed.

    Within a system, overlapping queries cannot access the same component
    if either query writes it. Different With[...] filters alone do not
    prove disjointness; opposing With[T] and Without[T] filters can.
    """

    @overload
    def __iter__(
        self: Query[AnyOf[tuple[T1, T2]], *Qs],
    ) -> Iterator[tuple[T1 | None, T2 | None]]: ...
    @overload
    def __iter__(
        self: Query[AnyOf[tuple[Mut[T1], T2]], *Qs],
    ) -> Iterator[tuple[T1 | None, T2 | None]]: ...
    @overload
    def __iter__(
        self: Query[AnyOf[tuple[T1, Mut[T2]]], *Qs],
    ) -> Iterator[tuple[T1 | None, T2 | None]]: ...
    @overload
    def __iter__(
        self: Query[AnyOf[tuple[Mut[T1], Mut[T2]]], *Qs],
    ) -> Iterator[tuple[T1 | None, T2 | None]]: ...
    @overload
    def __iter__(
        self: Query[AnyOf[tuple[T1, T2, T3]], *Qs],
    ) -> Iterator[tuple[T1 | None, T2 | None, T3 | None]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Entity, AnyOf[tuple[T1, T2]]], *Qs],
    ) -> Iterator[tuple[Entity, tuple[T1 | None, T2 | None]]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, T2, AnyOf[tuple[T3, T4]]], *Qs],
    ) -> Iterator[tuple[T1, T2, tuple[T3 | None, T4 | None]]]: ...
    @overload
    def __iter__(self: Query[Has[T], *Qs]) -> Iterator[bool]: ...
    @overload
    def __iter__(
        self: Query[tuple[Has[T], T1], *Qs],
    ) -> Iterator[tuple[bool, T1]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Has[T]], *Qs],
    ) -> Iterator[tuple[T1, bool]]: ...
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
    def __iter__(self: Query[tuple[()], *Qs]) -> Iterator[tuple[()]]: ...  # type: ignore[overload-overlap]

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
        self: Query[tuple[Mut[T1], Mut[T2], T3, T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2, Mut[T3], T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2, T3, Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2], Mut[T3], T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2], T3, Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, T2, Mut[T3], Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2], Mut[T3], T4], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2], T3, Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], T2, Mut[T3], Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[T1, Mut[T2], Mut[T3], Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[tuple[Mut[T1], Mut[T2], Mut[T3], Mut[T4]], *Qs],
    ) -> Iterator[tuple[T1, T2, T3, T4]]: ...
    @overload
    def __iter__(
        self: Query[QueryParam_T, *Qs],
    ) -> Iterator[QueryParam_T]: ...


    @overload
    def get(
        self: Query[AnyOf[tuple[T1, T2]], *Qs], entity: Entity
    ) -> tuple[T1 | None, T2 | None] | None: ...
    @overload
    def get(
        self: Query[AnyOf[tuple[Mut[T1], T2]], *Qs], entity: Entity
    ) -> tuple[T1 | None, T2 | None] | None: ...
    @overload
    def get(
        self: Query[AnyOf[tuple[T1, Mut[T2]]], *Qs], entity: Entity
    ) -> tuple[T1 | None, T2 | None] | None: ...
    @overload
    def get(
        self: Query[AnyOf[tuple[Mut[T1], Mut[T2]]], *Qs], entity: Entity
    ) -> tuple[T1 | None, T2 | None] | None: ...
    @overload
    def get(self: Query[Has[T], *Qs], entity: Entity) -> bool | None: ...
    @overload
    def get(
        self: Query[tuple[Has[T], T1], *Qs], entity: Entity
    ) -> tuple[bool, T1] | None: ...
    @overload
    def get(
        self: Query[tuple[T1, Has[T]], *Qs], entity: Entity
    ) -> tuple[T1, bool] | None: ...
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
    def get(
        self: Query[QueryParam_T, *Qs], entity: Entity
    ) -> QueryParam_T | None: ...

    @overload
    def single(
        self: Query[AnyOf[tuple[T1, T2]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def single(
        self: Query[AnyOf[tuple[Mut[T1], T2]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def single(
        self: Query[AnyOf[tuple[T1, Mut[T2]]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def single(
        self: Query[AnyOf[tuple[Mut[T1], Mut[T2]]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def single(self: Query[Has[T], *Qs]) -> bool: ...
    @overload
    def single(self: Query[tuple[Has[T], T1], *Qs]) -> tuple[bool, T1]: ...
    @overload
    def single(self: Query[tuple[T1, Has[T]], *Qs]) -> tuple[T1, bool]: ...
    @overload
    def single(self: Query[Mut[T], *Qs]) -> T: ...
    @overload
    def single(self: Query[T, *Qs]) -> T: ...
    @overload
    def single(self: Query[T | None, *Qs]) -> T | None: ...
    @overload
    def single(self: Query[tuple[Mut[T1], Mut[T2]], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def single(self: Query[tuple[T1, Mut[T2]], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def single(self: Query[tuple[Mut[T1], T2], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def single(self: Query[QueryParam_T, *Qs]) -> QueryParam_T: ...

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
    def iter_many(
        self: Query[AnyOf[tuple[T1, T2]], *Qs], entities: Iterable[Entity]
    ) -> list[tuple[T1 | None, T2 | None]]: ...
    @overload
    def iter_many(
        self: Query[AnyOf[tuple[Mut[T1], T2]], *Qs], entities: Iterable[Entity]
    ) -> list[tuple[T1 | None, T2 | None]]: ...
    @overload
    def iter_many(
        self: Query[AnyOf[tuple[T1, Mut[T2]]], *Qs], entities: Iterable[Entity]
    ) -> list[tuple[T1 | None, T2 | None]]: ...
    @overload
    def iter_many(
        self: Query[AnyOf[tuple[Mut[T1], Mut[T2]]], *Qs], entities: Iterable[Entity]
    ) -> list[tuple[T1 | None, T2 | None]]: ...
    @overload
    def iter_many(
        self: Query[Has[T], *Qs], entities: Iterable[Entity]
    ) -> list[bool]: ...
    @overload
    def iter_many(
        self: Query[tuple[Has[T], T1], *Qs], entities: Iterable[Entity]
    ) -> list[tuple[bool, T1]]: ...
    @overload
    def iter_many(
        self: Query[tuple[T1, Has[T]], *Qs], entities: Iterable[Entity]
    ) -> list[tuple[T1, bool]]: ...
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
    @overload
    def iter_many(
        self: Query[QueryParam_T, *Qs], entities: Iterable[Entity]
    ) -> list[QueryParam_T]: ...

class Single(Generic[QueryParam_T, *Qs]):
    """
    Single entity query that enforces exactly one entity matches.

    Skips the system before its body runs if zero or multiple entities match.
    Optional[Single[T]], using typing.Optional re-exported by this module,
    instead injects None for invalid cardinality and runs the system with
    unchanged scheduler access. Single[T] | None is also supported.
    Group multiple filters in the second position with tuple[...], as for Query.

    Examples:
        Single[Player] - single Player entity (read-only)
        Single[Mut[Transform], With[Player]] - single Player Transform (mutable)
        Single[tuple[Mut[Transform], Player]] - single entity with both components

    Usage:
        def system(row: Single[Mut[Transform]]) -> None:
            transform = row.into_inner()
            transform.translation.x += 10.0

        def pair(row: Single[tuple[Mut[Transform], Player]]) -> None:
            transform, player_data = row.into_inner()
            transform.translation.x += 10.0
    Use into_inner() for precisely typed component access and tuple unpacking.
    Component attribute reads and writes require extraction.
    The Single holder does not support iteration or indexed access.
    """

    @overload
    def into_inner(
        self: Single[AnyOf[tuple[T1, T2]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def into_inner(
        self: Single[AnyOf[tuple[Mut[T1], T2]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def into_inner(
        self: Single[AnyOf[tuple[T1, Mut[T2]]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def into_inner(
        self: Single[AnyOf[tuple[Mut[T1], Mut[T2]]], *Qs],
    ) -> tuple[T1 | None, T2 | None]: ...
    @overload
    def into_inner(
        self: Single[AnyOf[tuple[T1, T2, T3]], *Qs],
    ) -> tuple[T1 | None, T2 | None, T3 | None]: ...
    @overload
    def into_inner(
        self: Single[tuple[Entity, AnyOf[tuple[T1, T2]]], *Qs],
    ) -> tuple[Entity, tuple[T1 | None, T2 | None]]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, T2, AnyOf[tuple[T3, T4]]], *Qs],
    ) -> tuple[T1, T2, tuple[T3 | None, T4 | None]]: ...
    @overload
    def into_inner(self: Single[Has[T], *Qs]) -> bool: ...
    @overload
    def into_inner(self: Single[tuple[Has[T], T1], *Qs]) -> tuple[bool, T1]: ...
    @overload
    def into_inner(self: Single[tuple[T1, Has[T]], *Qs]) -> tuple[T1, bool]: ...
    @overload
    def into_inner(self: Single[Mut[T], *Qs]) -> T: ...
    @overload
    def into_inner(self: Single[T, *Qs]) -> T:
        """Return the borrowed row without copying or consuming the holder."""

    @overload
    def into_inner(self: Single[Mut[T] | None, *Qs]) -> T | None: ...
    @overload
    def into_inner(self: Single[T | None, *Qs]) -> T | None: ...
    @overload
    def into_inner(self: Single[tuple[T1, T2], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def into_inner(self: Single[tuple[Mut[T1], T2], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def into_inner(self: Single[tuple[T1, Mut[T2]], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def into_inner(self: Single[tuple[Mut[T1], Mut[T2]], *Qs]) -> tuple[T1, T2]: ...
    @overload
    def into_inner(self: Single[tuple[T1, T2, T3], *Qs]) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(self: Single[tuple[Mut[T1], T2, T3], *Qs]) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(self: Single[tuple[T1, Mut[T2], T3], *Qs]) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(self: Single[tuple[T1, T2, Mut[T3]], *Qs]) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], Mut[T2], T3], *Qs],
    ) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], T2, Mut[T3]], *Qs],
    ) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, Mut[T2], Mut[T3]], *Qs],
    ) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], Mut[T2], Mut[T3]], *Qs],
    ) -> tuple[T1, T2, T3]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, T2, T3, T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], T2, T3, T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, Mut[T2], T3, T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, T2, Mut[T3], T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, T2, T3, Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], Mut[T2], T3, T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], T2, Mut[T3], T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], T2, T3, Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, Mut[T2], Mut[T3], T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, Mut[T2], T3, Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, T2, Mut[T3], Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], Mut[T2], Mut[T3], T4], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], Mut[T2], T3, Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], T2, Mut[T3], Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[T1, Mut[T2], Mut[T3], Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(
        self: Single[tuple[Mut[T1], Mut[T2], Mut[T3], Mut[T4]], *Qs],
    ) -> tuple[T1, T2, T3, T4]: ...
    @overload
    def into_inner(self: Single[tuple[()], *Qs]) -> tuple[()]: ...
    @overload
    def into_inner(self: Single[tuple[T], *Qs]) -> tuple[T]: ...
    @overload
    def into_inner(self: Single[tuple[Mut[T]], *Qs]) -> tuple[T]: ...
    @overload
    def into_inner(self: Single[QueryParam_T, *Qs]) -> QueryParam_T: ...

V = TypeVar("V")

type In[T] = T
"""Value received from the previous stage of a compound ``pipe`` system."""

class Local(Generic[V]):
    """Per-system local state, persisted across system invocations.

    ``Local[T]`` default-constructs one ``T`` for each system and retains it
    between calls. Attributes of mutable object locals are forwarded directly.
    Use :attr:`current` when reading or replacing the complete value, especially
    for immutable values.

    Every hot reload, partial included, rebuilds the system and constructs the
    local again. Use a ``@resource`` for state that must survive one.

    Example:
        ```python
        def update_stats(stats: Local[Stats]) -> None:
            stats.frame_count += 1

        def count_frames(counter: Local[int]) -> None:
            counter.current += 1
        ```
    """

    def __init__(self, value: V) -> None: ...
    @property
    def value_type(self) -> type[V]: ...
    @property
    def current(self) -> V:
        """The current per-system value."""
    @current.setter
    def current(self, value: V) -> None:
        """Replace the current value with one of the same concrete type."""
    def get(self) -> V:
        """Explicit alias for reading :attr:`current`."""
    def set(self, value: V) -> None:
        """Explicit alias for replacing :attr:`current`."""
    def __getattr__(self, name: str) -> Any:
        """Forward unknown attribute reads to the current value."""
    def __setattr__(self, name: str, value: Any) -> None:
        """Forward unknown attribute writes to the current value."""

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

    Example - Nested hierarchy:
        ```python
        commands.spawn(Transform()).with_children(lambda parent:
            parent.spawn(Transform()).with_children(lambda p2:
                p2.spawn(Mesh3d(grandchild_mesh))
            )
        )
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
        def check_state(current: Res[State[GameState]]) -> None:
            if current.get() == GameState.MENU:
                print("In menu")

    Note:
        Install the machine with ``app.init_state(T)`` or
        ``app.insert_state(T.MEMBER)`` and read the
        current state with ``world.resource(State[T])`` or a
        ``Res[State[T]]`` system parameter. Calling ``State[T](...)`` raises TypeError.
    """
    def __init__(self, initial_state: StateType) -> None:
        """Create a State resource with an initial state value.

        Usually created via app.init_state() or app.insert_state().
        """

    def get(self) -> StateType:
        """Get the current state value."""

    def state_type(self) -> type[StateType]:
        """The @state enum this machine is for.

        Rust reads the machine's type parameter statically; Python carries it at
        runtime, so generic callers need a way to ask.
        """

    def __eq__(self, other: object) -> bool: ...

class NextState(Generic[StateType], Resource):
    """Pending state transition resource.

    Queue state transitions using set(). The transition will be applied
    by the StateTransition schedule (or manually via Commands).

    Example:
        def start_game(next_state: ResMut[NextState[GameState]]) -> None:
            next_state.set(GameState.IN_GAME)

    Note:
        Queue a transition with
        ``world.resource(NextState[T]).set(T.MEMBER)`` or declare
        ``next_state: ResMut[NextState[T]]`` in a system.
        Calling ``NextState[T](...)`` raises TypeError.
    """
    def set(self, state: StateType) -> None:
        """Queue a state transition."""

    def state_type(self) -> type[StateType]:
        """The @state enum this machine is for.

        Rust reads the machine's type parameter statically; Python carries it at
        runtime, so generic callers need a way to ask.
        """

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

def in_state(state: StateType) -> Callable[[Res[State[StateType]]], bool]:
    """Create a run condition that checks if current state matches target state.

    Args:
        state: The target state to check for

    Returns:
        A run condition function for `SystemConfig.run_if`

    Example:
        app.add_systems(Update, system(menu_system).run_if(in_state(GameState.MENU)))
    """

# Schedule label types (internal - returned by OnEnter/OnExit/OnTransition functions)
class OnEnterSchedule:
    """Schedule label for systems that run when entering a state.

    This is the internal type returned by OnEnter(). Users typically don't
    need to reference this type directly.
    """
    def __eq__(self, other: object) -> bool: ...

class OnExitSchedule:
    """Schedule label for systems that run when exiting a state.

    This is the internal type returned by OnExit(). Users typically don't
    need to reference this type directly.
    """
    def __eq__(self, other: object) -> bool: ...

class OnTransitionSchedule:
    """Schedule label for systems that run during state transitions.

    This is the internal type returned by OnTransition(). Users typically don't
    need to reference this type directly.
    """
    def __eq__(self, other: object) -> bool: ...

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

def OnTransition(exited: StateType, entered: StateType) -> OnTransitionSchedule:
    """Create a schedule label for systems that run on a specific state transition.

    Example:
        app.add_systems(OnTransition(GameState.MENU, GameState.IN_GAME), start_game)
    """

# Components for automatic entity lifecycle management
class DespawnOnExit(Component, Generic[StateType]):
    """Component that marks an entity to be despawned when exiting a state.

    Example:
        commands.spawn(Transform(), DespawnOnExit(GameState.MENU))
    """
    def __init__(self, state: StateType) -> None: ...
    def state_value(self) -> StateType:
        """Get the state value this component is associated with."""

class DespawnOnEnter(Component, Generic[StateType]):
    """Component that marks an entity to be despawned when entering a state.

    Example:
        commands.spawn(Transform(), DespawnOnEnter(GameState.PAUSE_MENU))
    """
    def __init__(self, state: StateType) -> None: ...
    def state_value(self) -> StateType:
        """Get the state value this component is associated with."""

class ChildOf(Component):
    """Relationship component indicating the parent entity.

    Used to build entity hierarchies. Spawn with ChildOf(parent_entity)
    to make an entity a child of the parent.
    """
    def __init__(self, value: Entity) -> None: ...
    @property
    def value(self) -> Entity:
        """The related parent entity.

        Read-only, unlike Bevy's public tuple field: the relationship hooks
        that keep `Children` in step run on insert only. Insert a new
        `ChildOf` to reparent.
        """
    def parent(self) -> Entity:
        """Get the parent entity."""
    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...

class Children(Component):
    """Auto-managed list of child entities (read-only).

    Cannot be created from Python: it is automatically managed by Bevy
    when ChildOf components are added. Query it to iterate over children.
    """
    def entities(self) -> list[Entity]:
        """Get all child entities as a list."""
    def len(self) -> int: ...
    def __len__(self) -> int: ...
    def is_empty(self) -> bool: ...
    def __iter__(self) -> Iterator[Entity]: ...
    def __getitem__(self, index: int) -> Entity: ...

@overload
def chain(*systems: SystemChainItem) -> ChainedSystems: ...
@overload
def chain(*sets: SystemSetChainItem) -> ChainedSystemSets:
    """Chain homogeneous systems or system sets in execution order."""

class MessageId:
    """A Bevy-style identifier for a sent message.

    IDs increase in write order within one message type and World. Equality and
    hashing include the message type because Python erases Bevy's generic
    ``MessageId[M]`` type; ordering is supported only between IDs for the same
    message type. Custom types use their registered qualified name, including
    across reloads; later class metadata edits do not change that identity.
    Native types use Bevy's type identity. Bevy's ``caller: MaybeLocation`` field
    is omitted: its payload is disabled without ``track_location``, and PyBevy
    does not expose ``MaybeLocation``.

    Example:
        ```python
        first = writer.write(Ping(1))
        second = writer.write(Ping(2))
        assert second.id == first.id + 1
        assert first < second
        ```
    """

    @property
    def id(self) -> int:
        """Order in which this message was written to its World and type."""

    def __eq__(self, other: object) -> bool: ...
    def __lt__(self, other: MessageId) -> bool: ...
    def __le__(self, other: MessageId) -> bool: ...
    def __gt__(self, other: MessageId) -> bool: ...
    def __ge__(self, other: MessageId) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...
