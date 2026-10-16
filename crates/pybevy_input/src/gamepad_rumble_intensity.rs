use bevy::input::gamepad::GamepadRumbleIntensity;
use pybevy_core::{FromBorrowedStorage, ValueStorage, repr::float_repr};
use pybevy_macros::pyvalue;
use pyo3::prelude::*;

// bevy documents these as 0.0 to 1.0 and stores them unchecked.
#[pyvalue(domains = [
    strong_motor => [range_inclusive(0.0, 1.0)],
    weak_motor => [range_inclusive(0.0, 1.0)],
])]
#[pyclass(
    name = "GamepadRumbleIntensity",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone)]
pub struct PyGamepadRumbleIntensity {
    pub(crate) storage: ValueStorage<GamepadRumbleIntensity>,
}

impl PartialEq for PyGamepadRumbleIntensity {
    fn eq(&self, other: &Self) -> bool {
        matches!((self.to_bevy(), other.to_bevy()), (Ok(a), Ok(b)) if a == b)
    }
}

impl From<GamepadRumbleIntensity> for PyGamepadRumbleIntensity {
    fn from(value: GamepadRumbleIntensity) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyGamepadRumbleIntensity> for GamepadRumbleIntensity {
    type Error = PyErr;

    fn try_from(value: PyGamepadRumbleIntensity) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyGamepadRumbleIntensity> for GamepadRumbleIntensity {
    type Error = PyErr;

    fn try_from(value: &PyGamepadRumbleIntensity) -> PyResult<Self> {
        value.to_bevy()
    }
}

#[pymethods]
impl PyGamepadRumbleIntensity {
    #[new]
    #[pyo3(signature = (*, strong_motor = 1.0, weak_motor = 1.0))]
    fn new(strong_motor: f32, weak_motor: f32) -> PyResult<Self> {
        Ok(Self::from_owned(GamepadRumbleIntensity {
            strong_motor: Self::check_strong_motor(strong_motor)?,
            weak_motor: Self::check_weak_motor(weak_motor)?,
        }))
    }

    #[classattr]
    const MAX: PyGamepadRumbleIntensity = PyGamepadRumbleIntensity {
        storage: ValueStorage::owned(GamepadRumbleIntensity::MAX),
    };

    #[classattr]
    const WEAK_MAX: PyGamepadRumbleIntensity = PyGamepadRumbleIntensity {
        storage: ValueStorage::owned(GamepadRumbleIntensity::WEAK_MAX),
    };

    #[classattr]
    const STRONG_MAX: PyGamepadRumbleIntensity = PyGamepadRumbleIntensity {
        storage: ValueStorage::owned(GamepadRumbleIntensity::STRONG_MAX),
    };

    #[getter]
    fn strong_motor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.strong_motor)
    }

    #[getter]
    fn weak_motor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.weak_motor)
    }

    fn __repr__(&self) -> PyResult<String> {
        let intensity = self.to_bevy()?;
        Ok(format!(
            "GamepadRumbleIntensity(strong_motor={}, weak_motor={})",
            float_repr(intensity.strong_motor),
            float_repr(intensity.weak_motor)
        ))
    }
}
