from pybevy.app import Plugin
from pybevy.assets import Asset, Handle
from pybevy.ecs import Component

class HanabiPlugin(Plugin):
    """Enables GPU particle effects (bevy_hanabi).

    Add to your app before spawning any ParticleEffect:

        app.add_plugins(HanabiPlugin())
    """

    def __init__(self) -> None: ...

class EffectAsset(Asset):
    """A GPU particle effect definition.

    Built via presets; add the returned asset with app.add_asset() /
    assets.add(), then spawn a ParticleEffect with the handle.

    Examples:
        effect = EffectAsset.fountain(rate=200.0, colors=[(0.2, 0.6, 1.0, 1.0), (0.0, 0.0, 1.0, 0.0)])
        boom = EffectAsset.burst(count=800.0, speed=(3.0, 10.0))
    """

    @staticmethod
    def fountain(
        capacity: int = 4096,
        rate: float = 100.0,
        lifetime: float = 1.2,
        speed: tuple[float, float] = (3.0, 5.0),
        spread: float = 0.25,
        radius: float = 0.1,
        gravity: float = -6.0,
        size: tuple[float, float] = (0.15, 0.03),
        colors: list[tuple[float, float, float, float]] = ...,
    ) -> EffectAsset:
        """Continuous upward stream of particles.

        Args:
            capacity: Max concurrent particles.
            rate: Particles spawned per second.
            lifetime: Mean particle lifetime in seconds (randomized ±20%).
            speed: (min, max) upward speed in world units/second.
            spread: Sideways velocity as a fraction of max speed.
            radius: Emission sphere radius around the entity.
            gravity: Y acceleration applied every frame (negative = down).
            size: (start, end) particle size over lifetime, world units.
            colors: RGBA gradient stops (0..1), evenly spaced over lifetime.
        """

    @staticmethod
    def burst(
        capacity: int = 2048,
        count: float = 500.0,
        lifetime: float = 0.9,
        speed: tuple[float, float] = (2.0, 8.0),
        radius: float = 0.2,
        gravity: float = -3.0,
        size: tuple[float, float] = (0.12, 0.02),
        colors: list[tuple[float, float, float, float]] = ...,
    ) -> EffectAsset:
        """One-shot radial explosion of particles.

        Args:
            capacity: Max concurrent particles.
            count: Particles emitted in the single burst.
            lifetime: Mean particle lifetime in seconds (randomized ±30%).
            speed: (min, max) radial speed in world units/second.
            radius: Emission sphere radius around the entity.
            gravity: Y acceleration applied every frame (negative = down).
            size: (start, end) particle size over lifetime, world units.
            colors: RGBA gradient stops (0..1), evenly spaced over lifetime.
        """

    @property
    def name(self) -> str:
        """Display name of the effect."""

    @property
    def capacity(self) -> int:
        """Maximum number of concurrent particles."""

class ParticleEffect(Component):
    """Spawns an instance of an EffectAsset at this entity's Transform.

    Examples:
        handle = assets.add(EffectAsset.fountain())
        commands.spawn(ParticleEffect(handle), Transform.IDENTITY)
    """

    def __init__(self, effect: Handle) -> None: ...
    effect: Handle
    """Handle of the EffectAsset to instantiate."""
