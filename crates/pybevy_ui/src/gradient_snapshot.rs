use bevy::ui::{ConicGradient, LinearGradient, RadialGradient};
use pyo3::{Borrowed, PyClass, prelude::*};

use crate::{
    conic_gradient::PyConicGradient, linear_gradient::PyLinearGradient,
    radial_gradient::PyRadialGradient,
};

pub trait SnapshotValue: Clone {
    type Wrapper: PyClass + Into<PyClassInitializer<Self::Wrapper>>;

    fn into_read_only_wrapper(self) -> Self::Wrapper;
    fn from_wrapper(wrapper: &Self::Wrapper) -> Self;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot<T>(pub T);

impl<T> From<T> for Snapshot<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: SnapshotValue> FromPyObject<'_, '_> for Snapshot<T> {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let wrapper = obj.extract::<PyRef<'_, T::Wrapper>>()?;
        Ok(Self(T::from_wrapper(&*wrapper)))
    }
}

impl<'py, T: SnapshotValue> IntoPyObject<'py> for Snapshot<T> {
    type Target = T::Wrapper;
    type Output = Bound<'py, T::Wrapper>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        Bound::new(py, self.0.into_read_only_wrapper())
    }
}

macro_rules! snapshot_value {
    ($($value:ty => $wrapper:ty),* $(,)?) => {$ (
        impl SnapshotValue for $value {
            type Wrapper = $wrapper;

            fn into_read_only_wrapper(self) -> Self::Wrapper {
                <$wrapper>::read_only_snapshot(self)
            }

            fn from_wrapper(wrapper: &Self::Wrapper) -> Self {
                wrapper.inner.clone()
            }
        }

        impl From<Snapshot<$value>> for $value {
            fn from(value: Snapshot<$value>) -> Self {
                value.0
            }
        }
    )* };
}

snapshot_value!(
    LinearGradient => PyLinearGradient,
    RadialGradient => PyRadialGradient,
    ConicGradient => PyConicGradient,
);
