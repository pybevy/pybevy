use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use pyo3::prelude::*;
use serde::Serialize;

/// Lightweight API index: module names -> class/function lists
#[derive(Debug, Serialize, Clone)]
pub struct ApiIndexEntry {
    pub module: String,
    pub classes: Vec<String>,
    pub functions: Vec<String>,
}

/// Lightweight guide index entry
#[derive(Debug, Serialize, Clone)]
pub struct GuideEntry {
    pub name: String,
    pub title: String,
    pub description: String,
}

/// Pre-built API index from .pyi stub files (with .py fallback)
pub struct ApiIndex {
    /// Module name -> index entry
    entries: Vec<ApiIndexEntry>,
    /// Module name -> full file content (.pyi preferred, .py fallback)
    contents: HashMap<String, String>,
    /// Guide entries (name, title, description)
    guides: Vec<GuideEntry>,
    /// Guide name -> full markdown content
    guide_contents: HashMap<String, String>,
    /// Default MCP instructions loaded from mcp/instructions.md
    instructions: Option<String>,
}

impl ApiIndex {
    /// Build the API index by scanning the pybevy/ directory for .pyi and .py files
    pub fn build(pybevy_dir: &Path) -> Self {
        let mut entries = Vec::new();
        let mut contents = HashMap::new();

        // Collect .pyi files (preferred) and .py fallbacks
        let mut pyi_files: Vec<(String, PathBuf)> = Vec::new();
        let mut py_fallbacks: Vec<(String, PathBuf)> = Vec::new();

        if let Ok(read_dir) = std::fs::read_dir(pybevy_dir) {
            let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
            dir_entries.sort_by_key(|e| e.file_name());

            for entry in dir_entries {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str());
                    let stem = path.file_stem().and_then(|s| s.to_str());
                    match (ext, stem) {
                        (Some("pyi"), Some(name)) => {
                            pyi_files.push((name.to_string(), path));
                        }
                        (Some("py"), Some(name)) if name != "__init__" => {
                            py_fallbacks.push((name.to_string(), path));
                        }
                        _ => {}
                    }
                } else if path.is_dir() {
                    // Scan one level of subdirectory (e.g. particle/, physics/, contrib/)
                    let subdir_name = entry.file_name();
                    let Some(subdir_str) = subdir_name.to_str() else {
                        continue;
                    };
                    if let Ok(sub_read) = std::fs::read_dir(&path) {
                        let mut sub_entries: Vec<_> = sub_read.filter_map(|e| e.ok()).collect();
                        sub_entries.sort_by_key(|e| e.file_name());
                        for sub_entry in sub_entries {
                            let sub_path = sub_entry.path();
                            if sub_path.is_file() {
                                let ext = sub_path.extension().and_then(|e| e.to_str());
                                let stem = sub_path.file_stem().and_then(|s| s.to_str());
                                match (ext, stem) {
                                    (Some("pyi"), Some(name)) => {
                                        let module_name = format!("{subdir_str}.{name}");
                                        pyi_files.push((module_name, sub_path));
                                    }
                                    (Some("py"), Some(name)) if name != "__init__" => {
                                        let module_name = format!("{subdir_str}.{name}");
                                        py_fallbacks.push((module_name, sub_path));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate: if a module has both .pyi and .py, keep only .pyi
        let pyi_modules: std::collections::HashSet<String> =
            pyi_files.iter().map(|(n, _)| n.clone()).collect();
        for (name, path) in py_fallbacks {
            if !pyi_modules.contains(&name) {
                pyi_files.push((name, path));
            }
        }

        for (module_name, path) in pyi_files {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let (classes, functions) = parse_stub_definitions(&content);
                entries.push(ApiIndexEntry {
                    module: module_name.clone(),
                    classes,
                    functions,
                });
                contents.insert(module_name, content);
            }
        }

        // Load guides from pybevy/mcp/guides/
        let (guides, guide_contents) = load_guides(&pybevy_dir.join("mcp/guides"));

        // Load default instructions from pybevy/mcp/instructions.md
        let instructions_path = pybevy_dir.join("mcp/instructions.md");
        let instructions = std::fs::read_to_string(&instructions_path).ok();

        Self {
            entries,
            contents,
            guides,
            guide_contents,
            instructions,
        }
    }

    /// Get the lightweight index (no content)
    pub fn get_index(&self) -> &[ApiIndexEntry] {
        &self.entries
    }

    /// Get the full .pyi content for a module
    pub fn get_module_content(&self, module_name: &str) -> Option<&String> {
        self.contents.get(module_name)
    }

    /// Get the guide index (name, title, description for each guide)
    pub fn get_guide_index(&self) -> &[GuideEntry] {
        &self.guides
    }

    /// Get the full markdown content of a guide
    pub fn get_guide(&self, name: &str) -> Option<&str> {
        self.guide_contents.get(name).map(|s| s.as_str())
    }

    /// Get the default MCP instructions loaded from mcp/instructions.md
    pub fn get_instructions(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// Search for a pattern across all .pyi files
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (module_name, content) in &self.contents {
            for (line_num, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    results.push(SearchResult {
                        module: module_name.clone(),
                        line: line_num + 1,
                        text: line.trim().to_string(),
                    });
                }
            }
        }

        results.truncate(100); // Limit results
        results
    }

    /// Get a single type definition from stubs
    pub fn get_type_definition(&self, type_name: &str) -> Option<String> {
        for content in self.contents.values() {
            if let Some(def) = extract_class_definition(content, type_name) {
                return Some(def);
            }
        }
        None
    }

    /// Get a structured type definition with sections separated for clarity
    pub fn get_type_definition_structured(&self, type_name: &str) -> Option<serde_json::Value> {
        let raw = self.get_type_definition(type_name)?;
        Some(format_class_structured(&raw))
    }

    /// Find the pybevy/ directory relative to the current working directory
    pub fn find_pybevy_dir() -> Option<PathBuf> {
        // Try current directory
        let cwd = std::env::current_dir().ok()?;
        let candidate = cwd.join("pybevy");
        if candidate.is_dir() {
            return Some(candidate);
        }
        // Try parent directories
        let mut dir = cwd.as_path();
        for _ in 0..5 {
            dir = dir.parent()?;
            let candidate = dir.join("pybevy");
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
        None
    }
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub module: String,
    pub line: usize,
    pub text: String,
}

/// Parse a .pyi file to extract class and function names
fn parse_stub_definitions(content: &str) -> (Vec<String>, Vec<String>) {
    let mut classes = Vec::new();
    let mut functions = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some(name) = rest.split(['(', ':']).next() {
                classes.push(name.trim().to_string());
            }
        } else if let Some(rest) = trimmed.strip_prefix("def ") {
            // Only top-level functions (no indentation)
            if !line.starts_with(' ')
                && !line.starts_with('\t')
                && let Some(name) = rest.split('(').next()
            {
                functions.push(name.trim().to_string());
            }
        }
    }

    (classes, functions)
}

/// Load guide markdown files from a directory (including one level of subdirectories).
///
/// Files in subdirectories use a relative path as the guide name:
/// - `guides/camera.md` → name `"camera"`
/// - `guides/recipes/outdoor.md` → name `"recipes/outdoor"`
fn load_guides(guides_dir: &Path) -> (Vec<GuideEntry>, HashMap<String, String>) {
    let mut entries = Vec::new();
    let mut contents = HashMap::new();

    let Ok(read_dir) = std::fs::read_dir(guides_dir) else {
        return (entries, contents);
    };

    let mut dir_entries: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    dir_entries.sort_by_key(|e| e.file_name());

    // Collect all .md files: top-level + one level of subdirectories
    let mut md_files: Vec<(String, PathBuf)> = Vec::new();

    for entry in &dir_entries {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                md_files.push((name.to_string(), path));
            }
        } else if path.is_dir() {
            // Scan one level of subdirectory
            let subdir_name = entry.file_name();
            let Some(subdir_str) = subdir_name.to_str() else {
                continue;
            };
            if let Ok(sub_read) = std::fs::read_dir(&path) {
                let mut sub_entries: Vec<_> = sub_read.filter_map(|e| e.ok()).collect();
                sub_entries.sort_by_key(|e| e.file_name());
                for sub_entry in sub_entries {
                    let sub_path = sub_entry.path();
                    if sub_path.is_file()
                        && sub_path.extension().is_some_and(|ext| ext == "md")
                        && let Some(stem) = sub_path.file_stem().and_then(|s| s.to_str())
                    {
                        let name = format!("{subdir_str}/{stem}");
                        md_files.push((name, sub_path));
                    }
                }
            }
        }
    }

    for (name, path) in md_files {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let (title, description) = parse_guide_header(&content);
            entries.push(GuideEntry {
                name: name.clone(),
                title,
                description,
            });
            contents.insert(name, content);
        }
    }

