use std::{fmt::LowerExp, iter::repeat_n};

use bevy::math::{Quat, Vec2, Vec3, Vec3A, Vec4};
use pyo3::{IntoPyObject, IntoPyObjectExt, prelude::*, types::PyString};

pub trait ReprLiteral {
    fn repr_literal(&self) -> String;
}

pub fn float_repr<T: LowerExp + Into<f64> + Copy>(value: T) -> String {
    let wide: f64 = value.into();
    if wide.is_nan() {
        return "nan".into();
    }
    if wide.is_infinite() {
        return if wide > 0.0 { "inf" } else { "-inf" }.into();
    }
    let scientific = format!("{value:e}");
    let (negative, unsigned) = match scientific.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, scientific.as_str()),
    };
    let (mantissa, exponent) = unsigned.split_once('e').unwrap_or((unsigned, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let point = exponent + 1;
    let count = digits.len() as i32;
    let mut out = String::with_capacity(digits.len() + 8);
    if negative {
        out.push('-');
    }
    if point > -4 && point <= 16 {
        if point <= 0 {
            out.push_str("0.");
            out.extend(repeat_n('0', (-point) as usize));
            out.push_str(&digits);
        } else if point >= count {
            out.push_str(&digits);
            out.extend(repeat_n('0', (point - count) as usize));
            out.push_str(".0");
        } else {
            out.push_str(&digits[..point as usize]);
            out.push('.');
            out.push_str(&digits[point as usize..]);
        }
    } else {
        out.push_str(&digits[..1]);
        if count > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let shown = point - 1;
        out.push_str(if shown < 0 { "e-" } else { "e+" });
        out.push_str(&format!("{:02}", shown.abs()));
    }
    out
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
            "Quat({}, {}, {}, {})",
            float_repr(self.x),
            float_repr(self.y),
            float_repr(self.z),
            float_repr(self.w)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_repr_matches_python_float_repr() {
        let cases: [(f64, &str); 16] = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (-2.5, "-2.5"),
            (0.1, "0.1"),
            (123.456, "123.456"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.5e20, "1.5e+20"),
            (2.5e-300, "2.5e-300"),
            (f64::NAN, "nan"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (1e100, "1e+100"),
        ];
        for (value, expected) in cases {
            assert_eq!(float_repr(value), expected, "{value:e}");
        }
    }

    #[test]
    fn f32_float_repr_uses_shortest_single_precision_digits() {
        assert_eq!(float_repr(0.1_f32), "0.1");
        assert_eq!(float_repr(0.70710677_f32), "0.70710677");
        assert_eq!(float_repr(-0.0_f32), "-0.0");
        assert_eq!(float_repr(f32::NAN), "nan");
        assert_eq!(float_repr(f32::MAX), "3.4028235e+38");
    }
}
