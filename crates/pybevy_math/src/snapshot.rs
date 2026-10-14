use bevy::math::{IVec2, UVec2, Vec2, Vec3};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{Borrowed, PyClass, prelude::*};

use crate::{ivec2::PyIVec2, uvec2::PyUVec2, vec2::PyVec2, vec3::PyVec3};

/// A Bevy math value whose wrapper can hold an enforced read-only snapshot.
pub trait SnapshotValue: Copy + for<'a> TryFrom<&'a Self::Wrapper, Error = PyErr> {
    type Wrapper: PyClass
        + FromBorrowedStorage<ValueStorage<Self>>
        + Into<PyClassInitializer<Self::Wrapper>>;
}

/// A frozen payload value that Python reads as a read-only snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot<T>(pub T);

impl<T: SnapshotValue> Snapshot<T> {
    pub fn into_wrapper(self) -> T::Wrapper {
        T::Wrapper::from_borrowed(ValueStorage::read_only_snapshot(self.0))
    }
}

impl<T> From<T> for Snapshot<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: SnapshotValue> FromPyObject<'_, '_> for Snapshot<T> {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let wrapper = obj.extract::<PyRef<'_, T::Wrapper>>()?;
        Ok(Self(T::try_from(&*wrapper)?))
    }
}

impl<'py, T: SnapshotValue> IntoPyObject<'py> for Snapshot<T> {
    type Target = T::Wrapper;
    type Output = Bound<'py, T::Wrapper>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        Bound::new(py, self.into_wrapper())
    }
}

macro_rules! snapshot_value {
    ($($value:ty => $wrapper:ty),* $(,)?) => {$(
        impl SnapshotValue for $value {
            type Wrapper = $wrapper;
        }

        impl From<Snapshot<$value>> for $value {
            fn from(value: Snapshot<$value>) -> Self {
                value.0
            }
        }
    )*};
}

snapshot_value!(Vec2 => PyVec2, IVec2 => PyIVec2, UVec2 => PyUVec2, Vec3 => PyVec3);
