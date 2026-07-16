use std::{
    collections::{HashMap, HashSet},
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

    /// Search for a pattern across all .pyi files.
    ///
    /// Returns the (possibly truncated) result list along with the total
    /// number of matches before truncation. When `limit` is `None` the full
    /// result set is returned.
    pub fn search(&self, query: &str, limit: Option<usize>) -> (Vec<SearchResult>, usize) {
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

        let total = results.len();
        if let Some(cap) = limit {
            results.truncate(cap);
        }
        (results, total)
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

/// Parse a .pyi file to extract class and function names.
///
/// Only collects top-level (unindented) definitions and deduplicates names so
/// `@overload` stubs and other repeated declarations appear once. Indented
/// `class Foo:` text inside docstrings or method bodies is ignored.
fn parse_stub_definitions(content: &str) -> (Vec<String>, Vec<String>) {
    let mut classes = Vec::new();
    let mut functions = Vec::new();
    let mut seen_classes: HashSet<String> = HashSet::new();
    let mut seen_functions: HashSet<String> = HashSet::new();

    for line in content.lines() {
        // Only consider top-level definitions; skip indented lines so
        // class-shaped text inside docstrings or method bodies doesn't leak in.
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some(name) = rest.split(['(', ':']).next() {
                let name = name.trim().to_string();
                if !name.is_empty() && seen_classes.insert(name.clone()) {
                    classes.push(name);
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("def ")
            && let Some(name) = rest.split('(').next()
        {
            let name = name.trim().to_string();
            if !name.is_empty() && seen_functions.insert(name.clone()) {
                functions.push(name);
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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn parse_stub_definitions_classes() {
        let content = "class Foo(Bar):\n    pass\nclass Baz:\n    pass\n";
        let (classes, functions) = parse_stub_definitions(content);
        assert_eq!(classes, vec!["Foo", "Baz"]);
        assert!(functions.is_empty());
    }

    #[test]
    fn parse_stub_definitions_functions() {
        let content = "def top_level(x: int) -> None: ...\n";
        let (classes, functions) = parse_stub_definitions(content);
        assert!(classes.is_empty());
        assert_eq!(functions, vec!["top_level"]);
    }

    #[test]
    fn parse_stub_definitions_ignores_methods() {
        let content = "class Foo:\n    def method(self) -> None: ...\n";
        let (classes, functions) = parse_stub_definitions(content);
        assert_eq!(classes, vec!["Foo"]);
        assert!(functions.is_empty()); // indented = method, not top-level
    }

    #[test]
    fn parse_stub_definitions_mixed() {
        let content = "\
class Alpha(Base):
    def method(self): ...

def utility() -> int: ...

class Beta:
    x: int
";
        let (classes, functions) = parse_stub_definitions(content);
        assert_eq!(classes, vec!["Alpha", "Beta"]);
        assert_eq!(functions, vec!["utility"]);
    }

    #[test]
    fn parse_stub_definitions_skips_class_in_docstring() {
        // class-shaped text inside a docstring (or any indented block) must not
        // be treated as a real top-level class.
        let content = "\
class Real(Base):
    \"\"\"Doc with a fake declaration:

        class GameState(Resource):
            pass
    \"\"\"
    pass
";
        let (classes, functions) = parse_stub_definitions(content);
        assert_eq!(classes, vec!["Real"]);
        assert!(functions.is_empty());
    }

    #[test]
    fn parse_stub_definitions_dedupes_overload_stubs() {
        // @overload produces multiple top-level def stubs with the same name;
        // they should collapse to a single entry.
        let content = "\
@overload
def component(cls: type) -> type: ...
@overload
def component(*, name: str) -> Callable[[type], type]: ...
@overload
def component(cls: None = None, *, name: str | None = None) -> Callable[[type], type]: ...

def resource(cls: type) -> type: ...
";
        let (classes, functions) = parse_stub_definitions(content);
        assert!(classes.is_empty());
        assert_eq!(functions, vec!["component", "resource"]);
    }

    #[test]
    fn parse_stub_definitions_dedupes_repeated_classes() {
        let content = "\
class GameState(Resource): ...
class Velocity(Component): ...
class GameState(Resource): ...
";
        let (classes, _functions) = parse_stub_definitions(content);
        assert_eq!(classes, vec!["GameState", "Velocity"]);
    }

    #[test]
    fn extract_class_definition_found() {
        let content = "\
class Foo(Bar):
    x: int
    def method(self): ...

class Baz:
    pass
";
        let result = extract_class_definition(content, "Foo").unwrap();
        assert!(result.starts_with("class Foo(Bar):"));
        assert!(result.contains("def method"));
        assert!(!result.contains("class Baz"));
    }

    #[test]
    fn extract_class_definition_not_found() {
        let content = "class Foo:\n    pass\n";
        assert!(extract_class_definition(content, "NotHere").is_none());
    }

    #[test]
    fn extract_class_definition_last_class() {
        let content = "\
class First:
    pass

class Last:
    x: int
    y: float
";
        let result = extract_class_definition(content, "Last").unwrap();
        assert!(result.starts_with("class Last:"));
        assert!(result.contains("y: float"));
    }

    #[test]
    fn extract_constructor_display_with_params() {
        let sig = "__init__(self, x: float, y: float) -> None";
        assert_eq!(extract_constructor_display(sig), "(x: float, y: float)");
    }

    #[test]
    fn extract_constructor_display_no_params() {
        let sig = "__init__(self) -> None";
        assert_eq!(extract_constructor_display(sig), "()");
    }

    #[test]
    fn extract_constructor_display_self_only() {
        let sig = "__init__(self) -> None";
        assert_eq!(extract_constructor_display(sig), "()");
    }

    #[test]
    fn extract_return_type_with_arrow() {
        assert_eq!(extract_return_type("def foo(self) -> Vec3"), "Vec3");
    }

    #[test]
    fn extract_return_type_no_arrow() {
        assert_eq!(extract_return_type("def foo(self)"), "None");
    }

    #[test]
    fn parse_guide_header_normal() {
        let content = "# My Guide\n\nThis is the description.\n\nMore content.\n";
        let (title, desc) = parse_guide_header(content);
        assert_eq!(title, "My Guide");
        assert_eq!(desc, "This is the description.");
    }

    #[test]
    fn parse_guide_header_no_heading() {
        let content = "Just some text.\n";
        let (title, _desc) = parse_guide_header(content);
        assert_eq!(title, "Untitled");
    }

    #[test]
    fn parse_guide_header_empty_content() {
        let (title, desc) = parse_guide_header("");
        assert_eq!(title, "Untitled");
        assert!(desc.is_empty());
    }

    #[test]
    fn format_class_structured_properties() {
        let raw = "\
class MyType(Component):
    @property
    def x(self) -> float: ...
    @x.setter
    def x(self, value: float) -> None: ...
    @property
    def y(self) -> float: ...
";
        let result = format_class_structured(raw);
        let props = result["properties"].as_array().unwrap();
        assert_eq!(props.len(), 2);

        // x has setter → readonly=false
        assert_eq!(props[0]["name"], "x");
        assert_eq!(props[0]["readonly"], false);

        // y has no setter → readonly=true
        assert_eq!(props[1]["name"], "y");
        assert_eq!(props[1]["readonly"], true);
    }

    #[test]
    fn format_class_structured_bare_annotations() {
        let raw = "\
class Sprite(Component):
    image: Handle[Image]
    \"\"\"Handle to the image asset.\"\"\"
    color: Color
    flip_x: bool
    flip_y: bool
    custom_size: tuple[float, float] | None
    def __init__(self, image: Handle[Image]) -> None: ...
    def as_asset_id(self) -> Handle[Image]: ...
";
        let result = format_class_structured(raw);
        let props = result["properties"].as_array().unwrap();
        assert_eq!(props.len(), 5, "Expected 5 bare annotation properties");

        assert_eq!(props[0]["name"], "image");
        assert_eq!(props[0]["type"], "Handle[Image]");
        assert_eq!(props[0]["readonly"], false);

        assert_eq!(props[1]["name"], "color");
        assert_eq!(props[2]["name"], "flip_x");
        assert_eq!(props[3]["name"], "flip_y");
        assert_eq!(props[4]["name"], "custom_size");
        assert_eq!(props[4]["type"], "tuple[float, float] | None");

        // Constructor and methods should still be parsed
        assert!(result["constructor"].is_string());
        let methods = result["methods"].as_array().unwrap();
        assert_eq!(methods.len(), 1);
    }

    #[test]
    fn format_class_structured_skips_docstring_content() {
        let raw = r#"class Sprite(Component):
    """A sprite component.

    Args:
        image: Handle to the image asset
        color: Color tint

    Examples:
        ```python
        Sprite.from_image(handle)
        ```

    Notes:
        Sprites are rendered by the SpritePlugin.
    """

    image: Handle[Image]
    """Handle to the image asset."""
    color: Color
    def __init__(self, image: Handle[Image]) -> None: ...
"#;
        let result = format_class_structured(raw);
        let props = result["properties"].as_array().unwrap();
        let names: Vec<&str> = props.iter().map(|p| p["name"].as_str().unwrap()).collect();
        // Only real attributes, not docstring content
        assert_eq!(names, vec!["image", "color"]);
        assert!(!names.contains(&"Args"));
        assert!(!names.contains(&"Examples"));
        assert!(!names.contains(&"Notes"));
    }

    #[test]
    fn format_class_structured_skips_classvar() {
        let raw = "\
class Transform(Component):
    translation: Vec3
    rotation: Quat
    scale: Vec3
    IDENTITY: ClassVar[Transform]
";
        let result = format_class_structured(raw);
        let props = result["properties"].as_array().unwrap();
        assert_eq!(props.len(), 3, "ClassVar should be excluded");
        let names: Vec<&str> = props.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"translation"));
        assert!(names.contains(&"rotation"));
        assert!(names.contains(&"scale"));
        assert!(!names.contains(&"IDENTITY"));
    }

    #[test]
    fn format_class_structured_bare_and_property_mix() {
        let raw = "\
class Mixed(Component):
    bare_field: float
    @property
    def prop_field(self) -> int: ...
    @prop_field.setter
    def prop_field(self, value: int) -> None: ...
";
        let result = format_class_structured(raw);
        let props = result["properties"].as_array().unwrap();
        assert_eq!(props.len(), 2);
        assert_eq!(props[0]["name"], "bare_field");
        assert_eq!(props[0]["readonly"], false);
        assert_eq!(props[1]["name"], "prop_field");
        assert_eq!(props[1]["readonly"], false);
    }

    #[test]
    fn format_class_structured_constructor() {
        let raw = "\
class Vec3:
    def __init__(self, x: float, y: float, z: float) -> None: ...
    def normalize(self) -> Vec3: ...
";
        let result = format_class_structured(raw);
        assert_eq!(
            result["constructor"].as_str().unwrap(),
            "(x: float, y: float, z: float)"
        );
        let methods = result["methods"].as_array().unwrap();
        assert_eq!(methods.len(), 1);
        assert!(methods[0].as_str().unwrap().contains("normalize"));
    }

    #[test]
    fn format_class_structured_static_methods() {
        let raw = "\
class Color:
    @staticmethod
    def linear_rgb(r: float, g: float, b: float) -> Color: ...
    def alpha(self) -> float: ...
";
        let result = format_class_structured(raw);
        let statics = result["static_methods"].as_array().unwrap();
        assert_eq!(statics.len(), 1);
        assert!(statics[0].as_str().unwrap().contains("linear_rgb"));
    }

    #[test]
    fn format_class_structured_property_with_docstring_body() {
        // Pattern used by DistanceFog: @property with docstring body instead of "..."
        let raw = r#"class DistanceFog(Component):
    @property
    def color(self) -> Color:
        """Base color of the fog."""
    @color.setter
    def color(self, value: Color) -> None:
        """Set the fog color."""
    @property
    def falloff(self) -> FogFalloff:
        """The fog falloff mode."""
"#;
        let result = format_class_structured(raw);
        let props = result["properties"].as_array().unwrap();
        assert_eq!(props.len(), 2, "Expected 2 properties, got {:?}", props);

        assert_eq!(props[0]["name"], "color");
        assert_eq!(props[0]["type"], "Color");
        assert_eq!(props[0]["readonly"], false); // has setter

        assert_eq!(props[1]["name"], "falloff");
        assert_eq!(props[1]["type"], "FogFalloff");
        assert_eq!(props[1]["readonly"], true); // no setter
    }

    #[test]
    fn format_class_structured_init_with_docstring_body() {
        // Pattern used by Window: __init__ with docstring body, plus @property with docstrings
        let raw = r#"class Window(Component):
    def __init__(
        self,
        resolution: WindowResolution = WindowResolution(),
        title: str = "App",
    ) -> None:
        """Create a new Window."""
    @property
    def resolution(self) -> WindowResolution:
        """Get the window resolution."""
    @resolution.setter
    def resolution(self, value: WindowResolution) -> None:
        """Set the window resolution."""
    @property
    def title(self) -> str:
        """Get the window title."""
"#;
        let result = format_class_structured(raw);

        // Constructor should be parsed
        assert!(
            result["constructor"].is_string(),
            "Constructor missing: {:?}",
            result["constructor"]
        );
        let ctor = result["constructor"].as_str().unwrap();
        assert!(
            ctor.contains("resolution"),
            "Constructor should have resolution param: {ctor}"
        );

        // Properties should be parsed
        let props = result["properties"].as_array().unwrap();
        assert_eq!(props.len(), 2, "Expected 2 properties, got {:?}", props);
        assert_eq!(props[0]["name"], "resolution");
        assert_eq!(props[0]["readonly"], false);
        assert_eq!(props[1]["name"], "title");
        assert_eq!(props[1]["readonly"], true);
    }

    #[test]
    fn format_class_structured_no_trailing_colon_in_type() {
        // Bloom pattern: verify return type doesn't have trailing colon
        let raw = "\
class Bloom(Component):
    @property
    def intensity(self) -> float: ...
    @intensity.setter
    def intensity(self, value: float) -> None: ...
";
        let result = format_class_structured(raw);
        let props = result["properties"].as_array().unwrap();
        assert_eq!(props.len(), 1);
        assert_eq!(props[0]["name"], "intensity");
        assert_eq!(props[0]["type"], "float"); // NOT "float:"
        assert_eq!(props[0]["readonly"], false);
    }

    #[test]
    fn api_index_build_with_temp_dir() {
        let dir = std::env::temp_dir().join("pybevy_test_api_index");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("math.pyi"),
            "class Vec3:\n    pass\n\ndef lerp(a: float, b: float) -> float: ...\n",
        )
        .unwrap();
        fs::write(
            dir.join("ecs.pyi"),
            "class Query:\n    pass\nclass Commands:\n    pass\n",
        )
        .unwrap();

        let index = ApiIndex::build(&dir);
        assert_eq!(index.entries.len(), 2);

        // Check contents were loaded
        assert!(index.contents.contains_key("math"));
        assert!(index.contents.contains_key("ecs"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_index_search_case_insensitive() {
        let dir = std::env::temp_dir().join("pybevy_test_search");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("test.pyi"),
            "class Transform:\n    translation: Vec3\n",
        )
        .unwrap();

        let index = ApiIndex::build(&dir);
        let (results, total) = index.search("transform", Some(100));
        assert!(!results.is_empty());
        assert!(results[0].text.contains("Transform"));
        assert_eq!(total, results.len());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_index_search_truncation() {
        let dir = std::env::temp_dir().join("pybevy_test_truncation");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Create a file with many matching lines
        let content: String = (0..200)
            .map(|i| format!("class Item{i}:\n    pass\n"))
            .collect();
        fs::write(dir.join("many.pyi"), content).unwrap();

        let index = ApiIndex::build(&dir);

        // Explicit limit caps the result list but `total` reports the true count.
        let (results, total) = index.search("Item", Some(50));
        assert_eq!(results.len(), 50);
        assert_eq!(total, 200);
        assert!(total > results.len(), "expected truncation");

        // No limit returns the full set with matching `total`.
        let (all, all_total) = index.search("Item", None);
        assert_eq!(all.len(), 200);
        assert_eq!(all_total, 200);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_index_get_type_definition() {
        let dir = std::env::temp_dir().join("pybevy_test_typedef");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("types.pyi"),
            "class Alpha:\n    x: int\n\nclass Beta:\n    y: float\n",
        )
        .unwrap();

        let index = ApiIndex::build(&dir);
        let def = index.get_type_definition("Alpha").unwrap();
        assert!(def.contains("class Alpha:"));
        assert!(def.contains("x: int"));
        assert!(!def.contains("Beta"));

        assert!(index.get_type_definition("NonExistent").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_index_guides() {
        let dir = std::env::temp_dir().join("pybevy_test_guides");
        let _ = fs::remove_dir_all(&dir);
        let guides_dir = dir.join("mcp/guides");
        fs::create_dir_all(&guides_dir).unwrap();

        fs::write(
            guides_dir.join("camera.md"),
            "# Camera Guide\n\nHow to set up cameras.\n",
        )
        .unwrap();

        let index = ApiIndex::build(&dir);
        let guide_entries = index.get_guide_index();
        assert_eq!(guide_entries.len(), 1);
        assert_eq!(guide_entries[0].name, "camera");
        assert_eq!(guide_entries[0].title, "Camera Guide");

        let content = index.get_guide("camera").unwrap();
        assert!(content.contains("# Camera Guide"));

        assert!(index.get_guide("nonexistent").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_index_py_fallback() {
        let dir = std::env::temp_dir().join("pybevy_test_py_fallback");
        let _ = fs::remove_dir_all(&dir);
        let contrib_dir = dir.join("contrib");
        fs::create_dir_all(&contrib_dir).unwrap();

        // .pyi file (preferred)
        fs::write(dir.join("math.pyi"), "class Vec3:\n    pass\n").unwrap();
        // .py file that has no .pyi counterpart (should be indexed)
        fs::write(
            contrib_dir.join("orbit_camera.py"),
            "class OrbitCamera(Component):\n    pass\n\nclass OrbitCameraPlugin(Plugin):\n    pass\n",
        )
        .unwrap();
        // .py file that has a .pyi counterpart (should be skipped)
        fs::write(dir.join("math.py"), "class Vec3Impl:\n    pass\n").unwrap();
        // __init__.py should be skipped
        fs::write(
            contrib_dir.join("__init__.py"),
            "from .orbit_camera import *\n",
        )
        .unwrap();

        let index = ApiIndex::build(&dir);

        // Should have math (from .pyi) and contrib.orbit_camera (from .py fallback)
        assert!(
            index.contents.contains_key("math"),
            "math .pyi should be indexed"
        );
        assert!(
            index.contents.contains_key("contrib.orbit_camera"),
            "contrib.orbit_camera .py should be indexed as fallback"
        );
        // math.py should NOT override math.pyi
        let math_content = index.contents.get("math").unwrap();
        assert!(math_content.contains("Vec3"), "math should come from .pyi");
        assert!(
            !math_content.contains("Vec3Impl"),
            "math.py should not override .pyi"
        );

        // Search should find contrib classes
        let (results, total) = index.search("OrbitCamera", Some(100));
        assert!(
            !results.is_empty(),
            "search_api should find OrbitCamera from .py fallback"
        );
        assert_eq!(total, results.len());

        // __init__.py should not be indexed
        assert!(!index.contents.contains_key("contrib.__init__"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_index_instructions() {
        let dir = std::env::temp_dir().join("pybevy_test_instructions");
        let _ = fs::remove_dir_all(&dir);
        let mcp_dir = dir.join("mcp");
        fs::create_dir_all(&mcp_dir).unwrap();

        fs::write(
            mcp_dir.join("instructions.md"),
            "# PyBevy MCP\n\nDefault instructions content.\n",
        )
        .unwrap();

        let index = ApiIndex::build(&dir);
        let instructions = index.get_instructions().unwrap();
        assert!(instructions.contains("PyBevy MCP"));
        assert!(instructions.contains("Default instructions content"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_index_instructions_missing() {
        let dir = std::env::temp_dir().join("pybevy_test_instructions_missing");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let index = ApiIndex::build(&dir);
        assert!(index.get_instructions().is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
