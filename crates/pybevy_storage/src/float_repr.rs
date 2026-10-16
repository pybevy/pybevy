//! Python-compatible float formatting for user-facing text.

use std::{fmt::LowerExp, iter::repeat_n};

/// Format a float the way Python's `repr(float)` does.
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
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
