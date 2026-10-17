use std::time::Duration;

use pyo3::{
    create_exception,
    exceptions::{PyException, PyRuntimeError, PyValueError},
    prelude::*,
    types::PyType,
};
use serde_json::Value;

use crate::transport::client::{self, ClientError, ClientErrorKind};

create_exception!(
    pybevy_control,
    PyControlTimeoutError,
    PyException,
    "The loopback control request exceeded its deadline."
);
create_exception!(
    pybevy_control,
    PyControlConnectError,
    PyException,
    "The loopback control server could not be reached."
);
create_exception!(
    pybevy_control,
    PyControlHttpStatusError,
    PyException,
    "The loopback control server returned a non-success status."
);
create_exception!(
    pybevy_control,
    PyControlProtocolError,
    PyException,
    "The loopback control server returned an invalid response."
);
create_exception!(
    pybevy_control,
    PyControlTransportError,
    PyException,
    "The loopback control request failed during transport."
);

#[pyfunction]
fn _control_request_tool(
    py: Python<'_>,
    port: u16,
    tool_name: &str,
    arguments: &Bound<'_, PyAny>,
    timeout: f64,
) -> PyResult<Py<PyAny>> {
    let arguments = python_to_json(py, arguments)?;
    let timeout = timeout_duration(timeout)?;
    let tool_name = tool_name.to_string();
    let result = py
        .detach(move || client::request_tool(port, &tool_name, arguments, timeout))
        .map_err(client_error_to_py)?;
    json_to_python(py, &result)
}

#[pyfunction]
fn _control_request_scene_resource(
    py: Python<'_>,
    port: u16,
    uri: &str,
    timeout: f64,
) -> PyResult<Py<PyAny>> {
    let timeout = timeout_duration(timeout)?;
    let uri = uri.to_string();
    let result = py
        .detach(move || client::request_scene_resource(port, &uri, timeout))
        .map_err(client_error_to_py)?;
    json_to_python(py, &result)
}

#[pyfunction]
fn _control_health(py: Python<'_>, port: u16, timeout: f64) -> PyResult<bool> {
    let timeout = timeout_duration(timeout)?;
    py.detach(move || client::control_health(port, timeout))
        .map_err(client_error_to_py)
}

#[pyfunction]
#[pyo3(signature = (port, token))]
fn _control_set_session_token(port: u16, token: Option<String>) {
    client::set_session_token(port, token);
}

#[pyfunction]
fn _control_engine_activity(py: Python<'_>, port: u16, timeout: f64) -> PyResult<Py<PyAny>> {
    let timeout = timeout_duration(timeout)?;
    let result = py
        .detach(move || client::control_engine_activity(port, timeout))
        .map_err(client_error_to_py)?;
    json_to_python(py, &result)
}

#[pyfunction]
fn _control_last_error(py: Python<'_>, port: u16, timeout: f64) -> PyResult<Py<PyAny>> {
    let timeout = timeout_duration(timeout)?;
    let result = py
        .detach(move || client::control_last_error(port, timeout))
        .map_err(client_error_to_py)?;
    json_to_python(py, &result)
}

pub fn add_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    add_control_exception(
        module,
        "_ControlTimeoutError",
        module.py().get_type::<PyControlTimeoutError>(),
    )?;
    add_control_exception(
        module,
        "_ControlConnectError",
        module.py().get_type::<PyControlConnectError>(),
    )?;
    add_control_exception(
        module,
        "_ControlHttpStatusError",
        module.py().get_type::<PyControlHttpStatusError>(),
    )?;
    add_control_exception(
        module,
        "_ControlProtocolError",
        module.py().get_type::<PyControlProtocolError>(),
    )?;
    add_control_exception(
        module,
        "_ControlTransportError",
        module.py().get_type::<PyControlTransportError>(),
    )?;
    module.add_function(wrap_pyfunction!(_control_request_tool, module)?)?;
    module.add_function(wrap_pyfunction!(_control_request_scene_resource, module)?)?;
    module.add_function(wrap_pyfunction!(_control_health, module)?)?;
    module.add_function(wrap_pyfunction!(_control_last_error, module)?)?;
    module.add_function(wrap_pyfunction!(_control_engine_activity, module)?)?;
    module.add_function(wrap_pyfunction!(_control_set_session_token, module)?)?;
    Ok(())
}

// pybevy.mcp is this native module, so pickle and tracebacks resolve the public name.
fn add_control_exception(
    module: &Bound<'_, PyModule>,
    public_name: &str,
    exception_type: Bound<'_, PyType>,
) -> PyResult<()> {
    exception_type.setattr("__module__", "pybevy.mcp")?;
    exception_type.setattr("__name__", public_name)?;
    exception_type.setattr("__qualname__", public_name)?;
    module.add(public_name, exception_type)
}

fn timeout_duration(timeout: f64) -> PyResult<Duration> {
    if !timeout.is_finite() || timeout <= 0.0 {
        return Err(PyValueError::new_err(
            "timeout must be a positive finite number of seconds",
        ));
    }
    Duration::try_from_secs_f64(timeout)
        .map_err(|_| PyValueError::new_err("timeout is outside the supported range"))
}

fn python_to_json(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    let json_module = py.import("json")?;
    let encoded: String = json_module.call_method1("dumps", (value,))?.extract()?;
    serde_json::from_str(&encoded)
        .map_err(|error| PyValueError::new_err(format!("invalid JSON arguments: {error}")))
}

fn json_to_python(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    let encoded = serde_json::to_string(value)
        .map_err(|error| PyRuntimeError::new_err(format!("failed to encode JSON: {error}")))?;
    Ok(py
        .import("json")?
        .call_method1("loads", (encoded,))?
        .unbind())
}

fn client_error_to_py(error: ClientError) -> PyErr {
    let message = error.to_string();
    match error.kind() {
        ClientErrorKind::Timeout => PyControlTimeoutError::new_err(message),
        ClientErrorKind::Connect => PyControlConnectError::new_err(message),
        ClientErrorKind::HttpStatus => PyControlHttpStatusError::new_err(message),
        ClientErrorKind::Protocol => PyControlProtocolError::new_err(message),
        ClientErrorKind::Validation => PyValueError::new_err(message),
        ClientErrorKind::Transport => PyControlTransportError::new_err(message),
    }
}
