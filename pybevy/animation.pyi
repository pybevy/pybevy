from collections.abc import Iterable
from typing import ClassVar, Literal
from uuid import UUID

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.ecs import Component, Entity

class AnimationEvent:
    """A simple animation event that can be triggered at specific timestamps.

    **LIMITATION**: Due to Python/Rust trait constraints, this is a simplified version
    of Bevy's AnimationEvent system. These events are for metadata/tracking purposes.
    They do NOT automatically trigger through Bevy's observer system.

    For event handling, users should:
    1. Store AnimationEvent metadata alongside animations
    2. Poll ActiveAnimation.seek_time() in systems
    3. Manually trigger logic when seek_time crosses event timestamps

    Example:
        >>> event = AnimationEvent("footstep", data='{"volume": 0.8}')
        >>> # Store this with your animation metadata
        >>> # In a system, check: if prev_time < event_time <= curr_time: handle_event()
    """

    name: str
    data: str | None

    def __init__(self, name: str, data: str | None = None) -> None:
        """Create an animation event.

        Args:
            name: Event identifier (e.g., "footstep", "weapon_swing")
            data: Optional JSON string for event data
        """

    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...


class AnimationEventData:
    """Pairs an AnimationEvent with a timestamp for timeline placement.

    **LIMITATION**: This is metadata only - not integrated with Bevy's event system.
    Users must manually poll and trigger these events based on seek_time().
    """

    time: float
    event: AnimationEvent

    def __init__(self, time: float, event: AnimationEvent) -> None:
        """Create event data with timestamp.

        Args:
            time: Time in seconds when event should occur
            event: The event to trigger
        """

    def __repr__(self) -> str: ...

class AnimationPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class AnimatableCurve:
    """A curve that can be used to animate a property."""

class AnimatableKeyframeCurve:
    """A keyframe-based animation curve."""

class WeightsCurve:
    """A curve for animating blend shape weights."""

class AnimatedField:
    """A field that can be animated."""

class AnimationNodeType:
    class Clip(AnimationNodeType):
        __match_args__: ClassVar[tuple[Literal["handle"]]]
        handle: Handle[AnimationClip]
        def __init__(self, handle: Handle[AnimationClip]) -> None: ...

    class Blend(AnimationNodeType):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Add(AnimationNodeType):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    def __repr__(self) -> str: ...

class AnimationNodeIndex:
    def __init__(self, index: int) -> None: ...
    def index(self) -> int: ...

class RepeatAnimation:
    class Never(RepeatAnimation):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Count(RepeatAnimation):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class Forever(RepeatAnimation):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    def __repr__(self) -> str: ...

class ActiveAnimation:
    @property
    def is_finished(self) -> bool: ...
    def replay(self) -> None: ...
    @property
    def weight(self) -> float: ...
    def set_weight(self, weight: float) -> ActiveAnimation: ...
    def pause(self) -> ActiveAnimation: ...
    def resume(self) -> ActiveAnimation: ...
    @property
    def is_paused(self) -> bool: ...
    def set_repeat(self, repeat: RepeatAnimation) -> ActiveAnimation: ...
    def repeat(self) -> ActiveAnimation: ...
    def repeat_mode(self) -> RepeatAnimation: ...
    @property
    def completions(self) -> int: ...
    @property
    def is_playback_reversed(self) -> bool: ...
    @property
    def speed(self) -> float: ...
    def set_speed(self, speed: float) -> ActiveAnimation: ...
    @property
    def elapsed(self) -> float: ...
    @property
    def last_seek_time(self) -> float | None: ...
    @property
    def just_completed(self) -> bool: ...
    @property
    def seek_time(self) -> float: ...
    def set_seek_time(self, seek_time: float) -> ActiveAnimation: ...
    def seek_to(self, seek_time: float) -> ActiveAnimation: ...
    def rewind(self) -> ActiveAnimation:
        """Rewind this animation to the start (equivalent to seek_to(0.0))."""

