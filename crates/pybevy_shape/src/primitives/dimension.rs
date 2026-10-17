use bevy::math::{Vec2, Vec3};
use pybevy_core::domain::{DomainValues, DomainViolation, FINITE_NON_NEGATIVE, check_for};
use pyo3::PyClass;

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

/// Validate a primitive dimension of `O`: finite and non-negative, with `-0.0`
/// stored as `0.0`.
pub fn dimension<O: PyClass, T: Dimension>(
    name: &'static str,
    value: T,
) -> Result<T, DomainViolation> {
    Ok(check_for::<O, _>(name, value, FINITE_NON_NEGATIVE)?.without_negative_zero())
}
