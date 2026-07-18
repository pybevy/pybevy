"""Python enum-family support for Bevy system sets."""

from __future__ import annotations

from collections.abc import Callable
from enum import Enum
from typing import Protocol, cast


class _NativeSystemSet(Protocol):
    def __init__(self, name: str) -> None: ...
    def in_set(self, parent: object) -> object: ...
    def before(self, target: object) -> object: ...
    def after(self, target: object) -> object: ...
    def run_if(self, condition: object) -> object: ...


class _NativeEcsModule(Protocol):
    SystemSet: type[_NativeSystemSet]
    SystemSetEnum: type[Enum]
    system_set: Callable[[type[object]], object]
    __name__: str


def install_system_set_api(native: _NativeEcsModule) -> None:
    """Install the Python-aware ``system_set`` decorator on the native module."""
    native_system_set = native.system_set
    native_system_set_type = native.SystemSet

    class SystemSetEnum(Enum):
        """Typed base for an enum whose members are distinct Bevy system sets."""

        def _set(self) -> _NativeSystemSet:
            try:
                return cast(
                    _NativeSystemSet,
                    object.__getattribute__(self, "_pybevy_system_set"),
                )
            except AttributeError as error:
                raise RuntimeError(
                    f"{type(self).__qualname__} must be decorated with @system_set"
                ) from error

        def in_set(self, parent: object) -> object:
            return self._set().in_set(parent)

        def before(self, target: object) -> object:
            return self._set().before(target)

        def after(self, target: object) -> object:
            return self._set().after(target)

        def run_if(self, condition: object) -> object:
            return self._set().run_if(condition)

    def system_set(cls: type[object], /) -> object:
        if not issubclass(cls, Enum):
            return native_system_set(cls)
        if not cls.__members__:
            raise ValueError("a system-set enum must declare at least one member")
        if len(cls.__members__) != len(cls):
            raise ValueError("system-set enum aliases are not supported")

        module = cls.__module__
        if module == "<run_path>" or not isinstance(module, str):
            module = "__main__"
        for member in cls:
            label = native_system_set_type(
                f"{module}.{cls.__qualname__}.{member.name}"
            )
            object.__setattr__(member, "_pybevy_system_set", label)

        if not issubclass(cls, SystemSetEnum):
            for name in ("_set", "in_set", "before", "after", "run_if"):
                if name in cls.__dict__:
                    raise TypeError(
                        f"system-set enum reserves the method name {name!r}"
                    )
                setattr(cls, name, SystemSetEnum.__dict__[name])
        return cls

    SystemSetEnum.__module__ = native.__name__
    SystemSetEnum.__qualname__ = "SystemSetEnum"
    SystemSetEnum.in_set.__annotations__ = {
        "parent": "SystemSet | SystemSetEnum",
        "return": "SystemSetConfig",
    }
    SystemSetEnum.before.__annotations__ = {
        "target": "SystemSet | SystemSetEnum | SystemFn",
        "return": "SystemSetConfig",
    }
    SystemSetEnum.after.__annotations__ = {
        "target": "SystemSet | SystemSetEnum | SystemFn",
        "return": "SystemSetConfig",
    }
    SystemSetEnum.run_if.__annotations__ = {
        "condition": "Callable[..., object]",
        "return": "SystemSetConfig",
    }
    system_set.__module__ = native.__name__
    system_set.__annotations__ = {
        "cls": "type[object]",
        "return": "SystemSet",
    }
    native.SystemSetEnum = SystemSetEnum
    native.system_set = system_set
