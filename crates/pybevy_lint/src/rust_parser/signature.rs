use std::collections::HashMap;

use syn::{Attribute, Expr, Meta};

use super::pymethods::SignatureInfo;

/// Parse #[pyo3(signature = (...))] attribute
pub fn parse_signature_attr(attr: &Attribute) -> Option<SignatureInfo> {
    if !attr.path().is_ident("pyo3") {
        return None;
    }

    let nested = attr
        .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .ok()?;

    for meta in nested {
        if let Meta::NameValue(nv) = meta
            && nv.path.is_ident("signature")
        {
            return parse_signature_expr(&nv.value);
        }
    }

    None
}

/// Parse the signature expression (param = default, ...)
fn parse_signature_expr(expr: &Expr) -> Option<SignatureInfo> {
    let raw = quote::quote!(#expr).to_string();

    // Parse defaults from the signature
    // The signature looks like: (param1 = default1, param2 = default2, ...)
    let defaults = parse_signature_defaults(&raw);

    Some(SignatureInfo { raw, defaults })
}

/// Parse defaults from a signature string
fn parse_signature_defaults(sig: &str) -> HashMap<String, String> {
    let mut defaults = HashMap::new();

    // Remove outer parentheses if present
    let sig = sig.trim();
    let sig = if sig.starts_with('(') && sig.ends_with(')') {
        &sig[1..sig.len() - 1]
    } else {
        sig
    };

    // Split by comma, but respect nested parentheses/brackets
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in sig.chars() {
        match c {
            '(' | '[' | '<' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '>' | '}' => {
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

    // Parse each part for name = default
    for part in parts {
        if let Some(eq_pos) = find_equals_sign(&part) {
            let name = part[..eq_pos].trim();
            let default = part[eq_pos + 1..].trim();

            // Clean up the name (might have * or ** prefix)
            let name = name.trim_start_matches(['*', '/']);
            let name = name.trim();

            if !name.is_empty() {
                defaults.insert(name.to_string(), default.to_string());
            }
        }
    }

    defaults
}

/// Find the position of the equals sign in a parameter definition
/// This needs to handle cases like:
/// - param = value
/// - param: type = value (shouldn't happen in pyo3 signature)
fn find_equals_sign(s: &str) -> Option<usize> {
    let mut depth = 0;

    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '<' | '{' => depth += 1,
            ')' | ']' | '>' | '}' => depth -= 1,
            '=' if depth == 0 => {
                // Make sure it's not == or !=
                let prev = s.chars().nth(i.saturating_sub(1));
                let next = s.chars().nth(i + 1);
                if prev != Some('!') && prev != Some('=') && next != Some('=') {
                    return Some(i);
                }
            }
            _ => {}
        }
    }

    None
}
