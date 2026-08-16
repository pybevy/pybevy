use std::time::Duration;

use bevy::{platform::time::Instant, time::TimeUpdateStrategy};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(TimeUpdateStrategy, resource, no_reflect)]
#[pyclass(name = "TimeUpdateStrategy", module = "pybevy.time")]
pub enum PyTimeUpdateStrategy {
    Automatic(),
    #[py_unsupported]
    #[py_bevy(tuple)]
    ManualInstant {
        value: Instant,
    },
    #[py_bevy(tuple)]
    ManualDuration {
        duration: Duration,
    },
    #[py_bevy(tuple)]
    FixedTimesteps {
        steps: u32,
    },
}

#[pymethods]
impl PyTimeUpdateStrategy {
    fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()? {
            TimeUpdateStrategy::Automatic => Ok("TimeUpdateStrategy.Automatic()".to_string()),
            TimeUpdateStrategy::ManualDuration(duration) => Ok(format!(
                "TimeUpdateStrategy.ManualDuration(timedelta(seconds={}))",
                duration.as_secs_f64()
            )),
            TimeUpdateStrategy::FixedTimesteps(steps) => {
                Ok(format!("TimeUpdateStrategy.FixedTimesteps({steps})"))
            }
            TimeUpdateStrategy::ManualInstant(_) => {
                Ok("TimeUpdateStrategy.<unsupported ManualInstant>".to_string())
            }
        }
    }
}
