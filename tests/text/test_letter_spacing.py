"""Tests for the LetterSpacing component."""

import pytest

from pybevy.ecs import Commands, Component, Query
from pybevy.prelude import App, MinimalPlugins
from pybevy.text import LetterSpacing


def test_default_is_px_zero() -> None:
    ls = LetterSpacing()
    assert isinstance(ls, Component)
    assert ls == LetterSpacing.Px(0.0)
    assert ls.value == pytest.approx(0.0)
    assert repr(ls) == "LetterSpacing.Px(0)"


def test_px() -> None:
    ls = LetterSpacing.Px(4.0)
    assert ls == LetterSpacing.Px(4.0)
    assert ls != LetterSpacing.Rem(4.0)
    assert ls.value == pytest.approx(4.0)


def test_rem() -> None:
    ls = LetterSpacing.Rem(0.5)
    assert ls == LetterSpacing.Rem(0.5)
    assert ls != LetterSpacing.Px(0.5)
    assert ls.value == pytest.approx(0.5)
    assert repr(ls) == "LetterSpacing.Rem(0.5)"


def test_spawn_and_query() -> None:
    results: list[int] = []

    def setup(commands: Commands) -> None:
        commands.spawn(LetterSpacing.Px(2.0))
        commands.spawn(LetterSpacing.Rem(1.0))

    def query_system(query: Query[LetterSpacing]) -> None:
        results.append(sum(1 for _ in query))

    App().add_plugins(MinimalPlugins)._run_systems_once(setup, query_system)

    assert results == [2]
