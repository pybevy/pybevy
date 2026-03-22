"""Benchmarks for PyBevy math types: Vec3, Quat, Transform, Cuboid."""

import math

import numpy as np
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.math import Cuboid, Quat, Vec3
from pybevy.transform import Transform


def test_vec3_new(benchmark: BenchmarkFixture) -> None:
    def create_vec3() -> Vec3:
        return Vec3(1.0, 2.0, 3.0)

    result = benchmark(create_vec3)
    assert result == Vec3(1.0, 2.0, 3.0)


def test_vec3_zero(benchmark: BenchmarkFixture) -> None:
    def create_vec3() -> Vec3:
        return Vec3.ZERO

    result = benchmark(create_vec3)
    assert result == Vec3.ZERO


def test_vec3_get_x(benchmark: BenchmarkFixture) -> None:
    v = Vec3(1.0, 2.0, 3.0)

    def get_x(vec: Vec3) -> float:
        return vec.x

    result = benchmark(get_x, v)
    assert result == 1.0


def test_vec3_get_length(benchmark: BenchmarkFixture) -> None:
    v = Vec3(3.0, 4.0, 0.0)

    def get_length(vec: Vec3) -> float:
        return vec.length()

    result = benchmark(get_length, v)
    assert result == 5.0


def test_vec3_set_value(benchmark: BenchmarkFixture) -> None:
    v = Vec3(1.0, 2.0, 3.0)

    def set_value(vec: Vec3) -> Vec3:
        vec.x = 3
        return vec

    result = benchmark(set_value, v)
    assert result == Vec3(3.0, 2.0, 3.0)


def test_vec3_add(benchmark: BenchmarkFixture) -> None:
    v1 = Vec3(1.0, 2.0, 3.0)
    v2 = Vec3(4.0, 5.0, 6.0)
    result = benchmark(lambda: v1 + v2)
    assert result == Vec3(5.0, 7.0, 9.0)


def test_vec3_sub(benchmark: BenchmarkFixture) -> None:
    v1 = Vec3(4.0, 5.0, 6.0)
    v2 = Vec3(1.0, 2.0, 3.0)
    result = benchmark(lambda: v1 - v2)
    assert result == Vec3(3.0, 3.0, 3.0)


def test_vec3_mul(benchmark: BenchmarkFixture) -> None:
    v = Vec3(1.0, 2.0, 3.0)
    scalar = 2.0
    result = benchmark(lambda: v * scalar)
    assert result == Vec3(2.0, 4.0, 6.0)


def test_vec3_numpy_add(benchmark: BenchmarkFixture) -> None:
    """Baseline comparison with NumPy."""
    v1 = np.array([1.0, 2.0, 3.0])
    v2 = np.array([4.0, 5.0, 6.0])
    result = benchmark(lambda: v1 + v2)
    assert np.array_equal(result, np.array([5.0, 7.0, 9.0]))


def test_rotate_quat(benchmark: BenchmarkFixture) -> None:
    q = Quat.IDENTITY
    mulquat = Quat.from_rotation_y(math.pi)
    result = benchmark(lambda: q * mulquat)
    assert result == Quat.from_rotation_y(math.pi)


def test_transform_new(benchmark: BenchmarkFixture) -> None:
    def create_transform() -> Transform:
        return Transform(Vec3(1.0, 2.0, 3.0), Quat.IDENTITY)

    result = benchmark(create_transform)
    assert result == Transform(Vec3(1.0, 2.0, 3.0), Quat.IDENTITY)


def test_transform_get_translation_x(benchmark: BenchmarkFixture) -> None:
    t = Transform.from_xyz(1.0, 2.0, 3.0)

    def get_translation_x(transform: Transform) -> float:
        return transform.translation.x

    result = benchmark(get_translation_x, t)
    assert result == 1.0


def test_cuboid_new(benchmark: BenchmarkFixture) -> None:
    size = Vec3(1.0, 2.0, 3.0)
    benchmark(lambda: Cuboid.from_size(size))
