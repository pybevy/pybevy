"""Decorated callables accepted by the scheduler: systems, conditions, observers."""

from __future__ import annotations

import functools
import inspect
import sys
import types
from collections.abc import Callable
from typing import Protocol, Self, cast


class _NativeSystemConfig(Protocol):
    def in_set(self, set: object) -> object: ...
    def before(self, target: object) -> object: ...
    def after(self, target: object) -> object: ...
    def run_if(self, condition: object) -> object: ...
    def pipe(self, target: object) -> object: ...


class _NativeEcsModule(Protocol):
    SystemConfig: Callable[[object], _NativeSystemConfig]
    _unwrap_decorated: Callable[[object, str, str], Callable[..., object]]
    __name__: str


_native: _NativeEcsModule | None = None

# 3.14 annotations are lazy (PEP 649): copy `__annotate__`, never the evaluated dict.
_WRAPPER_ASSIGNMENTS = tuple(
    name
    for name in functools.WRAPPER_ASSIGNMENTS
    if sys.version_info < (3, 14) or name != "__annotations__"
)


def _native_module() -> _NativeEcsModule:
    if _native is None:
        raise RuntimeError("pybevy.ecs decorators used before pybevy.ecs was imported")
    return _native


def _describe(value: object) -> str:
    if isinstance(value, _DecoratedCallable):
        return f"a callable already decorated with @{value._decorator}"
    return f"an object of type {type(value).__name__}"


class _DecoratedCallable:
    """A callable marked for one scheduler role; forwards calls and metadata."""

    # No class-level annotations: instances must not inherit a class __annotations__.
    _decorator = ""

    def __class_getitem__(cls, params: object) -> types.GenericAlias:
        # Runtime-evaluated hints such as `System[[Res[Time]], None]` must resolve.
        return types.GenericAlias(cls, params)

    def __init__(self, func: Callable[..., object]) -> None:
        if isinstance(func, _DecoratedCallable) or not callable(func):
            raise TypeError(f"@{self._decorator} cannot wrap {_describe(func)}")
        self.__wrapped__: Callable[..., object] = func
        functools.update_wrapper(self, func, assigned=_WRAPPER_ASSIGNMENTS, updated=())

    @classmethod
    def _wrap(cls, func: Callable[..., object]) -> Self:
        if type(func) is cls:
            return cast(Self, func)
        return cls(func)

    def __call__(self, *args: object, **kwargs: object) -> object:
        return self.__wrapped__(*args, **kwargs)

    def __getattr__(self, name: str) -> object:
        # Resolved lazily so decorating never evaluates PEP 649 annotations.
        if name == "__annotations__":
            return self.__wrapped__.__annotations__
        raise AttributeError(
            f"{type(self).__name__!r} object has no attribute {name!r}", name=name, obj=self
        )

    def __get__(self, instance: object, owner: type | None = None) -> Self:
        if instance is None or not inspect.isfunction(self.__wrapped__):
            return self
        return type(self)(self.__wrapped__.__get__(instance, owner))

    def __repr__(self) -> str:
        module = getattr(self, "__module__", None)
        qualname = getattr(self, "__qualname__", None) or repr(self.__wrapped__)
        name = f"{module}.{qualname}" if isinstance(module, str) else qualname
        return f"<{type(self).__name__} {name}>"


class System(_DecoratedCallable):
    """A function decorated with ``@system``, ready for ``add_systems``."""

    _decorator = "system"

    def _config(self) -> _NativeSystemConfig:
        return _native_module().SystemConfig(self)

    def in_set(self, set: object) -> object:
        return self._config().in_set(set)

    def before(self, target: object) -> object:
        return self._config().before(target)

    def after(self, target: object) -> object:
        return self._config().after(target)

    def run_if(self, condition: object) -> object:
        return self._config().run_if(condition)

    def pipe(self, target: object) -> object:
        return self._config().pipe(target)


class SystemCondition(_DecoratedCallable):
    """A predicate decorated with ``@condition``, ready for ``run_if``."""

    _decorator = "condition"

    def and_(self, *others: object) -> SystemCondition:
        return combine("and", (self, *others), "and_()")

    def or_(self, *others: object) -> SystemCondition:
        return combine("or", (self, *others), "or_()")

    def not_(self) -> SystemCondition:
        return combine("not", (self,), "not_()")


class ObserverSystem(_DecoratedCallable):
    """A function decorated with ``@observer``, ready for ``add_observer``."""

    _decorator = "observer"


def system(func: Callable[..., object], /) -> System:
    """Mark ``func`` as a system for ``add_systems`` and one-shot runs."""
    return System._wrap(func)


def condition(func: Callable[..., object], /) -> SystemCondition:
    """Mark ``func`` as a run condition for ``run_if``."""
    return SystemCondition._wrap(func)


def observer(func: Callable[..., object], /) -> ObserverSystem:
    """Mark ``func`` as an observer for ``add_observer`` and ``observe``."""
    return ObserverSystem._wrap(func)


def combine(operator: str, operands: tuple[object, ...], entry: str) -> SystemCondition:
    """Build a condition expression the scheduler lowers to Bevy combinators."""
    unwrap = _native_module()._unwrap_decorated
    for operand in operands:
        unwrap(operand, "condition", entry)
    conditions = cast(tuple[SystemCondition, ...], operands)

    if operator == "and":

        def combined() -> bool:
            return all(c() for c in conditions)
    elif operator == "or":

        def combined() -> bool:
            return any(c() for c in conditions)
    else:

        def combined() -> bool:
            return not conditions[0]()

    combined.__pybevy_condition_expr__ = (operator, conditions)  # type: ignore[attr-defined]
    return SystemCondition(combined)


def install_callable_api(native: _NativeEcsModule) -> None:
    """Install the decorators and their classes on the native module."""
    global _native
    _native = native
    # Public annotations spell the stub contract; the implementation stays untyped here.
    config_target = "SystemSet | SystemSetEnum | SystemFn | SystemConfig"
    public_annotations: dict[Callable[..., object], dict[str, str]] = {
        System.in_set: {"set": "SystemSet | SystemSetEnum", "return": "SystemConfig"},
        System.before: {"target": config_target, "return": "SystemConfig"},
        System.after: {"target": config_target, "return": "SystemConfig"},
        System.run_if: {"condition": "ConditionFn", "return": "SystemConfig"},
        System.pipe: {"target": "SystemFn", "return": "SystemConfig"},
        SystemCondition.and_: {"others": "ConditionFn", "return": "SystemCondition[..., bool]"},
        SystemCondition.or_: {"others": "ConditionFn", "return": "SystemCondition[..., bool]"},
        SystemCondition.not_: {"return": "SystemCondition[..., bool]"},
        system: {"func": "Callable[_P, _R]", "return": "System[_P, _R]"},
        condition: {"func": "Callable[_P, _R]", "return": "SystemCondition[_P, _R]"},
        observer: {"func": "Callable[_P, _R]", "return": "ObserverSystem[_P, _R]"},
    }
    for function, hints in public_annotations.items():
        function.__annotations__ = hints
    for value in (System, SystemCondition, ObserverSystem, system, condition, observer):
        value.__module__ = native.__name__
        setattr(native, value.__name__, value)
