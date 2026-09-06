use syn::Type;

/// Convert a Rust type to a string representation
pub fn type_to_string(ty: &Type) -> String {
    quote::quote!(#ty).to_string()
}

/// Normalize a Rust type for comparison with Python types
pub fn normalize_rust_type(ty: &str) -> String {
    let ty = ty.trim();

    // Remove lifetime annotations
    let ty = remove_lifetimes(ty);

    // A borrow names the same Python class as the value: pyo3 accepts
    // `&PyAxisSettings` wherever `PyAxisSettings` is the declared type.
    let ty = ty.trim().strip_prefix('&').map_or(ty.clone(), |rest| {
        rest.trim()
            .strip_prefix("mut ")
            .unwrap_or(rest)
            .trim()
            .to_string()
    });

    // Qualified paths compare by their final segment
    // (`crate :: quat :: PyQuat` names the same class as `PyQuat`).
    let ty = strip_path_qualification(&ty);

    // Handle PyResult<T> -> T
    if let Some(inner) = extract_generic(ty.trim(), "PyResult") {
        return normalize_rust_type(&inner);
    }

    // Handle Py<T> -> T
    if let Some(inner) = extract_generic(ty.trim(), "Py") {
        return normalize_rust_type(&inner);
    }

    // Handle Bound<'_, T> -> T. remove_lifetimes may have already consumed
    // the lifetime and its trailing comma, leaving a single parameter.
    if let Some(inner) = extract_generic(ty.trim(), "Bound") {
        let parts: Vec<&str> = inner.splitn(2, ',').collect();
        let target = if parts.len() == 2 { parts[1] } else { parts[0] };
        return normalize_rust_type(target.trim());
    }

    // Handle Option<T> -> T | None
    if let Some(inner) = extract_generic(ty.trim(), "Option") {
        let inner_normalized = normalize_rust_type(&inner);
        return format!("{} | None", inner_normalized);
    }

    // Handle Vec<T> -> list[T]
    if let Some(inner) = extract_generic(ty.trim(), "Vec") {
        let inner_normalized = normalize_rust_type(&inner);
        return format!("list[{}]", inner_normalized);
    }

    // Handle HashMap<K, V> -> dict[K, V]
    if let Some(inner) = extract_generic(ty.trim(), "HashMap") {
        return format!("dict[{}]", inner);
    }

    // Handle [T; N] arrays -> tuple[T, T, ...] (N times)
    if let Some((element_type, count)) = parse_rust_array(ty.trim()) {
        let element_normalized = normalize_rust_type(&element_type);
        if count > 0 && count <= 16 {
            // Reasonable limit for tuple expansion
            let elements = vec![element_normalized; count];
            return format!("tuple[{}]", elements.join(", "));
        }
    }

    // Handle Rust unit type () -> None (before tuple handling)
    if ty.trim() == "()" {
        return "None".to_string();
    }

    // Handle Rust tuples (A, B) -> tuple[A, B]
    if ty.trim().starts_with('(') && ty.trim().ends_with(')') {
        let inner = &ty.trim()[1..ty.trim().len() - 1];
        let parts: Vec<_> = split_by_comma(inner)
            .into_iter()
            .map(|p| normalize_rust_type(&p))
            .collect();
        return format!("tuple[{}]", parts.join(", "));
    }

    // Map PyO3 collection types BEFORE stripping Py prefix
    match ty.trim() {
        "PyList" => return "list".to_string(),
        "PyDict" => return "dict".to_string(),
        "PyTuple" => return "tuple".to_string(),
        "PySet" => return "set".to_string(),
        "PyFrozenSet" => return "frozenset".to_string(),
        "PyBytes" => return "bytes".to_string(),
        "PyByteArray" => return "bytearray".to_string(),
        "PyAny" => return "Any  # FIXME: check actual return type".to_string(),
        _ => {}
    }

    // Strip Py prefix from type names
    let ty = strip_py_prefix(ty.trim());

    // Map primitive types
    match ty.as_str() {
        "f32" | "f64" => "float".to_string(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "int".to_string(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "int".to_string(),
        "bool" => "bool".to_string(),
        "String" | "& str" | "&str" => "str".to_string(),
        "()" => "None".to_string(),
        // std::time::Duration -> datetime.timedelta (PyO3 auto-converts)
        "Duration" | "std :: time :: Duration" => "timedelta".to_string(),
        // Stripped "Any" (from PyAny after prefix stripping)
        "Any" => "Any  # FIXME: check actual return type".to_string(),
        _ => ty,
    }
}

/// Reduce a plain qualified path to its final segment. Types with generics,
/// tuples, or references are left for the structural handlers.
fn strip_path_qualification(ty: &str) -> String {
    let trimmed = ty.trim();
    if trimmed
        .chars()
        .any(|c| matches!(c, '<' | '(' | '[' | ',' | '&'))
    {
        return trimmed.to_string();
    }
    match trimmed.rsplit("::").next() {
        Some(last) => last.trim().to_string(),
        None => trimmed.to_string(),
    }
}

/// Extract the inner type from a generic type like Foo<Bar>
fn extract_generic(ty: &str, wrapper: &str) -> Option<String> {
    let ty = ty.trim();

    // Check if it starts with the wrapper
    if !ty.starts_with(wrapper) {
        return None;
    }

    let rest = &ty[wrapper.len()..].trim_start();

    // Must start with <
    if !rest.starts_with('<') {
        return None;
    }

    // Find matching >
    let mut depth = 0;
    let mut start = 0;
    let mut end = 0;

    for (i, c) in rest.char_indices() {
        match c {
            '<' => {
                if depth == 0 {
                    start = i + 1;
                }
                depth += 1;
            }
            '>' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if end > start {
        Some(rest[start..end].to_string())
    } else {
        None
    }
}

/// Parse a Rust array type like [T; N] and return (element_type, count)
fn parse_rust_array(ty: &str) -> Option<(String, usize)> {
    let ty = ty.trim();

    // Must start with [ and end with ]
    if !ty.starts_with('[') || !ty.ends_with(']') {
        return None;
    }

    let inner = &ty[1..ty.len() - 1];

    // Find the semicolon that separates type and count
    // Need to handle nested brackets correctly
    let mut depth = 0;
    let mut semicolon_pos = None;

    for (i, c) in inner.char_indices() {
        match c {
            '[' | '<' | '(' | '{' => depth += 1,
            ']' | '>' | ')' | '}' => depth -= 1,
            ';' if depth == 0 => {
                semicolon_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    let semicolon_pos = semicolon_pos?;
    let element_type = inner[..semicolon_pos].trim().to_string();
    let count_str = inner[semicolon_pos + 1..].trim();

    let count: usize = count_str.parse().ok()?;

    Some((element_type, count))
}

/// Split a string by comma, respecting bracket nesting
fn split_by_comma(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in s.chars() {
        match c {
            '[' | '<' | '(' | '{' => {
                depth += 1;
                current.push(c);
            }
            ']' | '>' | ')' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }

    parts
}

/// Remove lifetime annotations from a type string
fn remove_lifetimes(ty: &str) -> String {
    let mut result = String::new();
    let mut chars = ty.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\'' {
            // Skip the lifetime
            while let Some(&next) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            // Skip trailing comma and whitespace if any
            while let Some(&next) = chars.peek() {
                if next == ',' || next == ' ' {
                    chars.next();
                } else {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Strip "Py" prefix from type names
fn strip_py_prefix(ty: &str) -> String {
    if ty.starts_with("Py") && ty.len() > 2 {
        let rest = &ty[2..];
        // Make sure it's actually a type name (starts with uppercase)
        if rest.chars().next().is_some_and(|c| c.is_uppercase()) {
            return rest.to_string();
        }
    }
    ty.to_string()
}
