use std::{collections::HashMap, sync::OnceLock};

use bevy::ecs::world::World;
use pybevy_core::{CustomComponentInfo, CustomResourceInfo, ValidityFlag, ValidityGuard};
use pyo3::{
    exceptions::PyRuntimeError,
    ffi::{PyObject, PyTypeObject},
    prelude::*,
    types::{PyDict, PyModule, PyType},
};

use crate::bridge::ControlError;

/// Function that creates a Python World wrapper from a raw world pointer and validity flag.
/// Registered by the root pybevy crate which has access to PyWorld.
type CreateWorldWrapperFn = fn(*mut World, ValidityFlag, Python) -> PyResult<Py<PyAny>>;

/// Global hook for creating PyWorld wrappers (registered by root pybevy crate)
static WORLD_WRAPPER_HOOK: OnceLock<CreateWorldWrapperFn> = OnceLock::new();

const RUN_CODE_FILENAME: &str = "<pybevy run_code>";

struct StreamRestore<'py> {
    sys: Bound<'py, PyModule>,
    stdout: Bound<'py, PyAny>,
    stderr: Bound<'py, PyAny>,
}

impl Drop for StreamRestore<'_> {
    fn drop(&mut self) {
        let _ = self.sys.setattr("stdout", &self.stdout);
        let _ = self.sys.setattr("stderr", &self.stderr);
    }
}

fn format_python_error(py: Python<'_>, error: &PyErr) -> String {
    let formatted = py.import("traceback").and_then(|traceback| {
        traceback.call_method1(
            "format_exception",
            (error.get_type(py), error.value(py), error.traceback(py)),
        )
    });
    formatted
        .and_then(|lines| lines.extract::<Vec<String>>())
        .map(|lines| lines.concat())
        .unwrap_or_else(|_| error.to_string())
}

/// Register the world wrapper factory function.
/// Called by the root pybevy crate during initialization.
pub fn register_world_wrapper_hook(f: CreateWorldWrapperFn) {
    let _ = WORLD_WRAPPER_HOOK.set(f);
}

fn inject_registered_custom_types(
    world: &World,
    py: Python<'_>,
    globals: &Bound<'_, PyDict>,
) -> PyResult<()> {
    let mut entries = Vec::new();
    if let Some(info) = world.get_resource::<CustomComponentInfo>() {
        entries.extend(
            info.iter()
                .map(|(_, entry)| (entry.name.clone(), entry.type_ptr)),
        );
    }
    if let Some(info) = world.get_resource::<CustomResourceInfo>() {
        entries.extend(
            info.iter()
                .map(|(_, entry)| (entry.name.clone(), entry.type_ptr)),
        );
    }

    let mut by_name: HashMap<String, Vec<*const PyTypeObject>> = HashMap::new();
    for (name, type_ptr) in entries {
        let pointers = by_name.entry(name).or_default();
        if !pointers.contains(&type_ptr) {
            pointers.push(type_ptr);
        }
    }

    let modules = py
        .import("sys")?
        .getattr("modules")?
        .cast_into::<PyDict>()?;

    for (name, pointers) in by_name {
        // A bare name cannot select between unrelated scene types. Leave an
        // ambiguous name unresolved instead of injecting a nondeterministic class.
        if pointers.len() != 1 || name.starts_with("__") {
            continue;
        }

        // Registry pointers are opaque identity tokens only. Resolve the class
        // through a live module dictionary, which owns a strong Python reference.
        // A removed or renamed hot-reload class has no live match and is skipped.
        let type_ptr = pointers[0].cast::<PyObject>();
        for module in modules.values() {
            let Ok(module) = module.cast::<PyModule>() else {
                continue;
            };
            let Some(candidate) = module.dict().get_item(&name)? else {
                continue;
            };
            if candidate.is_instance_of::<PyType>() && candidate.as_ptr() == type_ptr.cast_mut() {
                globals.set_item(&name, candidate)?;
                break;
            }
        }
    }
    Ok(())
}

