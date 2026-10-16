// Parts of the codec serve only the PyO3 adapter.
#![cfg_attr(not(feature = "pyo3-runtime"), allow(dead_code))]

pub(crate) mod enum_payload;
pub(crate) mod json_float;
pub(crate) mod math_shape;
pub mod reflect;
pub(crate) mod value;
