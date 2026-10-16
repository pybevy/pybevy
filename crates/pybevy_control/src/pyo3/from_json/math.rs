use pybevy_core::public_error;
use pybevy_math::{quat::PyQuat, range::PyRange};
use pyo3::{
    prelude::*,
    types::{PyModule, PyTuple},
};

use crate::{
    codec::{
        json_float::nonfinite_float_from_json,
        value::{self as value_conversion},
    },
    pyo3::from_json::field::json_number_to_f32,
};

pub(crate) fn math_readback_array(
    current: &Bound<'_, PyAny>,
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    let fields = value.as_object()?;
    let type_name = current.get_type().name().ok()?;
    let coordinates: &[&str] = match type_name.to_string_lossy().as_ref() {
        "Vec2" | "UVec2" | "IVec2" => &["x", "y"],
        "UVec3" => &["x", "y", "z"],
        "Vec4" | "Quat" => &["x", "y", "z", "w"],
        _ => return None,
    };
    if fields.len() != coordinates.len() {
        return None;
    }
    coordinates
        .iter()
        .map(|name| fields.get(*name).cloned())
        .collect::<Option<Vec<_>>>()
        .map(serde_json::Value::Array)
}

pub(crate) fn math_value_from_json(
    py: Python<'_>,
    type_name: &str,
    values: &[serde_json::Value],
) -> Option<Result<Py<PyAny>, String>> {
    let count = match type_name {
        "Vec2" | "UVec2" | "IVec2" => 2,
        "Vec3" | "UVec3" => 3,
        "Vec4" | "Quat" => 4,
        _ => return None,
    };
    if values.len() != count {
        return None;
    }
    if type_name == "Quat" {
        return Some(
            value_conversion::unit_quat_from_json(&serde_json::Value::Array(values.to_vec()))
                .and_then(|quat| {
                    Py::new(py, PyQuat::from_quat(quat))
                        .map(Py::into_any)
                        .map_err(|error| error.to_string())
                }),
        );
    }
    let integral = matches!(type_name, "UVec2" | "IVec2" | "UVec3");
    let mut coordinates = Vec::with_capacity(count);
    for value in values {
        let coordinate = if integral {
            integer_vector_coordinate_to_py(py, type_name, value)
        } else {
            json_number_to_f32(value).and_then(|number| {
                number
                    .into_pyobject(py)
                    .map(|number| number.into_any().unbind())
                    .map_err(|error| error.to_string())
            })
        };
        match coordinate {
            Ok(coordinate) => coordinates.push(coordinate),
            Err(error) => return Some(Err(error)),
        }
    }
    Some((|| -> Result<Py<PyAny>, String> {
        let math = PyModule::import(py, "pybevy.math").map_err(|error| error.to_string())?;
        let class = math.getattr(type_name).map_err(|error| error.to_string())?;
        let args = PyTuple::new(py, coordinates).map_err(|error| error.to_string())?;
        class
            .call1(args)
            .map(Bound::unbind)
            .map_err(|error| error.to_string())
    })())
}

pub(crate) fn range_from_json(
    py: Python<'_>,
    current: &Bound<'_, PyRange>,
    value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    let fields = value
        .as_object()
        .ok_or_else(|| "Range expects an object with start and end".to_string())?;
    let mut start = current
        .getattr("start")
        .and_then(|value| value.extract::<f32>())
        .map_err(|error| error.to_string())?;
    let mut end = current
        .getattr("end")
        .and_then(|value| value.extract::<f32>())
        .map_err(|error| error.to_string())?;
    for (name, value) in fields {
        let endpoint = match name.as_str() {
            "start" => &mut start,
            "end" => &mut end,
            _ => return Err(format!("Range has no field '{name}'")),
        };
        *endpoint = if let Some(nonfinite) = nonfinite_float_from_json(value)
            && nonfinite.is_infinite()
        {
            nonfinite as f32
        } else {
            json_number_to_f32(value).map_err(|error| format!("{name}: {error}"))?
        };
    }
    Py::new(py, PyRange::from_range(start, end))
        .map(Py::into_any)
        .map_err(|error| error.to_string())
}

fn integer_vector_coordinate_to_py(
    py: Python<'_>,
    type_name: &str,
    value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    match type_name {
        "UVec2" | "UVec3" => {
            let coordinate = if let Some(integer) = value.as_i64() {
                u32::try_from(integer)
            } else if let Some(integer) = value.as_u64() {
                u32::try_from(integer)
            } else {
                return Err(public_error::integer_vector_coordinate(type_name, value));
            }
            .map_err(|_| {
                public_error::integer_vector_coordinate_out_of_range(
                    type_name,
                    value,
                    u32::MIN,
                    u32::MAX,
                )
            })?;
            coordinate
                .into_pyobject(py)
                .map(|value| value.into_any().unbind())
                .map_err(|error| error.to_string())
        }
        "IVec2" => {
            let Some(integer) = value.as_i64() else {
                if value.as_u64().is_some() {
                    return Err(public_error::integer_vector_coordinate_out_of_range(
                        type_name,
                        value,
                        i32::MIN,
                        i32::MAX,
                    ));
                }
                return Err(public_error::integer_vector_coordinate(type_name, value));
            };
            let coordinate = i32::try_from(integer).map_err(|_| {
                public_error::integer_vector_coordinate_out_of_range(
                    type_name,
                    value,
                    i32::MIN,
                    i32::MAX,
                )
            })?;
            coordinate
                .into_pyobject(py)
                .map(|value| value.into_any().unbind())
                .map_err(|error| error.to_string())
        }
        _ => unreachable!("integer vector conversion requires a known integer vector type"),
    }
}
