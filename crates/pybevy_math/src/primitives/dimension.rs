use std::fmt::Display;

use bevy::math::{Vec2, Vec3};
use pybevy_core::{
    StorageError,
    public_error::{primitive_angle, primitive_dimension},
};

pub trait Dimension: Copy + Display {
    fn is_valid_dimension(self) -> bool;
    fn without_negative_zero(self) -> Self;
}

impl Dimension for f32 {
    fn is_valid_dimension(self) -> bool {
        self.is_finite() && self >= 0.0
    }

    fn without_negative_zero(self) -> Self {
        self.abs()
    }
}

impl Dimension for Vec2 {
    fn is_valid_dimension(self) -> bool {
        self.is_finite() && self.min_element() >= 0.0
    }

    fn without_negative_zero(self) -> Self {
        self.abs()
    }
}

impl Dimension for Vec3 {
    fn is_valid_dimension(self) -> bool {
        self.is_finite() && self.min_element() >= 0.0
    }

    fn without_negative_zero(self) -> Self {
        self.abs()
    }
}

/// Validate a primitive dimension: finite and non-negative, with `-0.0` stored as `0.0`.
pub fn dimension<T: Dimension>(name: &str, value: T) -> Result<T, StorageError> {
    if value.is_valid_dimension() {
        Ok(value.without_negative_zero())
    } else {
        Err(StorageError::InvalidValue(primitive_dimension(name, value)))
    }
}

pub fn angle(name: &str, value: f32) -> Result<f32, StorageError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(StorageError::InvalidValue(primitive_angle(name, value)))
    }
}

pub fn check_half_size2(value: &Vec2) -> Result<(), StorageError> {
    dimension("half_size", *value).map(drop)
}

pub fn check_half_size3(value: &Vec3) -> Result<(), StorageError> {
    dimension("half_size", *value).map(drop)
}