class AnimationPlayer(Component):
    def __init__(self) -> None: ...
    def play(self, animation: AnimationNodeIndex) -> ActiveAnimation: ...
    def start(self, animation: AnimationNodeIndex) -> ActiveAnimation: ...
    def stop(self, animation: AnimationNodeIndex) -> AnimationPlayer: ...
    def stop_all(self) -> AnimationPlayer: ...
    def is_playing_animation(self, animation: AnimationNodeIndex) -> bool: ...
    @property
    def all_finished(self) -> bool: ...
    @property
    def all_paused(self) -> bool: ...
    def pause_all(self) -> AnimationPlayer: ...
    def resume_all(self) -> AnimationPlayer: ...
    def seek_all_by(self, amount: float) -> AnimationPlayer: ...
    def animation(self, animation: AnimationNodeIndex) -> ActiveAnimation | None:
        """Get a read-only reference to a playing animation without starting it.

        Returns None if the animation is not currently playing.

        Args:
            animation: The animation node to query

        Returns:
            ActiveAnimation if playing, None otherwise
        """
    def animation_mut(self, animation: AnimationNodeIndex) -> ActiveAnimation | None:
        """Get mutable reference to a specific playing animation.

        Returns None if the animation is not currently playing.

        Args:
            animation: The animation node to query

        Returns:
            ActiveAnimation if playing, None otherwise
        """
    def rewind_all(self) -> AnimationPlayer:
        """Rewind all playing animations to their start."""
    def adjust_speeds(self, factor: float) -> AnimationPlayer:
        """Multiply all animation speeds by the given factor.

        Useful for slow-motion (factor < 1.0) or fast-forward (factor > 1.0) effects.

        Args:
            factor: Speed multiplier (e.g., 0.5 for half speed, 2.0 for double speed)

        Returns:
            Self for method chaining
        """

class AnimationTransitions(Component):
    """Manages fade-out of animation blend factors, allowing for smooth transitions
    between animations.

    To use this component, place it on the same entity as the AnimationPlayer and
    AnimationGraphHandle. It'll take responsibility for adjusting the weight on the
    ActiveAnimation in order to fade out animations smoothly.

    When using an AnimationTransitions component, you should play all animations
    through the AnimationTransitions.play() method, rather than by directly
    manipulating the AnimationPlayer. Playing animations through the AnimationPlayer
    directly will cause the AnimationTransitions component to get confused about which
    animation is the "main" animation, and transitions will usually be incorrect as a result.
    """

    def __init__(self) -> None:
        """Creates a new AnimationTransitions component, ready to be added to
        an entity with an AnimationPlayer."""

    def play(
        self,
        player: AnimationPlayer,
        new_animation: AnimationNodeIndex,
        transition_duration: float,
    ) -> ActiveAnimation:
        """Plays a new animation on the given AnimationPlayer, fading out any
        existing animations that were already playing over the transition_duration.

        Pass 0.0 for transition_duration to instantly switch to a new animation,
        avoiding any transition.

        Args:
            player: The AnimationPlayer component to play the animation on
            new_animation: The index of the animation to play
            transition_duration: Duration in seconds for the cross-fade transition

        Returns:
            ActiveAnimation reference for the newly started animation
        """

    def get_main_animation(self) -> AnimationNodeIndex | None:
        """Obtain the currently playing main animation.

        Returns:
            The AnimationNodeIndex of the main animation, or None if no animation is playing
        """

class AnimationGraph(Asset):
    def __init__(self) -> None: ...
    @staticmethod
    def from_clip(
        clip: Handle[AnimationClip],
    ) -> tuple[AnimationGraph, AnimationNodeIndex]: ...
    @staticmethod
    def from_clips(
        clips: Iterable[Handle[AnimationClip]],
    ) -> tuple[AnimationGraph, list[AnimationNodeIndex]]: ...
    @property
    def root(self) -> AnimationNodeIndex: ...
    def add_blend(
        self, weight: float, parent: AnimationNodeIndex
    ) -> AnimationNodeIndex: ...
    def add_clip(
        self, clip: Handle[AnimationClip], weight: float, parent: AnimationNodeIndex
    ) -> AnimationNodeIndex: ...
    def add_edge(self, from_: AnimationNodeIndex, to: AnimationNodeIndex) -> None: ...
    def remove_edge(
        self, from_: AnimationNodeIndex, to: AnimationNodeIndex
    ) -> bool: ...
    def get(self, animation: AnimationNodeIndex) -> AnimationGraphNode | None:
        """Read-only view of a node; its setters raise."""

    def get_mut(self, animation: AnimationNodeIndex) -> AnimationGraphNode | None:
        """Writable view of a node."""

    def nodes(self) -> list[AnimationNodeIndex]: ...

