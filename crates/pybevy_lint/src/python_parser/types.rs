/// Normalize a Python type for comparison
pub fn normalize_python_type(ty: &str) -> String {
    let ty = ty.trim();

    // Handle Optional[T] -> T | None
    if ty.starts_with("Optional[") && ty.ends_with(']') {
        let inner = &ty[9..ty.len() - 1];
        return format!("{} | None", normalize_python_type(inner));
    }

    // Handle list[T]
    if ty.starts_with("list[") && ty.ends_with(']') {
        let inner = &ty[5..ty.len() - 1];
        return format!("list[{}]", normalize_python_type(inner));
    }

    // Handle dict[K, V]
    if ty.starts_with("dict[") && ty.ends_with(']') {
        return ty.to_string(); // Not normalized
    }

    // Handle Union[A, B] -> A | B
    if ty.starts_with("Union[") && ty.ends_with(']') {
        let inner = &ty[6..ty.len() - 1];
        let parts: Vec<_> = split_type_union(inner)
            .into_iter()
            .map(|p| normalize_python_type(&p))
            .collect();
        return parts.join(" | ");
    }

    // Handle tuple[A, B, ...]
    if ty.starts_with("tuple[") && ty.ends_with(']') {
        return ty.to_string(); // Not normalized
    }

    // Handle Callable[[Args], Return]
    if ty.starts_with("Callable[") {
        return ty.to_string(); // Not normalized
    }

    ty.to_string()
}

/// Split a union type string by commas, respecting brackets
fn split_type_union(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in s.chars() {
        match c {
            '[' | '(' | '{' | '<' => {
                depth += 1;
                current.push(c);
            }
            ']' | ')' | '}' | '>' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

/// Check if two types are compatible (Rust type vs Python type)
pub fn types_compatible(rust_type: &str, python_type: &str) -> bool {
    let rust_normalized = crate::rust_parser::types::normalize_rust_type(rust_type);
    let python_normalized = normalize_python_type(python_type);

    // Direct match
    if rust_normalized == python_normalized {
        return true;
    }

    // Check common equivalences
    match (rust_normalized.as_str(), python_normalized.as_str()) {
        ("float", "float") => true,
        ("int", "int") => true,
        ("bool", "bool") => true,
        ("str", "str") => true,
        ("None", "None") => true,

        // pyo3's `uuid` feature converts Rust `Uuid` to and from Python's `uuid.UUID`.
        ("Uuid", "UUID") => true,

        // PyType (normalized from Py<PyType>) is compatible with type[T] (generic type parameter)
        // After normalization: Py<PyType> -> PyType -> Type (Py prefix stripped)
        ("Type", python) => python.starts_with("type[") || python == "type",

        // PyAny is dynamic on the Rust side; the stub is the contract and may
        // declare any narrower type (a class, a union, a type variable).
        (rust, _python) if rust.starts_with("Any") => true,

        // A stub may refine a bare runtime container with element types.
        (rust, python)
            if matches!(rust, "tuple" | "list" | "dict" | "set" | "frozenset")
                && python.starts_with(&format!("{rust}[")) =>
        {
            true
        }

        // A stub may narrow a runtime str to a Literal of string values.
        ("str", python) if python.starts_with("Literal[") => true,

        // Handle union types (A | None vs Option<A>)
        (rust, python) if rust.ends_with(" | None") && python.ends_with(" | None") => {
            let rust_base = rust.trim_end_matches(" | None");
            let python_base = python.trim_end_matches(" | None");
            types_compatible(rust_base, python_base)
        }

        // Handle list types
        (rust, python) if rust.starts_with("list[") && python.starts_with("list[") => {
            let rust_inner = &rust[5..rust.len() - 1];
            let python_inner = &python[5..python.len() - 1];
            types_compatible(rust_inner, python_inner)
        }

        // Allow some flexibility for complex types
        // If both are non-primitive, just check the base name matches
        (rust, python) if !is_primitive(rust) && !is_primitive(python) => {
            let rust_base = base_type_name(rust);
            let python_base = base_type_name(python);
            rust_base == python_base
        }

        _ => false,
    }
}

/// Check if a type is a primitive
fn is_primitive(ty: &str) -> bool {
    matches!(ty, "float" | "int" | "bool" | "str" | "None")
}

/// Extract the base type name (without generics)
fn base_type_name(ty: &str) -> &str {
    if let Some(bracket_pos) = ty.find('[') {
        &ty[..bracket_pos]
    } else if let Some(space_pos) = ty.find(' ') {
        &ty[..space_pos]
    } else {
        ty
    }
}
