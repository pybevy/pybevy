"""Detached Python state for atomic control-plane resource patches."""

from __future__ import annotations

from contextlib import suppress
from copy import deepcopy
from types import BuiltinFunctionType, GetSetDescriptorType, MemberDescriptorType
from typing import cast

from ..ecs import Component, Resource

_State = tuple[dict[str, object] | None, dict[MemberDescriptorType, object]]


class _ResourcePatch:
    def __init__(self, resource: Resource) -> None:
        self._resource = resource
        self._dict: GetSetDescriptorType | None = None
        slots: list[MemberDescriptorType] = []
        for base in type(resource).__mro__:
            descriptors = vars(base)
            # Only the marker base has native storage initialized by this allocator.
            if base not in (Resource, Component, object) and isinstance(
                descriptors.get("__new__"), BuiltinFunctionType
            ):
                raise TypeError("resource has a native constructor")
            if self._dict is None and "__dict__" in descriptors:
                descriptor = descriptors["__dict__"]
                if not isinstance(descriptor, GetSetDescriptorType):
                    raise TypeError("resource has a custom __dict__ descriptor")
                self._dict = descriptor
            slots.extend(
                descriptor for descriptor in descriptors.values()
                if isinstance(descriptor, MemberDescriptorType)
            )
        self._slots = tuple(slots)
        self._original = self._read(resource)
        self.value = Resource.__new__(type(resource))
        state = deepcopy(self._original, {id(resource): self.value})
        self._write(self.value, state)

    def _read(self, resource: Resource) -> _State:
        fields = None
        if self._dict is not None:
            fields = cast(dict[str, object], self._dict.__get__(resource))
        slots: dict[MemberDescriptorType, object] = {}
        for descriptor in self._slots:
            with suppress(AttributeError):
                slots[descriptor] = descriptor.__get__(resource)
        return fields, slots

    def _write(self, resource: Resource, state: _State) -> None:
        fields, slots = state
        if self._dict is not None:
            self._dict.__set__(resource, fields)
        for descriptor in self._slots:
            if descriptor in slots:
                descriptor.__set__(resource, slots[descriptor])
            else:
                try:
                    descriptor.__get__(resource)
                except AttributeError:
                    continue
                descriptor.__delete__(resource)

    def commit(self) -> None:
        state = deepcopy(self._read(self.value), {id(self.value): self._resource})
        self._write(self._resource, state)
