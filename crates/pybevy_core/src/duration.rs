use std::time::Duration;

use pybevy_storage::domain::{
    DomainValues, DurationInput, FINITE_NON_NEGATIVE, FieldDomain, POSITIVE, check,
};
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::public_error::{
    DURATION_NEGATIVE, DURATION_NON_FINITE, DURATION_OVERFLOW, FREQUENCY_OUT_OF_RANGE,
    RELATIVE_SPEED_OUT_OF_RANGE,
};

/// Seconds a duration parameter accepts; zero is allowed.
pub const DURATION_DOMAIN: &[FieldDomain] = FINITE_NON_NEGATIVE;
/// A duration that must not be zero, such as a timestep or wrap period.
pub const POSITIVE_DURATION_DOMAIN: &[FieldDomain] = POSITIVE;
/// A frequency in hertz.
pub const FREQUENCY_DOMAIN: &[FieldDomain] = POSITIVE;
/// A virtual-clock speed multiplier; zero is allowed.
pub const RELATIVE_SPEED_DOMAIN: &[FieldDomain] = FINITE_NON_NEGATIVE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationFromSecondsError {
    Negative,
    NonFinite,
    Overflow,
}

impl DurationFromSecondsError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Negative => DURATION_NEGATIVE,
            Self::NonFinite => DURATION_NON_FINITE,
            Self::Overflow => DURATION_OVERFLOW,
        }
    }
}

/// Convert floating-point seconds without invoking `Duration`'s panicking constructor.
pub fn try_duration_from_secs_f64(seconds: f64) -> Result<Duration, DurationFromSecondsError> {
    if !seconds.is_finite() {
        return Err(DurationFromSecondsError::NonFinite);
    }
    if seconds < 0.0 {
        return Err(DurationFromSecondsError::Negative);
    }
    Duration::try_from_secs_f64(seconds).map_err(|_| DurationFromSecondsError::Overflow)
}

/// Check a duration parameter against `domains` and convert it.
pub fn check_duration(
    owner: &'static str,
    field: &'static str,
    value: DurationInput,
    domains: &'static [FieldDomain],
) -> PyResult<Duration> {
    Ok(check(owner, field, value, domains)?.stored())
}

/// The fixed timestep of a frequency; `1 / hz` must be a positive,
/// representable `Duration`.
pub fn timestep_from_hz(owner: &'static str, field: &'static str, hz: f64) -> PyResult<Duration> {
    let hz = check(owner, field, hz, FREQUENCY_DOMAIN)?;
    match Duration::try_from_secs_f64(1.0 / hz) {
        Ok(timestep) if !timestep.is_zero() => Ok(timestep),
        _ => Err(PyValueError::new_err(FREQUENCY_OUT_OF_RANGE)),
    }
}

/// Check a virtual-clock speed multiplier, including that its frame delta
/// at `max_delta` stays representable.
pub fn relative_speed<T: DomainValues + Into<f64>>(
    owner: &'static str,
    field: &'static str,
    ratio: T,
    max_delta: Duration,
) -> PyResult<f64> {
    let ratio = check(owner, field, ratio, RELATIVE_SPEED_DOMAIN)?.into();
    if Duration::try_from_secs_f64(max_delta.as_secs_f64() * ratio).is_err() {
        return Err(PyValueError::new_err(RELATIVE_SPEED_OUT_OF_RANGE));
    }
    Ok(ratio)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn checked_float_seconds_reject_invalid_values() {
        assert_eq!(
            try_duration_from_secs_f64(f64::NAN),
            Err(DurationFromSecondsError::NonFinite)
        );
        assert_eq!(
            try_duration_from_secs_f64(f64::INFINITY),
            Err(DurationFromSecondsError::NonFinite)
        );
        assert_eq!(
            try_duration_from_secs_f64(-1.0),
            Err(DurationFromSecondsError::Negative)
        );
        assert_eq!(
            try_duration_from_secs_f64(f64::MAX),
            Err(DurationFromSecondsError::Overflow)
        );
    }

    #[test]
    fn checked_float_seconds_preserve_valid_values() {
        assert_eq!(
            try_duration_from_secs_f64(1.25),
            Ok(Duration::from_millis(1_250))
        );
    }

    #[test]
    fn duration_parameters_convert_accepted_values() {
        let seconds =
            |value| check_duration("T", "d", DurationInput::Seconds(value), DURATION_DOMAIN);
        assert_eq!(seconds(0.0).unwrap(), Duration::ZERO);
        assert_eq!(seconds(0.000_001).unwrap(), Duration::from_micros(1));
        assert_eq!(seconds(1e-320).unwrap(), Duration::ZERO);
        assert!(
            check_duration(
                "T",
                "d",
                DurationInput::Seconds(1e-320),
                POSITIVE_DURATION_DOMAIN
            )
            .is_err()
        );
    }

    #[test]
    fn timesteps_from_frequency_stay_positive_and_representable() {
        assert_eq!(
            timestep_from_hz("T", "hz", 0.5).unwrap(),
            Duration::from_secs(2)
        );
        assert_eq!(
            timestep_from_hz("T", "hz", 4.0).unwrap(),
            Duration::from_millis(250)
        );
        for hz in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX, 1e-30] {
            assert!(timestep_from_hz("T", "hz", hz).is_err(), "{hz}");
        }
    }

    #[test]
    fn relative_speed_rejects_what_bevy_asserts_on() {
        let max_delta = Duration::from_millis(250);
        for ratio in [f64::NAN, f64::INFINITY, -1e-30, 1e30] {
            assert!(
                relative_speed("T", "s", ratio, max_delta).is_err(),
                "{ratio}"
            );
        }
        assert_eq!(relative_speed("T", "s", 0.0, max_delta).unwrap(), 0.0);
        assert_eq!(relative_speed("T", "s", 4.0f32, max_delta).unwrap(), 4.0);
    }
}
