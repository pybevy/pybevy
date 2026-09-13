use std::time::Duration;

use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::public_error::{
    DURATION_NEGATIVE, DURATION_NON_FINITE, DURATION_OVERFLOW, DURATION_ZERO, FREQUENCY_NON_FINITE,
    FREQUENCY_NON_POSITIVE, FREQUENCY_OUT_OF_RANGE, RELATIVE_SPEED_NEGATIVE,
    RELATIVE_SPEED_NON_FINITE, RELATIVE_SPEED_OUT_OF_RANGE,
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

/// Validate that `frequency` is finite and strictly positive.
pub fn try_finite_positive_frequency(frequency: f64) -> Result<f64, DurationFromFrequencyError> {
    if !frequency.is_finite() {
        return Err(DurationFromFrequencyError::NonFinite);
    }
    if frequency <= 0.0 {
        return Err(DurationFromFrequencyError::NonPositive);
    }
    Ok(frequency)
}

pub fn try_duration_from_hz(hz: f64) -> Result<Duration, DurationFromFrequencyError> {
    let hz = try_finite_positive_frequency(hz)?;
    try_positive_duration_from_secs_f64(1.0 / hz)
        .map_err(|_| DurationFromFrequencyError::OutOfRange)
}

pub fn duration_from_hz(hz: f64) -> PyResult<Duration> {
    try_duration_from_hz(hz).map_err(|error| PyTypeError::new_err(error.message()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeSpeedError {
    NonFinite,
    Negative,
    OutOfRange,
}

impl RelativeSpeedError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::NonFinite => RELATIVE_SPEED_NON_FINITE,
            Self::Negative => RELATIVE_SPEED_NEGATIVE,
            Self::OutOfRange => RELATIVE_SPEED_OUT_OF_RANGE,
        }
    }
}

/// Validate a virtual-clock speed multiplier against `max_delta`.
pub fn try_relative_speed(ratio: f64, max_delta: Duration) -> Result<f64, RelativeSpeedError> {
    if !ratio.is_finite() {
        return Err(RelativeSpeedError::NonFinite);
    }
    if ratio < 0.0 {
        return Err(RelativeSpeedError::Negative);
    }
    if Duration::try_from_secs_f64(max_delta.as_secs_f64() * ratio).is_err() {
        return Err(RelativeSpeedError::OutOfRange);
    }
    Ok(ratio)
}

pub fn relative_speed(ratio: f64, max_delta: Duration) -> PyResult<f64> {
    try_relative_speed(ratio, max_delta).map_err(|error| PyValueError::new_err(error.message()))
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
    fn relative_speed_rejects_what_bevy_asserts_on() {
        let max_delta = Duration::from_millis(250);
        assert_eq!(
            try_relative_speed(f64::NAN, max_delta),
            Err(RelativeSpeedError::NonFinite)
        );
        assert_eq!(
            try_relative_speed(f64::INFINITY, max_delta),
            Err(RelativeSpeedError::NonFinite)
        );
        assert_eq!(
            try_relative_speed(-1e-30, max_delta),
            Err(RelativeSpeedError::Negative)
        );
    }

    #[test]
    fn relative_speed_rejects_ratios_whose_frame_delta_overflows() {
        let max_delta = Duration::from_millis(250);
        assert_eq!(
            try_relative_speed(1e30, max_delta),
            Err(RelativeSpeedError::OutOfRange)
        );
        assert_eq!(try_relative_speed(0.0, max_delta), Ok(0.0));
        assert_eq!(try_relative_speed(4.0, max_delta), Ok(4.0));
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

    #[test]
    fn finite_positive_frequency_rejects_invalid_values() {
        assert_eq!(
            try_finite_positive_frequency(f64::NAN),
            Err(DurationFromFrequencyError::NonFinite)
        );
        assert_eq!(
            try_finite_positive_frequency(f64::INFINITY),
            Err(DurationFromFrequencyError::NonFinite)
        );
        assert_eq!(
            try_finite_positive_frequency(0.0),
            Err(DurationFromFrequencyError::NonPositive)
        );
        assert_eq!(
            try_finite_positive_frequency(-440.0),
            Err(DurationFromFrequencyError::NonPositive)
        );
        assert_eq!(try_finite_positive_frequency(440.0), Ok(440.0));
    }

    fn adapter_error(error: &PyErr, py: Python<'_>) -> (bool, bool, String) {
        let value = error.value(py);
        (
            value.is_instance_of::<PyTypeError>(),
            value.is_instance_of::<PyValueError>(),
            value.str().unwrap().to_string(),
        )
    }

    fn expect_adapter_error(
        error: &PyErr,
        py: Python<'_>,
        type_error: bool,
        value_error: bool,
        message: &str,
    ) {
        assert_eq!(adapter_error(error, py).0, type_error, "message: {message}");
        assert_eq!(
            adapter_error(error, py).1,
            value_error,
            "message: {message}"
        );
        assert_eq!(adapter_error(error, py).2, message);
    }

    #[test]
    fn duration_adapters_map_errors_to_their_classes_and_shared_messages() {
        Python::attach(|py| {
            let max_delta = Duration::from_millis(250);

            expect_adapter_error(
                &duration_from_secs_f64(-1.0).unwrap_err(),
                py,
                true,
                false,
                "Duration cannot be negative",
            );
            expect_adapter_error(
                &duration_from_secs_f64(f64::NAN).unwrap_err(),
                py,
                true,
                false,
                "Duration must be finite",
            );
            expect_adapter_error(
                &duration_from_secs_f64(f64::MAX).unwrap_err(),
                py,
                true,
                false,
                "Duration is too large",
            );
            expect_adapter_error(
                &positive_duration_from_secs_f64(0.0).unwrap_err(),
                py,
                true,
                false,
                "Duration must be greater than zero",
            );
            expect_adapter_error(
                &duration_from_hz(0.0).unwrap_err(),
                py,
                true,
                false,
                "Frequency must be greater than zero",
            );
            expect_adapter_error(
                &duration_from_hz(f64::NAN).unwrap_err(),
                py,
                true,
                false,
                "Frequency must be finite",
            );
            expect_adapter_error(
                &duration_from_hz(f64::MAX).unwrap_err(),
                py,
                true,
                false,
                "Frequency produces a timestep outside the supported duration range",
            );
            // relative_speed alone reports ValueError, not TypeError.
            expect_adapter_error(
                &relative_speed(-1.0, max_delta).unwrap_err(),
                py,
                false,
                true,
                "Relative speed cannot be negative",
            );
            expect_adapter_error(
                &relative_speed(f64::NAN, max_delta).unwrap_err(),
                py,
                false,
                true,
                "Relative speed must be finite",
            );
            expect_adapter_error(
                &relative_speed(1e30, max_delta).unwrap_err(),
                py,
                false,
                true,
                "Relative speed produces a frame delta outside the supported duration range",
            );
        });
    }

    #[test]
    fn duration_adapters_preserve_independent_unit_oracles() {
        assert_eq!(duration_from_secs_f64(0.0).unwrap(), Duration::ZERO);
        assert_eq!(duration_from_secs_f64(1.0).unwrap(), Duration::from_secs(1));
        assert_eq!(
            duration_from_secs_f64(0.000_001).unwrap(),
            Duration::from_micros(1)
        );
        assert_eq!(
            positive_duration_from_secs_f64(0.000_001).unwrap(),
            Duration::from_micros(1)
        );
        assert_eq!(duration_from_hz(0.5).unwrap(), Duration::from_secs(2));
        assert_eq!(duration_from_hz(4.0).unwrap(), Duration::from_millis(250));
    }

    #[test]
    fn duration_from_py_accepts_timedelta_and_pins_the_bc99_intended_diagnostic() {
        Python::attach(|py| {
            let timedelta = py.import("datetime").unwrap().getattr("timedelta").unwrap();

            // timedelta(days=1, seconds=2) positional: 86_402 s exact.
            let one_day_two_seconds = timedelta.call1((1, 2)).unwrap().into_any();
            assert_eq!(
                duration_from_py(&one_day_two_seconds).unwrap(),
                Duration::from_secs(86_402)
            );

            // A non-numeric string is the legitimate unsupported-kind contract:
            // the final kind-rejection, exact message.
            let text = "3".into_pyobject(py).unwrap().into_any();
            let (type_error, value_error, message) =
                adapter_error(&duration_from_py(&text).unwrap_err(), py);
            assert!(type_error && !value_error);
            assert_eq!(
                message,
                "Duration must be a Duration object, float (seconds), or int (seconds)"
            );

            // Preserve the extractor's value diagnostic rather than a kind error.
            let negative = timedelta.call1((-1,)).unwrap();
            let extracted: PyResult<Duration> = negative.extract();
            let (type_error, value_error, message) = adapter_error(&extracted.unwrap_err(), py);
            assert!(!type_error && value_error);
            assert_eq!(
                message,
                "It is not possible to convert a negative timedelta to a Rust Duration"
            );
        });
    }
}
