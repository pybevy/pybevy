use bevy::math::{Vec2, Vec3};
use pybevy_core::{
    StorageError,
    domain::{DomainValues, FINITE, FINITE_NON_NEGATIVE, check},
};

pub trait Dimension: Copy + DomainValues {
    fn without_negative_zero(self) -> Self;
}

impl Dimension for f32 {
    fn without_negative_zero(self) -> Self {
        self.abs()
    }
}

impl Dimension for Vec2 {
    fn without_negative_zero(self) -> Self {
        self.abs()
    }
}

impl Dimension for Vec3 {
    fn without_negative_zero(self) -> Self {
        self.abs()
    }
}

/// Validate a primitive dimension: finite and non-negative, with `-0.0` stored as `0.0`.
pub fn dimension<T: Dimension>(
    owner: &'static str,
    name: &'static str,
    value: T,
) -> Result<T, StorageError> {
    Ok(check(owner, name, value, FINITE_NON_NEGATIVE)?.without_negative_zero())
}

pub fn angle(owner: &'static str, name: &'static str, value: f32) -> Result<f32, StorageError> {
    Ok(check(owner, name, value, FINITE)?)
}
