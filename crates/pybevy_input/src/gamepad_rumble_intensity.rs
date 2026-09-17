use bevy::input::gamepad::GamepadRumbleIntensity;
use pybevy_core::public_error::value_out_of_range;
use pyo3::{exceptions::PyValueError, prelude::*};

/// bevy documents these as 0.0 to 1.0 and stores them unchecked.
pub(crate) fn check_motor(name: &str, value: f32) -> PyResult<f32> {
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(PyValueError::new_err(value_out_of_range(
            name, 0.0, 1.0, value,
        )))
    }
}

#[pyclass(
    name = "GamepadRumbleIntensity",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PyGamepadRumbleIntensity {
    pub strong_motor: f32,
    pub weak_motor: f32,
}

impl From<GamepadRumbleIntensity> for PyGamepadRumbleIntensity {
    fn from(intensity: GamepadRumbleIntensity) -> Self {
        PyGamepadRumbleIntensity {
            strong_motor: intensity.strong_motor,
            weak_motor: intensity.weak_motor,
        }
    }
}

impl From<PyGamepadRumbleIntensity> for GamepadRumbleIntensity {
    fn from(intensity: PyGamepadRumbleIntensity) -> Self {
        GamepadRumbleIntensity {
            strong_motor: intensity.strong_motor,
            weak_motor: intensity.weak_motor,
        }
    }
}

#[pymethods]
impl PyGamepadRumbleIntensity {
    #[new]
    #[pyo3(signature = (*, strong_motor = 1.0, weak_motor = 1.0))]
    fn new(strong_motor: f32, weak_motor: f32) -> PyResult<Self> {
        Ok(PyGamepadRumbleIntensity {
            strong_motor: check_motor("strong_motor", strong_motor)?,
            weak_motor: check_motor("weak_motor", weak_motor)?,
        })
    }

    #[classattr]
    const MAX: PyGamepadRumbleIntensity = PyGamepadRumbleIntensity {
        strong_motor: 1.0,
        weak_motor: 1.0,
    };

    #[classattr]
    const WEAK_MAX: PyGamepadRumbleIntensity = PyGamepadRumbleIntensity {
        strong_motor: 0.0,
        weak_motor: 1.0,
    };

    #[classattr]
    const STRONG_MAX: PyGamepadRumbleIntensity = PyGamepadRumbleIntensity {
        strong_motor: 1.0,
        weak_motor: 0.0,
    };

    #[getter]
    fn strong_motor(&self) -> f32 {
        self.strong_motor
    }

    #[getter]
    fn weak_motor(&self) -> f32 {
        self.weak_motor
    }

    fn __repr__(&self) -> String {
        format!(
            "GamepadRumbleIntensity(strong_motor={}, weak_motor={})",
            self.strong_motor, self.weak_motor
        )
    }
}