class AnimationGraphNode:
    @property
    def node_type(self) -> AnimationNodeType: ...
    @property
    def mask(self) -> int: ...
    @mask.setter
    def mask(self, value: int) -> None: ...
    @property
    def weight(self) -> float: ...
    @weight.setter
    def weight(self, value: float) -> None: ...
    def add_mask(self, mask: int) -> AnimationGraphNode:
        """Masks out the mask groups specified by the given `mask` bitfield.

        A 1 in bit position N causes this function to mask out mask group N, and
        thus neither this node nor its descendants will animate any animation
        targets that belong to group N.
        """
    def remove_mask(self, mask: int) -> AnimationGraphNode:
        """Unmasks the mask groups specified by the given `mask` bitfield.

        A 1 in bit position N causes this function to unmask mask group N, and
        thus this node and its descendants will be allowed to animate animation
        targets that belong to group N, unless another mask masks those targets out.
        """
    def add_mask_group(self, group: int) -> AnimationGraphNode:
        """Masks out the single mask group specified by `group`.

        After calling this function, neither this node nor its descendants will
        animate any animation targets that belong to the given `group`.
        """
    def remove_mask_group(self, group: int) -> AnimationGraphNode:
        """Unmasks the single mask group specified by `group`.

        After calling this function, this node and its descendants will be
        allowed to animate animation targets that belong to the given `group`,
        unless another mask masks those targets out.
        """

class AnimationGraphHandle(Component):
    """Component that references an AnimationGraph asset handle.

    Used to attach an animation graph to an entity with AnimationPlayer.
    """

    def __init__(self, value: Handle[AnimationGraph]) -> None: ...
    @property
    def value(self) -> Handle[AnimationGraph]:
        """Get the underlying asset handle."""

class AnimationTargetId(Component):
    def __init__(self, uuid: UUID) -> None: ...
    @staticmethod
    def from_name(name: str) -> AnimationTargetId:
        """Creates a new AnimationTargetId by hashing a single name.

        Args:
            name: The name to hash (e.g., bone name like "Spine" or "Arm")
        """
    @staticmethod
    def from_names(names: list[str]) -> AnimationTargetId:
        """Creates a new AnimationTargetId by hashing a list of names.

        Typically, this will be the path from the animation root to the
        animation target (e.g., bone) that is to be animated.

        Args:
            names: List of names forming the path (e.g., ["Root", "Spine", "Arm"])
        """
    @property
    def value(self) -> UUID:
        """The wrapped uuid.UUID (Bevy's newtype payload)."""

    def __hash__(self) -> int: ...

class AnimationCurve:
    """Base class for animation curves.

    **LIMITATION**: PyBevy does not wrap bevy's curve constructors or
    evaluation methods (domain, sample, etc.) yet.

    This class exists for type compatibility but cannot be constructed or used
    directly. Use AnimationPlayer for animation playback.
    """

class VariableCurve(AnimationCurve):
    """Wrapper for Bevy's VariableCurve.

    **LIMITATION**: PyBevy does not wrap bevy's curve constructors or
    evaluation methods, so this class serves as a type marker for
    curves returned by AnimationClip.curves() but has no accessible methods.

    For animation playback, use AnimationPlayer - the full animation system
    works internally, just without external curve introspection.

    Note: Cannot be constructed from Python. Instances are only returned by
    AnimationClip.curves() and AnimationClip.curves_for_target().
    """

class AnimationClip(Asset):
    """A set of curves keyed by animation target.

    Clips are playable and inspectable, but not authorable: the curve-adding
    methods below take a VariableCurve, and PyBevy does not wrap bevy's curve
    constructors yet. Build clips by loading them (glTF), and drive playback
    with AnimationPlayer.
    """
    def __init__(self) -> None: ...
    def duration(self) -> float: ...
    def set_duration(self, duration_sec: float) -> None: ...
    def curves(self) -> dict[AnimationTargetId, list[VariableCurve]]: ...
    def curves_for_target(
        self, target_id: AnimationTargetId
    ) -> list[VariableCurve] | None: ...
    def add_curve_to_target(
        self, target_id: AnimationTargetId, curve: VariableCurve
    ) -> None: ...
    def add_variable_curve_to_target(
        self, target_id: AnimationTargetId, variable_curve: VariableCurve
    ) -> None: ...
    # def add_event(self, time: float, event: str) -> None: ...
    # def add_event_to_target(
    #     self, target_id: AnimationTargetId, time: float, event: str
    # ) -> None: ...

class AnimatedBy(Component):
    """Component indicating which AnimationPlayer entity drives this entity's animations.

    Added automatically when an entity is targeted by an AnimationPlayer.
    Can also be added manually to link an entity to a specific player.
    """

    def __init__(self, entity: Entity) -> None:
        """Create an AnimatedBy component pointing to an AnimationPlayer entity."""

    @property
    def entity(self) -> Entity:
        """The entity containing the AnimationPlayer."""
