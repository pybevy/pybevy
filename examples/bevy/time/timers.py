"""Illustrates how Timers can be used both as resources and components.

Demonstrates:
- Using Timer as a component (PrintOnCompletionTimer)
- Using Timer as a resource (Countdown)
- Ticking timers with Time.delta()
- Checking timer states (just_finished, is_finished, fraction)
- Timer modes (Once vs Repeating)
"""

from pybevy.ecs import Res, ResMut
from pybevy.prelude import *


@resource
class Countdown(Resource):
    """Resource holding countdown timers."""

    def __init__(self):
        self.percent_trigger = Timer(4.0, TimerMode.REPEATING)
        self.main_timer = Timer(20.0, TimerMode.ONCE)
        self.entity_timer = Timer(5.0, TimerMode.ONCE)


def print_when_completed(time: Res[Time], countdown_res: ResMut[Countdown]) -> None:
    """Tick the entity timer and print when complete."""
    delta = time.delta_secs()
    countdown_res.entity_timer.tick(delta)
    if countdown_res.entity_timer.just_finished():
        print("Entity timer just finished")


def countdown(time: Res[Time], countdown_res: ResMut[Countdown]) -> None:
    """Control ticking the timer within the countdown resource."""
    delta = time.delta_secs()
    countdown_res.main_timer.tick(delta)

    # Tick the percent trigger timer and check if just finished
    countdown_res.percent_trigger.tick(delta)
    if countdown_res.percent_trigger.just_finished():
        if not countdown_res.main_timer.is_finished():
            # Print the percent complete the main timer is
            percent = countdown_res.main_timer.fraction() * 100.0
            print(f"Timer is {percent:.0f}% complete!")
        else:
            # The timer has finished so we pause the percent output timer
            countdown_res.percent_trigger.pause()
            print("Paused percent trigger timer")


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(Countdown())
        .add_systems(Update, (countdown, print_when_completed))
    )


if __name__ == "__main__":
    main().run()
