use std::sync::OnceLock;

use bevy::ecs::world::World;
use pybevy_core::{ValidityFlag, ValidityGuard};
use pyo3::prelude::*;

use crate::bridge::ControlError;

/// Function that creates a Python World wrapper from a raw world pointer and validity flag.
/// Registered by the root pybevy crate which has access to PyWorld.
type CreateWorldWrapperFn = fn(*mut World, ValidityFlag, Python) -> PyResult<Py<PyAny>>;

/// Global hook for creating PyWorld wrappers (registered by root pybevy crate)
static WORLD_WRAPPER_HOOK: OnceLock<CreateWorldWrapperFn> = OnceLock::new();

/// Register the world wrapper factory function.
/// Called by the root pybevy crate during initialization.
pub fn register_world_wrapper_hook(f: CreateWorldWrapperFn) {
    let _ = WORLD_WRAPPER_HOOK.set(f);
}

/// Execute arbitrary Python code with stdout/stderr capture and world access
pub fn execute_python(world: &mut World, code: String) -> Result<serde_json::Value, ControlError> {
    Python::attach(|py| {
        // Create validity flag and guard for borrowed world access
        let validity = ValidityFlag::new();
        let _guard = ValidityGuard::new(validity.clone());

        // Build the globals dict from __main__ and inject world
        let globals = py
            .import("__main__")
            .and_then(|m| m.dict().copy())
            .map_err(|e| ControlError::internal(format!("Failed to create globals: {e}")))?;

        // Inject world if hook is registered
        if let Some(create_wrapper) = WORLD_WRAPPER_HOOK.get() {
            let world_ptr = world as *mut World;
            match create_wrapper(world_ptr, validity.clone(), py) {
                Ok(world_obj) => {
                    if let Err(e) = globals.set_item("world", world_obj) {
                        return Ok(serde_json::json!({
                            "stdout": "",
                            "stderr": "",
                            "error": format!("Failed to inject world: {e}"),
                            "success": false,
                        }));
                    }
                }
                Err(e) => {
                    return Ok(serde_json::json!({
                        "stdout": "",
                        "stderr": "",
                        "error": format!("Failed to create world wrapper: {e}"),
                        "success": false,
                    }));
                }
            }
        }

        // Set up stdout/stderr capture and execute user code
        let capture_code = format!(
            r#"
import io, sys
_mcp_stdout = io.StringIO()
_mcp_stderr = io.StringIO()
_mcp_old_stdout = sys.stdout
_mcp_old_stderr = sys.stderr
sys.stdout = _mcp_stdout
sys.stderr = _mcp_stderr
_mcp_result = None
_mcp_error = None
try:
    exec('''{escaped_code}''', globals())
except Exception as e:
    import traceback
    _mcp_error = traceback.format_exc()
finally:
    sys.stdout = _mcp_old_stdout
    sys.stderr = _mcp_old_stderr
"#,
            escaped_code = code.replace('\'', "\\'")
        );

        let c_code = std::ffi::CString::new(capture_code)
            .map_err(|e| ControlError::invalid_params(format!("Invalid code: {e}")))?;

        match py.run(&c_code, Some(&globals), None) {
            Ok(()) => {
                let stdout = py
                    .eval(c"_mcp_stdout.getvalue()", Some(&globals), None)
                    .and_then(|v| v.extract::<String>())
                    .unwrap_or_default();
                let stderr = py
                    .eval(c"_mcp_stderr.getvalue()", Some(&globals), None)
                    .and_then(|v| v.extract::<String>())
                    .unwrap_or_default();
                let error = py
                    .eval(c"_mcp_error", Some(&globals), None)
                    .and_then(|v| v.extract::<Option<String>>())
                    .unwrap_or(None);

                Ok(serde_json::json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "error": error,
                    "success": error.is_none(),
                }))
            }
            Err(e) => Ok(serde_json::json!({
                "stdout": "",
                "stderr": "",
                "error": format!("{e}"),
                "success": false,
            })),
        }
    })
}
