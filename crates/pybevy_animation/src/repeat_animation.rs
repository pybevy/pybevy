use bevy::animation::RepeatAnimation;
use pyo3::prelude::*;

#[pyclass(name = "RepeatAnimation", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyRepeatAnimation {
    Never(),
    Count(u32),
    Forever(),
}

#[pymethods]
impl PyRepeatAnimation {
    #[new]
    pub fn new() -> Self {
        PyRepeatAnimation::Never()
    }

    pub fn count(&self) -> Option<u32> {
        match self {
            PyRepeatAnimation::Count(count) => Some(*count),
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match self {
            PyRepeatAnimation::Never() => "RepeatAnimation.Never()".to_string(),
            PyRepeatAnimation::Count(count) => format!("RepeatAnimation.Count({})", count),
            PyRepeatAnimation::Forever() => "RepeatAnimation.Forever()".to_string(),
        }
    }
}

impl From<&PyRepeatAnimation> for RepeatAnimation {
    fn from(value: &PyRepeatAnimation) -> Self {
        match value {
            PyRepeatAnimation::Never() => RepeatAnimation::Never,
            PyRepeatAnimation::Count(count) => RepeatAnimation::Count(*count),
            PyRepeatAnimation::Forever() => RepeatAnimation::Forever,
        }
    }
}

impl From<PyRepeatAnimation> for RepeatAnimation {
    fn from(value: PyRepeatAnimation) -> Self {
        (&value).into()
    }
}

impl From<RepeatAnimation> for PyRepeatAnimation {
    fn from(value: RepeatAnimation) -> Self {
        match value {
            RepeatAnimation::Never => PyRepeatAnimation::Never(),
            RepeatAnimation::Count(count) => PyRepeatAnimation::Count(count),
            RepeatAnimation::Forever => PyRepeatAnimation::Forever(),
        }
    }
}

impl Default for PyRepeatAnimation {
    fn default() -> Self {
        Self::new()
    }
}