/// Create the root adapter's Python `World` wrapper for another PyO3 control
/// handler.
///
/// Keeping this behind the existing hook lets control operations reuse the
/// main adapter's component insertion path without making `pybevy_control`
/// depend on the root crate (which would create a dependency cycle).
pub(super) fn create_world_wrapper(
    world: &mut World,
    validity: ValidityFlag,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    let create_wrapper = WORLD_WRAPPER_HOOK
        .get()
        .ok_or_else(|| PyRuntimeError::new_err("PyBevy World adapter is not registered"))?;
    create_wrapper(world as *mut World, validity, py)
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

        inject_registered_custom_types(world, py, &globals).map_err(|e| {
            ControlError::internal(format!("Failed to inject registered scene types: {e}"))
        })?;

        // Inject world if hook is registered
        if WORLD_WRAPPER_HOOK.get().is_some() {
            match create_world_wrapper(world, validity.clone(), py) {
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

        let io = py
            .import("io")
            .map_err(|error| ControlError::internal(format!("Failed to import io: {error}")))?;
        let sys = py
            .import("sys")
            .map_err(|error| ControlError::internal(format!("Failed to import sys: {error}")))?;
        let stdout_capture = io
            .getattr("StringIO")
            .and_then(|string_io| string_io.call0())
            .map_err(|error| {
                ControlError::internal(format!("Failed to create stdout capture: {error}"))
            })?;
        let stderr_capture = io
            .getattr("StringIO")
            .and_then(|string_io| string_io.call0())
            .map_err(|error| {
                ControlError::internal(format!("Failed to create stderr capture: {error}"))
            })?;
        let restore = StreamRestore {
            stdout: sys.getattr("stdout").map_err(|error| {
                ControlError::internal(format!("Failed to read sys.stdout: {error}"))
            })?,
            stderr: sys.getattr("stderr").map_err(|error| {
                ControlError::internal(format!("Failed to read sys.stderr: {error}"))
            })?,
            sys: sys.clone(),
        };
        sys.setattr("stdout", &stdout_capture).map_err(|error| {
            ControlError::internal(format!("Failed to redirect sys.stdout: {error}"))
        })?;
        sys.setattr("stderr", &stderr_capture).map_err(|error| {
            ControlError::internal(format!("Failed to redirect sys.stderr: {error}"))
        })?;

        // Compile the caller's source as-is. In particular, do not embed it in
        // another Python string, which would interpret its backslash escapes a
        // second time and offset every traceback line by the wrapper source.
        let execution = py.import("builtins").and_then(|builtins| {
            let compiled =
                builtins.call_method1("compile", (code.as_str(), RUN_CODE_FILENAME, "exec"))?;
            builtins
                .call_method1("exec", (compiled, &globals, &globals))
                .map(|_| ())
        });

        let stdout = stdout_capture
            .call_method0("getvalue")
            .and_then(|value| value.extract::<String>())
            .unwrap_or_default();
        let stderr = stderr_capture
            .call_method0("getvalue")
            .and_then(|value| value.extract::<String>())
            .unwrap_or_default();
        let error = execution.err().map(|error| format_python_error(py, &error));
        drop(restore);

        Ok(serde_json::json!({
            "stdout": stdout,
            "stderr": stderr,
            "error": error,
            "success": error.is_none(),
        }))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use super::*;

    static INIT: Once = Once::new();
    fn init_python() {
        INIT.call_once(|| {
            pyo3::Python::initialize();
        });
    }

    #[test]
    fn execute_simple_code() {
        init_python();
        let mut world = World::new();
        let result = execute_python(&mut world, "x = 1 + 1".to_string()).unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["stdout"], "");
        assert!(result["error"].is_null());
    }

    #[test]
    fn execute_with_print_captures_stdout() {
        init_python();
        let mut world = World::new();
        let result = execute_python(&mut world, "print('hello world')".to_string()).unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["stdout"], "hello world\n");
    }

    #[test]
    fn execute_with_error_captures_traceback() {
        init_python();
        let mut world = World::new();
        let result =
            execute_python(&mut world, "raise ValueError('test error')".to_string()).unwrap();
        assert_eq!(result["success"], false);
        let error = result["error"].as_str().unwrap();
        assert!(error.contains("ValueError"));
        assert!(error.contains("test error"));
    }

    #[test]
    fn execute_preserves_source_escapes_triple_quotes_and_unicode() {
        init_python();
        let mut world = World::new();
        let code = r#"print(repr("\n"))
print(repr("\t"))
print(b"\x00".hex())
print("""triple ' quotes""")
print("héllø 🦀")"#;

        let result = execute_python(&mut world, code.to_string()).unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(
            result["stdout"],
            "'\\n'\n'\\t'\n00\ntriple ' quotes\nhéllø 🦀\n"
        );
        assert!(result["error"].is_null());
    }

    #[test]
    fn execute_runtime_traceback_uses_caller_filename_and_line() {
        init_python();
        let mut world = World::new();
        let code = "first = 1\nsecond = 2\nraise RuntimeError('boom')";

        let result = execute_python(&mut world, code.to_string()).unwrap();

        assert_eq!(result["success"], false);
        let error = result["error"].as_str().unwrap();
        assert!(error.contains("File \"<pybevy run_code>\", line 3"));
        assert!(error.contains("RuntimeError: boom"));
    }

    #[test]
    fn execute_with_syntax_error() {
        init_python();
        let mut world = World::new();
        let result = execute_python(
            &mut world,
            "first = 1\nif True print('invalid')".to_string(),
        )
        .unwrap();
        assert_eq!(result["success"], false);
        let error = result["error"].as_str().unwrap();
        assert!(error.contains("File \"<pybevy run_code>\", line 2"));
        assert!(error.contains("SyntaxError"));
    }

    #[test]
    fn execute_reports_an_embedded_null_as_a_python_syntax_error() {
        init_python();
        let mut world = World::new();
        let code = format!("print(1){}print(2)", '\0');

        let result = execute_python(&mut world, code).unwrap();

        assert_eq!(result["success"], false);
        let error = result["error"].as_str().unwrap();
        assert!(error.contains("SyntaxError"));
        assert!(error.contains("null bytes"));
    }

    #[test]
    fn execute_empty_code() {
        init_python();
        let mut world = World::new();
        let result = execute_python(&mut world, String::new()).unwrap();
        assert_eq!(result["success"], true);
    }
}
