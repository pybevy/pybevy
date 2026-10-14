use std::path::{Path, PathBuf};

use pyo3::prelude::*;

use crate::discovery::ApiIndex;

/// Python-accessible wrapper around ApiIndex
#[pyclass(name = "ApiIndex", module = "pybevy.mcp")]
pub struct PyApiIndex {
    inner: ApiIndex,
}

#[pymethods]
impl PyApiIndex {
    #[new]
    #[pyo3(signature = (pybevy_dir = None))]
    fn new(py: Python<'_>, pybevy_dir: Option<String>) -> Self {
        let dir = match pybevy_dir {
            Some(d) => Some(PathBuf::from(d)),
            None => {
                // Try Python import path first (works for pip-installed packages)
                py.import("pybevy")
                    .ok()
                    .and_then(|m| m.getattr("__path__").ok())
                    .and_then(|p| p.get_item(0).ok())
                    .and_then(|s| s.extract::<String>().ok())
                    .map(PathBuf::from)
                    .filter(|p| p.is_dir())
                    // Fall back to filesystem search (works in dev repos)
                    .or_else(ApiIndex::find_pybevy_dir)
            }
        };
        let index = match dir {
            Some(d) => ApiIndex::build(&d),
            None => ApiIndex::build(Path::new("")),
        };
        PyApiIndex { inner: index }
    }

    /// Search .pyi stub files for a pattern. Returns JSON string with
    /// `{ "results": [...], "total": N, "truncated": bool }`. When `limit`
    /// is omitted the full result set is returned.
    #[pyo3(signature = (query, limit = None))]
    fn search(&self, query: &str, limit: Option<usize>) -> String {
        let (results, total) = self.inner.search(query, limit);
        let truncated = results.len() < total;
        let payload = serde_json::json!({
            "results": results,
            "total": total,
            "truncated": truncated,
        });
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    }

    /// Get the full class definition from stubs. Returns None if not found.
    fn get_type_definition(&self, type_name: &str) -> Option<String> {
        self.inner.get_type_definition(type_name)
    }

    /// Get a structured type definition as JSON string. Returns None if not found.
    fn get_type_definition_structured(&self, type_name: &str) -> Option<String> {
        self.inner
            .get_type_definition_structured(type_name)
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
    }

    /// Near-miss qualified names for a type query that did not resolve.
    fn suggest_type_names(&self, type_name: &str) -> Vec<String> {
        self.inner.suggest_type_names(type_name)
    }

    /// Get the lightweight API index as JSON string.
    fn get_index(&self) -> String {
        serde_json::to_string_pretty(&self.inner.get_index()).unwrap_or_default()
    }

    /// Get the full .pyi content for a module.
    fn get_module_content(&self, module_name: &str) -> Option<String> {
        self.inner.get_module_content(module_name).cloned()
    }

    /// Get the guide index as JSON string.
    fn get_guide_index(&self) -> String {
        serde_json::to_string_pretty(&self.inner.get_guide_index()).unwrap_or_default()
    }

    /// Get the full markdown content of a guide.
    fn get_guide(&self, name: &str) -> Option<String> {
        self.inner.get_guide(name).map(|s| s.to_string())
    }

    /// Get the default MCP instructions loaded from mcp/instructions.md.
    fn get_instructions(&self) -> Option<String> {
        self.inner.get_instructions().map(|s| s.to_string())
    }
}
