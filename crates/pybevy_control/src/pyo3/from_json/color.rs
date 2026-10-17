use pybevy_core::public_error;
use pyo3::{
    prelude::*,
    types::{PyDict, PyModule, PyTuple, PyType},
};

use crate::{
    codec::value::{LINEAR_RGBA_CHANNELS, json_f32, json_kind_name},
    pyo3::from_json::{
        enums::{enum_variant_names, inline_json, reject_enum_constant},
        field::json_to_py,
    },
};

/// `Color.srgba(r, g, b, a)` from a four-element JSON array.
pub(crate) fn srgba_from_array(
    color: &Bound<'_, PyType>,
    values: &[serde_json::Value],
) -> Result<Py<PyAny>, String> {
    let [red, green, blue, alpha] = values else {
        return Err(format!(
            "Color expects [r, g, b, a], got {} elements",
            values.len()
        ));
    };
    color
        .call_method1(
            "srgba",
            (
                json_f32(red)?,
                json_f32(green)?,
                json_f32(blue)?,
                json_f32(alpha)?,
            ),
        )
        .map(Bound::unbind)
        .map_err(|error| error.to_string())
}

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
        let variants = enum_variant_names(color_type);
        if !variants.iter().any(|variant| variant == variant_name) {
            return Err(public_error::mcp_unknown_variant(
                variant_name,
                "Color",
                &variants,
            ));
        }
    }
    let variant = color
        .getattr(variant_name)
        .map_err(|error| error.to_string())?;
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

/// Names the accepted Color JSON shapes and its variants.
pub(crate) fn color_shape_error(py: Python<'_>, got: &serde_json::Value) -> String {
    let variants = PyModule::import(py, "pybevy.color")
        .and_then(|module| module.getattr("Color"))
        .ok()
        .and_then(|color| color.cast_into::<PyType>().ok())
        .map(|color| enum_variant_names(&color))
        .unwrap_or_default();
    public_error::mcp_color_shape(&inline_json(got), &variants)
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
                    json_f32(value).map_err(|error| {
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
                let channel = json_f32(value).map_err(|error| {
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
