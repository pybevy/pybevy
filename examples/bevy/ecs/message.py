"""PyBevy adaptation of Bevy 0.19's message example.

It demonstrates how to control system ordering so that messages are processed
in a specific order. It simulates a damage over time effect with armor.

The Rust example uses MessageMutator[DealDamage] to read, write, and mutate one
channel. PyBevy does not expose MessageMutator yet, so this adaptation uses
distinct attempted-damage and approved-damage channels instead.
"""

from dataclasses import dataclass

from pybevy.prelude import *


@dataclass
class AttemptDamage(Message):
    """Message sent when something attempts to deal damage."""

    amount: int


@dataclass
class DealDamage(Message):
    """Damage approved after armor processing."""

    amount: int


@dataclass
class DamageReceived(Message):
    """Message sent when an entity receives damage."""


@dataclass
class ArmorBlockedDamage(Message):
    """Message sent when an entity blocks damage with armor."""


@resource
class DamageTimer(Resource):
    """Timer used to determine when to deal damage."""

    def __init__(self):
        self.timer: Timer = Timer(1.0, TimerMode.REPEATING)


def deal_damage_over_time(
    time: Res[Time],
    state: ResMut[DamageTimer],
    attempt_writer: MessageWriter[AttemptDamage],
) -> None:
    """Read DamageTimer, update it, then attempt damage if finished."""
    state.timer.tick(time.delta_secs())
    if state.timer.just_finished():
        attempt_writer.write(AttemptDamage(amount=10))


def apply_armor_to_damage(
    dmg_reader: MessageReader[AttemptDamage],
    dmg_writer: MessageWriter[DealDamage],
    armor_writer: MessageWriter[ArmorBlockedDamage],
) -> None:
    """Apply armor to damage messages.

    AttemptDamage and DealDamage are separate channels so this system has no
    same-channel reader/writer conflict.
    """
    for message in dmg_reader:
        reduced_amount = message.amount - 1
        if reduced_amount <= 0:
            armor_writer.write(ArmorBlockedDamage())
        else:
            dmg_writer.write(DealDamage(amount=reduced_amount))


def apply_damage_to_health(
    deal_damage_reader: MessageReader[DealDamage],
    damage_received_writer: MessageWriter[DamageReceived],
) -> None:
    """Read DealDamage messages and send DamageReceived if amount is non-zero."""
    for deal_damage in deal_damage_reader:
        print(f"Applying {deal_damage.amount} damage")
        if deal_damage.amount > 0:
            damage_received_writer.write(DamageReceived())


def play_damage_received_sound(damage_received_reader: MessageReader[DamageReceived]) -> None:
    """Play sound when damage is received."""
    for _ in damage_received_reader:
        print("Playing a sound.")


def play_damage_received_particle_effect(
    damage_received_reader: MessageReader[DamageReceived],
) -> None:
    """Play particle effect when damage is received.

    Note: Both this and the sound system receive the same DamageReceived messages.
    Any number of systems can receive the same message type.
    """
    for _ in damage_received_reader:
        print("Playing particle effect.")


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        # Messages must be added to the app before they can be used
        .add_message(AttemptDamage)
        .add_message(DealDamage)
        .add_message(ArmorBlockedDamage)
        .add_message(DamageReceived)
        .insert_resource(DamageTimer())
        # Ordering makes the three-stage message pipeline visible in one pass.
        # deal_damage_over_time writes AttemptDamage
        # apply_armor_to_damage reads AttemptDamage and writes DealDamage
        # apply_damage_to_health reads DealDamage
        .add_systems(
            Update,
            chain(
                deal_damage_over_time,
                apply_armor_to_damage,
                apply_damage_to_health,
            ),
        )
        # These readers are unordered relative to the producer chain and may
        # run before it. In that case they process its messages next update.
        .add_systems(
            Update,
            (
                play_damage_received_sound,
                play_damage_received_particle_effect,
            ),
        )
    )


if __name__ == "__main__":
    main().run()
