use syn::Attribute;

use super::pymethods::SignatureInfo;
use crate::model::ParameterKind;

/// Parse #[pyo3(signature = (...))] attribute
pub fn parse_signature_attr(attr: &Attribute) -> Option<SignatureInfo> {
    parse_keyed_group_attr(attr, "signature")
}

/// Parse `#[pyo3(constructor = (...))]` on a generated enum variant block.
/// The variant's keyword-only/positional kinds come from `*`/`/` separators
/// exactly like `signature = (...)`, so the runtime's declared constructor
/// is authoritative over any stub claim.
pub fn parse_variant_constructor_attr(attr: &Attribute) -> Option<SignatureInfo> {
    parse_keyed_group_attr(attr, "constructor")
}

fn parse_keyed_group_attr(attr: &Attribute, key: &str) -> Option<SignatureInfo> {
    if !attr.path().is_ident("pyo3") {
        return None;
    }

    // Meta/Expr parsing rejects signatures containing bare `*` or `/`
    // separators (they are not valid Rust expressions), so scan the raw
    // token stream for `key = (...)` instead. Joint punct spacing
    // survives `to_string`, keeping `*args`/`**kwargs` splats intact.
    let args: proc_macro2::TokenStream = attr.parse_args().ok()?;
    let mut tokens = args.into_iter();

    while let Some(token) = tokens.next() {
        let ident = match token {
            proc_macro2::TokenTree::Ident(ident) => ident,
            _ => continue,
        };
        if ident != key {
            continue;
        }
        match tokens.next() {
            Some(proc_macro2::TokenTree::Punct(p)) if p.as_char() == '=' => {}
            _ => return None,
        }
        let group = match tokens.next() {
            Some(proc_macro2::TokenTree::Group(group))
                if group.delimiter() == proc_macro2::Delimiter::Parenthesis =>
            {
                group
            }
            _ => return None,
        };
        let raw = group.stream().to_string();
        let (kinds, defaults) = parse_signature_kinds_and_defaults(&raw);
        return Some(SignatureInfo {
            raw,
            defaults,
            kinds,
        });
    }

    None
}

/// Parse kinds and defaults from a signature string.
/// - `/` marks every preceding parameter positional-only.
/// - a bare `*` marks every following parameter keyword-only.
/// - `*args` / `**kwargs` splats are not separators.
/// - a `*` inside a nested default expression (tracked by depth) is not a
///   keyword-only marker.
fn parse_signature_kinds_and_defaults(
    sig: &str,
) -> (
    std::collections::HashMap<String, ParameterKind>,
    std::collections::HashMap<String, String>,
) {
    let mut kinds = std::collections::HashMap::new();
    let mut defaults = std::collections::HashMap::new();

    let trimmed = sig.trim();
    let inner = trimmed
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(trimmed);

    let mut after_star = false;

    for (segment, is_separator) in split_signature_segments(inner) {
        if is_separator {
            if segment == "/" {
                for name in kinds.keys().cloned().collect::<Vec<_>>() {
                    kinds.insert(name, ParameterKind::PositionalOnly);
                }
                continue;
            }
            if segment == "*" {
                after_star = true;
                continue;
            }
        }

        let segment_trimmed = segment.trim();
        if let Some(stripped) = segment_trimmed.strip_prefix("**") {
            let name = stripped.trim();
            if !name.is_empty() {
                kinds.insert(name.to_string(), ParameterKind::KeywordOnly);
            }
            continue;
        }
        if let Some(stripped) = segment_trimmed.strip_prefix('*') {
            let name = stripped.trim();
            if !name.is_empty() {
                kinds.insert(name.to_string(), ParameterKind::PositionalOrKeyword);
            }
            after_star = true;
            continue;
        }

        let name_part = match find_equals_sign(segment_trimmed) {
            Some(eq_pos) => {
                let name = segment_trimmed[..eq_pos].trim();
                let default = segment_trimmed[eq_pos + 1..].trim();
                if !name.is_empty() {
                    defaults.insert(name.to_string(), default.to_string());
                }
                name
            }
            None => segment_trimmed,
        };

        let name_clean = name_part.split(':').next().unwrap_or(name_part).trim();
        if name_clean.is_empty() {
            continue;
        }
        if after_star {
            kinds.insert(name_clean.to_string(), ParameterKind::KeywordOnly);
        } else {
            kinds.insert(name_clean.to_string(), ParameterKind::PositionalOrKeyword);
        }
    }

    (kinds, defaults)
}

/// Split a signature into (segment, is_separator) pairs. Commas at depth 0
/// delimit arguments; a single argument that is exactly `*`, `**kwargs` or
/// `/` is a separator. `*`/`/` inside an argument (e.g. a default such as
/// `1.0 / 125.0`) is not a separator.
fn split_signature_segments(sig: &str) -> Vec<(String, bool)> {
    let mut raw: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for c in sig.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            ',' if depth == 0 => {
                raw.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        raw.push(current.trim().to_string());
    }

    raw.iter()
        .map(|segment| {
            let is_separator = *segment == "*" || *segment == "**" || *segment == "/";
            (segment.clone(), is_separator)
        })
        .collect()
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
