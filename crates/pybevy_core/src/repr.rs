use std::fmt::LowerExp;

use bevy::math::{Quat, Vec2, Vec3, Vec3A, Vec4};
pub use pybevy_storage::float_repr::float_repr;
use pyo3::{IntoPyObject, IntoPyObjectExt, prelude::*, types::PyString};

pub trait ReprLiteral {
    fn repr_literal(&self) -> String;
}

pub fn float_list_repr<T: LowerExp + Into<f64> + Copy>(values: &[T]) -> String {
    let items: Vec<String> = values.iter().copied().map(float_repr).collect();
    format!("[{}]", items.join(", "))
}

pub fn str_repr(value: &str) -> String {
    Python::attach(|py| {
        PyString::new(py, value)
            .repr()
            .and_then(|repr| repr.to_str().map(str::to_owned))
    })
    .unwrap_or_else(|_| format!("{value:?}"))
}

pub fn object_repr<T>(value: T) -> String
where
    T: for<'py> IntoPyObject<'py>,
{
    Python::attach(|py| {
        let object = value.into_bound_py_any(py)?;
        object
            .repr()
            .and_then(|repr| repr.to_str().map(str::to_owned))
    })
    .unwrap_or_else(|error| format!("<repr failed: {error}>"))
}

impl ReprLiteral for bool {
    fn repr_literal(&self) -> String {
        if *self { "True" } else { "False" }.into()
    }
}

macro_rules! display_literal {
    ($($ty:ty),*) => {$(
        impl ReprLiteral for $ty {
            fn repr_literal(&self) -> String {
                self.to_string()
            }
        }
    )*};
}

display_literal!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl ReprLiteral for f32 {
    fn repr_literal(&self) -> String {
        float_repr(*self)
    }
}

impl ReprLiteral for f64 {
    fn repr_literal(&self) -> String {
        float_repr(*self)
    }
}

impl ReprLiteral for str {
    fn repr_literal(&self) -> String {
        str_repr(self)
    }
}

impl ReprLiteral for String {
    fn repr_literal(&self) -> String {
        str_repr(self)
    }
}

impl<T: ReprLiteral> ReprLiteral for Option<T> {
    fn repr_literal(&self) -> String {
        match self {
            Some(value) => value.repr_literal(),
            None => "None".into(),
        }
    }
}

impl ReprLiteral for Vec2 {
    fn repr_literal(&self) -> String {
        format!("Vec2({}, {})", float_repr(self.x), float_repr(self.y))
    }
}

impl ReprLiteral for Vec3 {
    fn repr_literal(&self) -> String {
        format!(
            "Vec3({}, {}, {})",
            float_repr(self.x),
            float_repr(self.y),
            float_repr(self.z)
        )
    }
}

impl ReprLiteral for Vec3A {
    fn repr_literal(&self) -> String {
        format!(
            "Vec3A({}, {}, {})",
            float_repr(self.x),
            float_repr(self.y),
            float_repr(self.z)
        )
    }
}

impl ReprLiteral for Vec4 {
    fn repr_literal(&self) -> String {
        format!(
            "Vec4({}, {}, {}, {})",
            float_repr(self.x),
            float_repr(self.y),
            float_repr(self.z),
            float_repr(self.w)
        )
    }
}

impl ReprLiteral for Quat {
    fn repr_literal(&self) -> String {
        format!(
            "Quat.from_xyzw({}, {}, {}, {})",
            float_repr(self.x),
            float_repr(self.y),
            float_repr(self.z),
            float_repr(self.w)
        )
    }
}
