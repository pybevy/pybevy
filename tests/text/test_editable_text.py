"""Tests for the EditableText component."""

from datetime import timedelta

import pytest

from pybevy.ecs import Commands, Component, Mut, Query
from pybevy.prelude import App, MinimalPlugins
from pybevy.text import EditableText


def test_defaults() -> None:
    et = EditableText()
    assert isinstance(et, Component)
    assert et.value == ""
    assert et.max_characters is None
    assert et.allow_newlines is False
    assert et.cursor_width == pytest.approx(0.2)
    assert et.cursor_blink_period == timedelta(seconds=1)
    assert et.visible_lines == pytest.approx(1.0)
    assert et.visible_width is None


def test_initial_text() -> None:
    et = EditableText("hello world")
    assert et.value == "hello world"


def test_config() -> None:
    et = EditableText(
        "abc",
        max_characters=10,
        allow_newlines=True,
        cursor_width=0.5,
        cursor_blink_period=2.5,
        visible_lines=3.0,
        visible_width=20.0,
    )
    assert et.value == "abc"
    assert et.max_characters == 10
    assert et.allow_newlines is True
    assert et.cursor_width == pytest.approx(0.5)
    assert et.cursor_blink_period.total_seconds() == pytest.approx(2.5)
    assert et.visible_lines == pytest.approx(3.0)
    assert et.visible_width == pytest.approx(20.0)


def test_setters() -> None:
    et = EditableText()
    et.max_characters = 5
    et.allow_newlines = True
    et.cursor_width = 0.3
    et.cursor_blink_period = timedelta(milliseconds=500)
    et.visible_lines = 4.0
    et.visible_width = 8.0
    assert et.max_characters == 5
    assert et.allow_newlines is True
    assert et.cursor_width == pytest.approx(0.3)
    assert et.cursor_blink_period.total_seconds() == pytest.approx(0.5)
    assert et.visible_lines == pytest.approx(4.0)
    assert et.visible_width == pytest.approx(8.0)


def test_max_characters_clearable() -> None:
    et = EditableText("x", max_characters=3)
    assert et.max_characters == 3
    et.max_characters = None
    assert et.max_characters is None


def test_spawn_and_query() -> None:
    results: list[int] = []

    def setup(commands: Commands) -> None:
        commands.spawn(EditableText("one"))
        commands.spawn(EditableText("two", max_characters=5))

    def query_system(query: Query[EditableText]) -> None:
        results.append(sum(1 for _ in query))

    App().add_plugins(MinimalPlugins)._run_systems_once(setup, query_system)

    assert results == [2]


def test_mutation_persists() -> None:
    results: list[bool] = []

    def setup(commands: Commands) -> None:
        commands.spawn(EditableText("hi", allow_newlines=False))

    def mutate(query: Query[Mut[EditableText]]) -> None:
        for et in query:
            et.allow_newlines = True

    def verify(query: Query[EditableText]) -> None:
        for et in query:
            results.append(et.allow_newlines)

    App().add_plugins(MinimalPlugins)._run_systems_once(setup, mutate, verify)

    assert results == [True]