    (entries, contents)
}

/// Parse a guide's first heading and first paragraph for index display.
pub(crate) fn parse_guide_header(content: &str) -> (String, String) {
    let mut title = String::new();
    let mut description = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !title.is_empty() && !description.is_empty() {
                break;
            }
            continue;
        }
        if title.is_empty() {
            if let Some(heading) = trimmed.strip_prefix("# ") {
                title = heading.to_string();
            }
        } else if description.is_empty() && !trimmed.starts_with('#') {
            description = trimmed.to_string();
        }
    }

    if title.is_empty() {
        title = "Untitled".to_string();
    }
    (title, description)
}

/// Extract a class definition (class line through next class/end of file)
fn extract_class_definition(content: &str, class_name: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let class_prefix = format!("class {class_name}");

    let start = lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(&class_prefix) && trimmed[class_prefix.len()..].starts_with(['(', ':'])
    })?;

    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        // Next top-level class or function definition
        if (trimmed.starts_with("class ") || trimmed.starts_with("def "))
            && !line.starts_with(' ')
            && !line.starts_with('\t')
        {
            end = i;
            break;
        }
    }

    Some(lines[start..end].join("\n"))
}

/// Parse a raw class definition into structured JSON with separate sections.
///
/// Produces: { class_header, constructor, static_methods, properties, methods }
fn format_class_structured(raw: &str) -> serde_json::Value {
    let lines: Vec<&str> = raw.lines().collect();

    let class_header = lines.first().map(|s| s.trim()).unwrap_or("").to_string();

    let mut constructor: Option<String> = None;
    let mut static_methods: Vec<String> = Vec::new();
    let mut properties: Vec<serde_json::Value> = Vec::new();
    let mut methods: Vec<String> = Vec::new();

    // State machine to parse .pyi stub members
    let mut i = 1; // skip class line
    let mut next_is_static = false;
    let mut next_is_property = false;
    let mut next_is_setter = false;
    let mut in_docstring = false;
    // Track property names we've already seen (to pair getter + setter)
    let mut property_names: HashMap<String, usize> = HashMap::new();

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Track triple-quoted docstring blocks
        let triple_count = trimmed.matches("\"\"\"").count();
        if in_docstring {
            if triple_count % 2 == 1 {
                in_docstring = false; // closing """
            }
            i += 1;
            continue;
        }
        if triple_count == 1 {
            // Opening """ without closing on same line
            in_docstring = true;
            i += 1;
            continue;
        }
        // triple_count == 0 or 2 (single-line docstring like """text""") — not in docstring

        if trimmed == "@staticmethod" {
            next_is_static = true;
            i += 1;
            continue;
        }
        if trimmed == "@property" {
            next_is_property = true;
            i += 1;
            continue;
        }
        if trimmed.ends_with(".setter") && trimmed.starts_with('@') {
            next_is_setter = true;
            i += 1;
            continue;
        }
        // Skip @overload and other decorators
        if trimmed.starts_with('@') {
            i += 1;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("def ") {
            // Handle multi-line signatures: collect continuation lines
            let mut full_sig = rest.to_string();
            while !full_sig.contains(": ...") && !full_sig.ends_with(" ...") && i + 1 < lines.len()
            {
                // Signature complete if parens are balanced and ends with ':'
                // (handles stubs where the body is a docstring, not '...')
                let open_parens = full_sig.matches('(').count();
                let close_parens = full_sig.matches(')').count();
                if open_parens > 0
                    && open_parens == close_parens
                    && full_sig.trim_end().ends_with(':')
                {
                    break;
                }
                i += 1;
                let cont = lines[i].trim();
                full_sig.push(' ');
                full_sig.push_str(cont);
            }
            // Trim stub body markers; try ": ..." first (longer match) before " ..."
            let sig = full_sig
                .trim_end_matches(": ...")
                .trim_end_matches(" ...")
                .trim_end_matches(':')
                .trim();

            if rest.starts_with("__init__(") {
                // Constructor — extract params (skip self)
                constructor = Some(extract_constructor_display(sig));
            } else if next_is_static {
                static_methods.push(format!("def {sig}"));
            } else if next_is_property {
                // Property getter
                let prop_name = rest.split('(').next().unwrap_or("").to_string();
                let return_type = extract_return_type(sig);
                let idx = properties.len();
                property_names.insert(prop_name.clone(), idx);
                properties.push(serde_json::json!({
                    "name": prop_name,
                    "type": return_type,
                    "readonly": true,
                }));
            } else if next_is_setter {
                // Property setter — mark existing property as read-write
                let prop_name = rest.split('(').next().unwrap_or("").to_string();
                if let Some(&idx) = property_names.get(&prop_name)
                    && let Some(obj) = properties[idx].as_object_mut()
                {
                    obj.insert("readonly".into(), serde_json::Value::Bool(false));
                }
            } else {
                // Regular method
                methods.push(format!("def {sig}"));
            }

            next_is_static = false;
            next_is_property = false;
            next_is_setter = false;
        } else if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with('"')
            && !trimmed.starts_with("class ")
            && !trimmed.starts_with("pass")
        {
            // Bare attribute annotation: "name: Type"
            if let Some(colon_pos) = trimmed.find(':') {
                let attr_name = trimmed[..colon_pos].trim();
                let attr_type = trimmed[colon_pos + 1..].trim().to_string();
                if !attr_name.is_empty()
                    && !attr_name.starts_with('_')
                    && attr_name.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && !attr_name.starts_with(|c: char| c.is_ascii_digit())
                    && !attr_type.starts_with("ClassVar")
                    && !property_names.contains_key(attr_name)
                {
                    let idx = properties.len();
                    property_names.insert(attr_name.to_string(), idx);
                    properties.push(serde_json::json!({
                        "name": attr_name,
                        "type": attr_type,
                        "readonly": false,
                    }));
                }
            }
        }

        i += 1;
    }

    serde_json::json!({
        "class": class_header,
        "constructor": constructor,
        "static_methods": static_methods,
        "properties": properties,
        "methods": methods,
    })
}

/// Extract a displayable constructor signature, removing `self` param.
fn extract_constructor_display(sig: &str) -> String {
    // sig looks like: __init__(self, radius: float = 1.0, height: float = 1.0) -> None
    if let Some(paren_start) = sig.find('(')
        && let Some(paren_end) = sig.rfind(')')
    {
        let params_str = &sig[paren_start + 1..paren_end];
        let params: Vec<&str> = params_str
            .split(',')
            .map(|p| p.trim())
            .filter(|p| *p != "self" && !p.is_empty())
            .collect();
        if params.is_empty() {
            return "()".to_string();
        }
        return format!("({})", params.join(", "));
    }
    sig.to_string()
}

/// Extract return type from a function signature.
fn extract_return_type(sig: &str) -> String {
    if let Some(arrow_pos) = sig.rfind(" -> ") {
        sig[arrow_pos + 4..].trim().to_string()
    } else {
        "None".to_string()
    }
}

/// Python-accessible wrapper around ApiIndex
#[pyclass(name = "ApiIndex")]
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

    /// Search .pyi stub files for a pattern. Returns JSON string of results.
    fn search(&self, query: &str) -> String {
        let results = self.inner.search(query);
        serde_json::to_string_pretty(&results).unwrap_or_default()
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
