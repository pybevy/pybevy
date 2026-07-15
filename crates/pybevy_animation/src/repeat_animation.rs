use bevy::animation::RepeatAnimation;
use pyo3::prelude::*;

#[pyclass(name = "RepeatAnimation", eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyRepeatAnimation {
    Never(),
    Count { value: u32 },
    Forever(),
}

#[pymethods]
impl PyRepeatAnimation {
    pub fn __repr__(&self) -> String {
        match self {
            PyRepeatAnimation::Never() => "RepeatAnimation.Never()".to_string(),
            PyRepeatAnimation::Count { value } => format!("RepeatAnimation.Count({value})"),
            PyRepeatAnimation::Forever() => "RepeatAnimation.Forever()".to_string(),
        }
    }
}

impl From<&PyRepeatAnimation> for RepeatAnimation {
    fn from(value: &PyRepeatAnimation) -> Self {
        match value {
            PyRepeatAnimation::Never() => RepeatAnimation::Never,
            PyRepeatAnimation::Count { value } => RepeatAnimation::Count(*value),
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
            RepeatAnimation::Count(value) => PyRepeatAnimation::Count { value },
            RepeatAnimation::Forever => PyRepeatAnimation::Forever(),
        }
    }
}

impl Default for PyRepeatAnimation {
    fn default() -> Self {
        Self::Never()
    }
}
