use pybevy_core::public_error;
use pyo3::{
    prelude::*,
    types::{PyDict, PyModule, PyTuple, PyType},
};

use crate::{
    codec::value::json_kind_name,
    engine::dispatch::LINEAR_RGBA_CHANNELS,
    pyo3::from_json::{
        enums::reject_enum_constant,
        field::{json_number_to_f32, json_to_py},
    },
};

pub(crate) fn construct_color_variant(
    py: Python<'_>,
    variant_name: &str,
    variant_value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    let color_module = PyModule::import(py, "pybevy.color").map_err(|error| error.to_string())?;
    let color = color_module
        .getattr("Color")
        .map_err(|error| error.to_string())?;
    if let Ok(color_type) = color.cast::<PyType>() {
        reject_enum_constant(color_type, variant_name)?;
    }
    let variant = color
        .getattr(variant_name)
        .map_err(|_| format!("unknown Color variant '{variant_name}'"))?;
    let payload_type = color_module
        .getattr(variant_name)
        .map_err(|_| format!("Color variant '{variant_name}' has no payload type"))?;

    let payload = match variant_value {
        serde_json::Value::Object(fields) => {
            let kwargs = PyDict::new(py);
            for (name, value) in fields {
                kwargs
                    .set_item(name, json_to_py(py, value)?)
                    .map_err(|error| error.to_string())?;
            }
            payload_type
                .call((), Some(&kwargs))
                .map_err(|error| format!("invalid payload for Color.{variant_name}: {error}"))?
        }
        serde_json::Value::Array(values) => {
            let args = values
                .iter()
                .map(|value| json_to_py(py, value))
                .collect::<Result<Vec<_>, _>>()?;
            let args = PyTuple::new(py, args).map_err(|error| error.to_string())?;
            payload_type
                .call1(args)
                .map_err(|error| format!("invalid payload for Color.{variant_name}: {error}"))?
        }
        _ => {
            return Err(format!(
                "Color.{variant_name} expects an object or array payload"
            ));
        }
    };

    variant
        .call1((payload,))
        .map(Bound::unbind)
        .map_err(|error| format!("invalid payload for Color.{variant_name}: {error}"))
}

pub(crate) fn linear_rgba_from_json(
    py: Python<'_>,
    current: &Bound<'_, PyAny>,
    value: &serde_json::Value,
) -> Result<Py<PyAny>, String> {
    let class = current.get_type();
    match value {
        serde_json::Value::Array(values) => {
            if values.len() != LINEAR_RGBA_CHANNELS.len() {
                return Err(public_error::mcp_linear_rgba_input(format!(
                    "LinearRgba requires exactly 4 channels, got {} elements",
                    values.len()
                )));
            }
            let channels = LINEAR_RGBA_CHANNELS
                .iter()
                .zip(values)
                .map(|(name, value)| {
                    json_number_to_f32(value).map_err(|error| {
                        public_error::mcp_linear_rgba_input(format!("LinearRgba.{name}: {error}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            class
                .call1((channels[0], channels[1], channels[2], channels[3]))
                .map(Bound::unbind)
                .map_err(|error| error.to_string())
        }
        serde_json::Value::Object(fields) => {
            if fields.is_empty() {
                return Ok(current.clone().unbind());
            }
            let kwargs = PyDict::new(py);
            for (name, value) in fields {
                if !LINEAR_RGBA_CHANNELS.contains(&name.as_str()) {
                    return Err(public_error::mcp_linear_rgba_input(format!(
                        "LinearRgba has no field '{name}'"
                    )));
                }
                let channel = json_number_to_f32(value).map_err(|error| {
                    public_error::mcp_linear_rgba_input(format!("LinearRgba.{name}: {error}"))
                })?;
                kwargs
                    .set_item(name, channel)
                    .map_err(|error| error.to_string())?;
            }
            class
                .call((), Some(&kwargs))
                .map(Bound::unbind)
                .map_err(|error| error.to_string())
        }
        _ => Err(public_error::mcp_linear_rgba_input(format!(
            "LinearRgba got {}",
            json_kind_name(value)
        ))),
    }
}
