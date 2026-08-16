use std::time::Duration;

use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::public_error::{
    DURATION_NEGATIVE, DURATION_NON_FINITE, DURATION_OVERFLOW, DURATION_ZERO, FREQUENCY_NON_FINITE,
    FREQUENCY_NON_POSITIVE, FREQUENCY_OUT_OF_RANGE,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationFromSecondsError {
    Negative,
    NonFinite,
    Overflow,
    Zero,
}

impl DurationFromSecondsError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Negative => DURATION_NEGATIVE,
            Self::NonFinite => DURATION_NON_FINITE,
            Self::Overflow => DURATION_OVERFLOW,
            Self::Zero => DURATION_ZERO,
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

/// PyO3 adapter for the shared floating-point duration validation.
pub fn duration_from_secs_f64(seconds: f64) -> PyResult<Duration> {
    try_duration_from_secs_f64(seconds).map_err(|error| PyTypeError::new_err(error.message()))
}

pub fn try_positive_duration_from_secs_f64(
    seconds: f64,
) -> Result<Duration, DurationFromSecondsError> {
    let duration = try_duration_from_secs_f64(seconds)?;
    if duration.is_zero() {
        return Err(DurationFromSecondsError::Zero);
    }
    Ok(duration)
}

pub fn positive_duration_from_secs_f64(seconds: f64) -> PyResult<Duration> {
    try_positive_duration_from_secs_f64(seconds)
        .map_err(|error| PyTypeError::new_err(error.message()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationFromFrequencyError {
    NonFinite,
    NonPositive,
    OutOfRange,
}

impl DurationFromFrequencyError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::NonFinite => FREQUENCY_NON_FINITE,
            Self::NonPositive => FREQUENCY_NON_POSITIVE,
            Self::OutOfRange => FREQUENCY_OUT_OF_RANGE,
        }
    }
}

pub fn try_duration_from_hz(hz: f64) -> Result<Duration, DurationFromFrequencyError> {
    if !hz.is_finite() {
        return Err(DurationFromFrequencyError::NonFinite);
    }
    if hz <= 0.0 {
        return Err(DurationFromFrequencyError::NonPositive);
    }
    try_positive_duration_from_secs_f64(1.0 / hz)
        .map_err(|_| DurationFromFrequencyError::OutOfRange)
}

pub fn duration_from_hz(hz: f64) -> PyResult<Duration> {
    try_duration_from_hz(hz).map_err(|error| PyTypeError::new_err(error.message()))
}

/// Convert a Python `timedelta`, `float` (seconds), or `int` (seconds) to a `Duration`.
pub fn duration_from_py(value: &Bound<'_, PyAny>) -> PyResult<Duration> {
    if let Ok(duration) = value.extract::<Duration>() {
        return Ok(duration);
    }

    if let Ok(seconds) = value.extract::<u64>() {
        return Ok(Duration::from_secs(seconds));
    }

    if let Ok(seconds) = value.extract::<f64>() {
        return duration_from_secs_f64(seconds);
    }

    Err(PyTypeError::new_err(
        "Duration must be a Duration object, float (seconds), or int (seconds)",
    ))
}

#[cfg(test)]
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
    fn positive_duration_rejects_values_that_round_to_zero() {
        assert_eq!(
            try_positive_duration_from_secs_f64(0.0),
            Err(DurationFromSecondsError::Zero)
        );
        assert_eq!(
            try_positive_duration_from_secs_f64(f64::MIN_POSITIVE),
            Err(DurationFromSecondsError::Zero)
        );
    }

    #[test]
    fn python_values_convert_by_kind() {
        Python::initialize();
        Python::attach(|py| {
            let exact = 1u64 << 53 | 1;
            let cases: [(Bound<'_, PyAny>, Duration); 5] = [
                (
                    5i64.into_pyobject(py).unwrap().into_any(),
                    Duration::from_secs(5),
                ),
                (
                    2.5f64.into_pyobject(py).unwrap().into_any(),
                    Duration::from_millis(2_500),
                ),
                (
                    true.into_pyobject(py).unwrap().to_owned().into_any(),
                    Duration::from_secs(1),
                ),
                (0i64.into_pyobject(py).unwrap().into_any(), Duration::ZERO),
                (
                    exact.into_pyobject(py).unwrap().into_any(),
                    Duration::from_secs(exact),
                ),
            ];
            for (value, expected) in cases {
                assert_eq!(duration_from_py(&value).unwrap(), expected, "{value:?}");
            }
        });
    }

    #[test]
    fn python_values_reject_invalid_kinds() {
        Python::initialize();
        Python::attach(|py| {
            let negative = (-1i64).into_pyobject(py).unwrap().into_any();
            assert!(duration_from_py(&negative).is_err());

            let text = "3".into_pyobject(py).unwrap().into_any();
            assert!(duration_from_py(&text).is_err());
        });
    }

    #[test]
    fn checked_frequency_rejects_invalid_timesteps() {
        assert_eq!(
            try_duration_from_hz(f64::NAN),
            Err(DurationFromFrequencyError::NonFinite)
        );
        assert_eq!(
            try_duration_from_hz(0.0),
            Err(DurationFromFrequencyError::NonPositive)
        );
        assert_eq!(
            try_duration_from_hz(f64::MAX),
            Err(DurationFromFrequencyError::OutOfRange)
        );
        assert_eq!(try_duration_from_hz(2.0), Ok(Duration::from_millis(500)));
    }
}
