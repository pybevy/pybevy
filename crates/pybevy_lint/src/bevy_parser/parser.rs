use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::Command,
};

use anyhow::{Context, Result, bail};

use super::{
    cache,
    types::{
        BevyCrate, BevyEnumVariant, BevyField, BevyItem, BevyItemKind, BevyMethod, BevyParameter,
        BevyVariantKind, SelfKind,
    },
};

/// Bevy can re-export a dependency wholesale (`pub use glam::*`), so those types are absent
/// from the re-exporting crate's own API. Merge the ones PyBevy wraps; target-defined items win.
pub fn merge_source_types(
    target: &mut BevyCrate,
    source: &BevyCrate,
    wanted_names: &HashSet<String>,
) -> usize {
    let mut merged = 0;
    for (name, item) in &source.items {
        if !matches!(item.kind, BevyItemKind::Struct | BevyItemKind::Enum) {
            continue;
        }
        if !wanted_names.contains(name) || target.items.contains_key(name) {
            continue;
        }
        target.items.insert(name.clone(), item.clone());
        merged += 1;
    }
    merged
}

/// Parse re-export source crates and merge their wrapped types into the parsed
/// Bevy crates. See [`merge_source_types`] for the merge rules.
pub fn merge_reexported_types(
    bevy_crates: &mut HashMap<String, BevyCrate>,
    bevy_path: &Path,
    type_sources: &HashMap<String, Vec<String>>,
    wanted_names: &HashSet<String>,
    use_cache: bool,
) {
    let git_ref = if use_cache {
        cache::get_bevy_git_ref(bevy_path).ok()
    } else {
        None
    };

    for (crate_name, sources) in type_sources {
        if !bevy_crates.contains_key(crate_name) {
            continue;
        }
        for source in sources {
            let parsed = match parse_bevy_crate_impl(bevy_path, source, git_ref.as_deref()) {
                Ok(parsed) => parsed,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse re-export source crate {}: {}",
                        source, e
                    );
                    continue;
                }
            };
            let target = bevy_crates.get_mut(crate_name).expect("checked above");
            let merged = merge_source_types(target, &parsed, wanted_names);
            if merged > 0 {
                eprintln!(
                    "  Merged {} re-exported type(s) from {} into {}",
                    merged, source, crate_name
                );
            }
        }
    }
}

/// Parse multiple Bevy crates
pub fn parse_bevy_crates(
    bevy_path: &Path,
    crate_names: &[&str],
) -> Result<HashMap<String, BevyCrate>> {
    parse_bevy_crates_with_cache(bevy_path, crate_names, true)
}

/// Parse multiple Bevy crates with optional caching
pub fn parse_bevy_crates_with_cache(
    bevy_path: &Path,
    crate_names: &[&str],
    use_cache: bool,
) -> Result<HashMap<String, BevyCrate>> {
    let mut crates = HashMap::new();

    let git_ref = if use_cache {
        cache::get_bevy_git_ref(bevy_path).ok()
    } else {
        None
    };

    for crate_name in crate_names {
        match parse_bevy_crate_impl(bevy_path, crate_name, git_ref.as_deref()) {
            Ok(parsed) => {
                crates.insert(crate_name.to_string(), parsed);
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {}", crate_name, e);
            }
        }
    }

    Ok(crates)
}

/// Parse a single Bevy crate using cargo public-api (with caching enabled by default)
pub fn parse_bevy_crate(bevy_path: &Path, crate_name: &str) -> Result<BevyCrate> {
    let git_ref = cache::get_bevy_git_ref(bevy_path).ok();
    parse_bevy_crate_impl(bevy_path, crate_name, git_ref.as_deref())
}

/// Parse a single Bevy crate with optional caching
fn parse_bevy_crate_impl(
    bevy_path: &Path,
    crate_name: &str,
    git_ref: Option<&str>,
) -> Result<BevyCrate> {
    if let Some(git_ref) = git_ref
        && let Some(cached) = cache::load_cached(git_ref, crate_name)
    {
        eprintln!("  Using cached API for {}", crate_name);
        return Ok(cached);
    }

    eprintln!("  Parsing {} with cargo public-api...", crate_name);

    let output = Command::new("cargo")
        .args(public_api_args(crate_name))
        .current_dir(bevy_path)
        .output()
        .with_context(|| format!("Failed to run cargo public-api for {}", crate_name))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "cargo public-api failed for {}: {}",
            crate_name,
            stderr.lines().take(5).collect::<Vec<_>>().join("\n")
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_public_api_output(crate_name, &stdout)?;

    if let Some(git_ref) = git_ref
        && let Err(e) = cache::save_to_cache(git_ref, crate_name, &result)
    {
        eprintln!("Warning: Failed to cache {}: {}", crate_name, e);
    }

    Ok(result)
}

/// Features that expose modules Bevy gates off by default.
///
/// Bevy hides whole modules behind opt-in features: `bevy_input` ships
/// `gamepad`, `keyboard`, `mouse`, `touch` and `gestures` off, so a
/// default-feature parse reports 5 types where the crate has 46, and every
/// config entry naming a gamepad type looks stale. PyBevy wraps what the `bevy`
/// umbrella crate enables, so the parse has to ask for the same modules.
///
/// This is deliberately a reviewed list rather than `--all-features`. Turning
/// everything on makes Bevy's mutually exclusive backends collide, and
/// `bevy_image`'s `compile_error!` for an unbacked `zstd` then takes
/// `bevy_ecs`, `bevy_app`, `bevy_winit` and `bevy_core_pipeline` down with it.
const CRATE_FEATURES: &[(&str, &[&str])] = &[
    (
        "bevy_input",
        &["gamepad", "gestures", "keyboard", "mouse", "touch"],
    ),
    ("bevy_mesh", &["morph"]),
];

/// Build the `cargo public-api` invocation for one crate.
fn public_api_args(crate_name: &str) -> Vec<String> {
    let mut args = vec![
        "public-api".to_owned(),
        "-p".to_owned(),
        crate_name.to_owned(),
        // Omit blanket impls and auto trait impls
        "-ss".to_owned(),
    ];
    for (name, features) in CRATE_FEATURES {
        if *name == crate_name {
            args.push("--features".to_owned());
            args.push(features.join(","));
        }
    }
    args
}

/// Describe the parse configuration for generated-artifact provenance, so a
/// change to `CRATE_FEATURES` cannot silently outdate what the artifacts claim.
pub fn public_api_flags() -> String {
    let mut description = "-ss".to_owned();
    for (name, features) in CRATE_FEATURES {
        description.push_str(&format!(" --features {name}:{}", features.join(",")));
    }
    description
}

/// Identify the feature set a cache entry was produced with, so widening the
/// features does not reuse an entry parsed under the old, narrower set.
pub fn feature_key(crate_name: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    public_api_args(crate_name).hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Parse the output of cargo public-api
pub fn parse_public_api_output(crate_name: &str, output: &str) -> Result<BevyCrate> {
    let mut items: HashMap<String, BevyItem> = HashMap::new();
    // Track current trait impl context: (type_name, trait_name)
    // When we see "impl Trait for Type", subsequent methods for that type come from that trait
    let mut current_trait_context: Option<(String, String)> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(item) = parse_line(line, crate_name) {
            match item {
                ParsedLine::Item(bevy_item) => {
                    // New type definition resets trait context, but NOT type aliases
                    // (associated types like AudioSource::Decoder don't reset context)
                    if bevy_item.kind != BevyItemKind::TypeAlias {
                        current_trait_context = None;
                    }
                    // Real type definitions outrank aliases sharing a short name:
                    // glam's swizzle aliases (`pub type glam::Vec4::Vec2`) would
                    // otherwise clobber the Vec2 struct and its methods.
                    let existing = items.get(&bevy_item.name);
                    let alias_shadows_definition = bevy_item.kind == BevyItemKind::TypeAlias
                        && matches!(
                            existing.map(|i| &i.kind),
                            Some(BevyItemKind::Struct | BevyItemKind::Enum)
                        );
                    if alias_shadows_definition {
                        continue;
                    }
                    // A definition arriving after an alias with the same short
                    // name (swizzle aliases can precede the type's own section)
                    // replaces it, keeping methods attached to the alias.
                    let mut bevy_item = bevy_item;
                    if let Some(prev) = existing
                        && prev.kind == BevyItemKind::TypeAlias
                        && matches!(bevy_item.kind, BevyItemKind::Struct | BevyItemKind::Enum)
                        && bevy_item.methods.is_empty()
                        && !prev.methods.is_empty()
                    {
                        bevy_item.methods = prev.methods.clone();
                    }
                    items.insert(bevy_item.name.clone(), bevy_item);
                }
                ParsedLine::Method {
                    type_name,
                    mut method,
                } => {
                    if let Some((ctx_type, ctx_trait)) = &current_trait_context
                        && ctx_type == &type_name
                    {
                        method.from_trait = Some(ctx_trait.clone());
                    }

                    // For types with generics, create the item if it doesn't exist
                    // Mark generic templates (e.g., Extrusion<P>) as is_generic_base
                    // so they're used for method inheritance but not reported as unimplemented
                    if !items.contains_key(&type_name) && type_name.contains('<') {
                        let full_path = format!("{}::{}", crate_name, type_name);
                        let is_template = is_type_generic_template(&type_name);
                        let item = if is_template {
                            BevyItem::new_generic_base(full_path, BevyItemKind::Struct)
                        } else {
                            BevyItem::new(full_path, BevyItemKind::Struct)
                        };
                        items.insert(type_name.clone(), item);
                    }

                    if let Some(item) = items.get_mut(&type_name) {
                        item.methods.push(method);
                    }
                }
                ParsedLine::Constant {
                    type_name,
                    name,
                    const_type,
                } => {
                    if let Some(item) = items.get_mut(&type_name) {
                        item.constants.push((name, const_type));
                    }
                }
                ParsedLine::Field {
                    type_name,
                    name,
                    field_type,
                } => {
                    if let Some(item) = items.get_mut(&type_name) {
                        item.fields.push(BevyField {
                            name,
                            field_type,
                            is_public: true,
                        });
                    }
                }
                ParsedLine::TraitImpl {
                    type_name,
                    trait_name,
                } => {
                    current_trait_context = Some((type_name.clone(), trait_name.clone()));
                    if let Some(item) = items.get_mut(&type_name) {
                        item.trait_impls.push(trait_name);
                    }
                }
                ParsedLine::InherentImpl {
                    type_name,
                    is_generic,
                } => {
                    // For specialized types (e.g., Time<Virtual>) or generic bases (e.g., Time<T>),
                    // create the item if it doesn't exist
                    if !items.contains_key(&type_name) && type_name.contains('<') {
                        let full_path = format!("{}::{}", crate_name, type_name);
                        if is_generic {
                            // This is a generic base type like Time<T>
                            items.insert(
                                type_name.clone(),
                                BevyItem::new_generic_base(full_path, BevyItemKind::Struct),
                            );
                        } else {
                            // This is a specialized type like Time<Virtual>
                            items.insert(
                                type_name.clone(),
                                BevyItem::new(full_path, BevyItemKind::Struct),
                            );
                        }
                    }

                    // Inherent impl resets trait context (methods are not from a trait)
                    current_trait_context = Some((type_name, String::new()));
                }
                ParsedLine::Variant { enum_name, variant } => {
                    if let Some(item) = items.get_mut(&enum_name) {
                        item.variants.push(variant);
                    }
                }
                ParsedLine::StructVariantField {
                    enum_name,
                    variant_name,
                    field_name,
                    field_type,
                } => {
                    if let Some(item) = items.get_mut(&enum_name) {
                        for v in &mut item.variants {
                            if v.name == variant_name {
                                match &mut v.kind {
                                    BevyVariantKind::Struct(fields) => {
                                        fields.push((field_name.clone(), field_type.clone()));
                                    }
                                    BevyVariantKind::Unit => {
                                        v.kind = BevyVariantKind::Struct(vec![(
                                            field_name.clone(),
                                            field_type.clone(),
                                        )]);
                                    }
                                    BevyVariantKind::Tuple(_) => {
                                        // Shouldn't happen, but convert to struct
                                        v.kind = BevyVariantKind::Struct(vec![(
                                            field_name.clone(),
                                            field_type.clone(),
                                        )]);
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
                ParsedLine::Skip => {}
            }
        }
    }

    Ok(BevyCrate {
        name: crate_name.to_string(),
        items,
    })
}

enum ParsedLine {
    Item(BevyItem),
    Method {
        type_name: String,
        method: BevyMethod,
    },
    Constant {
        type_name: String,
        name: String,
        const_type: String,
    },
    Field {
        type_name: String,
        name: String,
        field_type: String,
    },
    Variant {
        enum_name: String,
        variant: BevyEnumVariant,
    },
    StructVariantField {
        enum_name: String,
        variant_name: String,
        field_name: String,
        field_type: String,
    },
    TraitImpl {
        type_name: String,
        trait_name: String,
    },
    /// Inherent impl block. is_generic is true for `impl<T> Type<T>` style impls
    InherentImpl {
        type_name: String,
        is_generic: bool,
    },
    Skip,
}

/// Strip leading outer attributes (`#[repr(C)]`, `#[non_exhaustive]`, ...)
/// that cargo-public-api prefixes to definition lines. Without this, types
/// like glam's Vec2 or bevy_math's Rect are invisible to the parser.
fn strip_outer_attributes(line: &str) -> &str {
    let mut rest = line.trim_start();
    while rest.starts_with("#[") {
        match find_attribute_end(rest) {
            Some(end) => rest = rest[end..].trim_start(),
            None => break,
        }
    }
    rest
}

fn find_attribute_end(attr: &str) -> Option<usize> {
    let bytes = attr.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'(' | b'{' => depth += 1,
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            b')' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn parse_line(line: &str, crate_name: &str) -> Option<ParsedLine> {
    let line = strip_outer_attributes(line);

    if line.starts_with("pub mod ") {
        return Some(ParsedLine::Skip);
    }

    if let Some(rest) = line.strip_prefix("pub struct ") {
        let path = rest.split(&['(', '<', ' '][..]).next()?;
        // Validate we can extract a type name
        let _ = extract_type_name(path)?;

        return Some(ParsedLine::Item(BevyItem::new(
            path.to_string(),
            BevyItemKind::Struct,
        )));
    }

    if let Some(rest) = line.strip_prefix("pub enum ") {
        let path = rest.split(&['<', ' '][..]).next()?;
        // Validate we can extract a type name
        let _ = extract_type_name(path)?;

        return Some(ParsedLine::Item(BevyItem::new(
            path.to_string(),
            BevyItemKind::Enum,
        )));
    }

    if let Some(rest) = line.strip_prefix("pub trait ") {
        // Split at < (generics) or space, then handle trait bounds (single : not ::)
        let path = rest.split(&['<', ' '][..]).next()?;
        // Remove trailing : from trait bound if present (e.g., "Trait:" from "Trait: Bound")
        let path = path.trim_end_matches(':');

        return Some(ParsedLine::Item(BevyItem::new(
            path.to_string(),
            BevyItemKind::Trait,
        )));
    }

    if line.starts_with("impl") && !line.contains(" for ") {
        let is_generic = line.starts_with("impl<");

        let rest = if is_generic {
            // Skip past the generic params: "impl<T: Default>" -> find the closing >
            let impl_start = line.find("> ")?;
            &line[impl_start + 2..]
        } else {
            line.strip_prefix("impl ")?
        };

        // For specialized types like Time<Virtual>, preserve the generic parameter
        // This creates separate items for Time vs Time<Virtual>
        // Use _for_impl variant to allow generic templates like Time<T> for method inheritance
        if let Some(type_name) = extract_type_name_with_generics_for_impl(rest.trim()) {
            return Some(ParsedLine::InherentImpl {
                type_name,
                is_generic,
            });
        }
        return Some(ParsedLine::Skip);
    }

    if line.starts_with("impl ")
        && line.contains(" for ")
        && let Some((trait_part, type_part)) = line.strip_prefix("impl ")?.split_once(" for ")
    {
        let type_name = extract_type_name(type_part.split(&['<', ' '][..]).next()?)?;
        let trait_name = trait_part.split(&['<', ' '][..]).next()?.to_string();

        if trait_name.contains("Reflect")
            || trait_name.contains("TypePath")
            || trait_name.contains("GetTypeRegistration")
        {
            return Some(ParsedLine::Skip);
        }

        return Some(ParsedLine::TraitImpl {
            type_name,
            trait_name,
        });
    }

    if line.starts_with("pub fn ")
        || line.starts_with("pub const fn ")
        || line.starts_with("pub unsafe fn ")
    {
        return parse_method_line(line, crate_name);
    }

    if line.starts_with("pub const ") && !line.starts_with("pub const fn ") {
        return parse_constant_line(line);
    }

    if line.starts_with("pub ")
        && !line.starts_with("pub fn ")
        && !line.starts_with("pub struct ")
        && !line.starts_with("pub enum ")
        && !line.starts_with("pub mod ")
        && !line.starts_with("pub type ")
        && !line.starts_with("pub trait ")
        && !line.starts_with("pub const ")
        && !line.starts_with("pub unsafe ")
    {
        return parse_field_or_variant_line(line);
    }

    if let Some(rest) = line.strip_prefix("pub type ") {
        let path = rest.split(&['<', ' ', '='][..]).next()?;

        return Some(ParsedLine::Item(BevyItem::new(
            path.to_string(),
            BevyItemKind::TypeAlias,
        )));
    }

    Some(ParsedLine::Skip)
}

fn parse_method_line(line: &str, _crate_name: &str) -> Option<ParsedLine> {
    let is_const = line.contains("const fn ");
    let is_unsafe = line.contains("unsafe fn ");

    let fn_start = line.find("fn ")? + 3;
    let rest = &line[fn_start..];

    let paren_pos = rest.find('(')?;
    let path = &rest[..paren_pos];

    // For specialized types like Time<Virtual>::pause, we need to handle the generics
    // Use find_last_path_separator to avoid splitting on :: inside generic bounds
    // e.g., "Srgba::hex<T: core::convert::AsRef<str>>" should split at Srgba::hex, not AsRef<str>
    let sep_pos = find_last_path_separator(path)?;
    let type_path = &path[..sep_pos];
    let method_with_generics = &path[sep_pos + 2..]; // Skip the "::"

    let method_name = method_with_generics
        .split('<')
        .next()
        .unwrap_or(method_with_generics)
        .to_string();

    // Use _for_impl variant to allow methods on generic templates (for method inheritance)
    let type_name = extract_type_name_with_generics_for_impl(type_path)?;

    let params_start = rest.find('(')?;
    let params_end = find_matching_paren(rest, params_start)?;
    let params_str = &rest[params_start + 1..params_end];

    let (self_kind, parameters) = parse_parameters(params_str);

    let after_params = &rest[params_end + 1..];
    let return_type = after_params
        .find(" -> ")
        .map(|pos| after_params[pos + 4..].trim().to_string());

    let method = BevyMethod {
        name: method_name,
        signature: line.to_string(),
        parameters,
        return_type,
        self_kind,
        is_const,
        is_unsafe,
        is_async: line.contains("async fn "),
        from_trait: None, // Set later in parse_public_api_output based on context
    };

    Some(ParsedLine::Method { type_name, method })
}

fn parse_constant_line(line: &str) -> Option<ParsedLine> {
    let rest = &line["pub const ".len()..];

    let colon_pos = rest.find(": ")?;
    let path = &rest[..colon_pos];
    let const_type = rest[colon_pos + 2..].trim().to_string();

    let parts: Vec<&str> = path.rsplitn(2, "::").collect();
    if parts.len() < 2 {
        return Some(ParsedLine::Skip);
    }

    let const_name = parts[0].to_string();
    let type_path = parts[1];

    // Use _for_impl variant to allow constants on generic templates
    let type_name = extract_type_name_with_generics_for_impl(type_path)?;

    Some(ParsedLine::Constant {
        type_name,
        name: const_name,
        const_type,
    })
}

fn parse_field_or_variant_line(line: &str) -> Option<ParsedLine> {
    let rest = &line["pub ".len()..];

    if let Some(colon_pos) = rest.find(": ") {
        let path = &rest[..colon_pos];
        let field_type = rest[colon_pos + 2..].trim().to_string();

        // Count path segments to determine if this is a struct field or struct variant field
        let segments: Vec<&str> = path.split("::").collect();

        // Check for struct variant field pattern:
        // Format: module::path::EnumName::VariantName::field_name
        // We need at least 4 segments and TWO consecutive uppercase segments before the field
        // e.g., ["bevy_window", "FileDragAndDrop", "DroppedFile", "path_buf"]
        //                       ^enum             ^variant       ^field
        if segments.len() >= 4 {
            let last = segments[segments.len() - 1];
            let variant = segments[segments.len() - 2];
            let enum_name = segments[segments.len() - 3];

            let last_is_lower = last
                .chars()
                .next()
                .map(|c| c.is_lowercase())
                .unwrap_or(false);
            let variant_is_upper = variant
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false);
            let enum_is_upper = enum_name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false);

            // Struct variant field: both enum and variant names are uppercase, field is lowercase
            if last_is_lower && variant_is_upper && enum_is_upper {
                return Some(ParsedLine::StructVariantField {
                    enum_name: enum_name.to_string(),
                    variant_name: variant.to_string(),
                    field_name: last.to_string(),
                    field_type,
                });
            }
        }

        // Regular struct field
        let parts: Vec<&str> = path.rsplitn(2, "::").collect();
        if parts.len() < 2 {
            return Some(ParsedLine::Skip);
        }

        let field_name = parts[0].to_string();
        let type_path = parts[1];
        let type_name = extract_type_name(type_path)?;

        // Skip if field_name looks like a type (starts with uppercase) - likely a constant
        if field_name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            return Some(ParsedLine::Skip);
        }

        return Some(ParsedLine::Field {
            type_name,
            name: field_name,
            field_type,
        });
    }

    if let Some(paren_pos) = rest.find('(') {
        let path = &rest[..paren_pos];
        let parts: Vec<&str> = path.rsplitn(2, "::").collect();
        if parts.len() < 2 {
            return Some(ParsedLine::Skip);
        }

        let variant_name = parts[0].to_string();
        let enum_path = parts[1];
        let enum_name = extract_type_name(enum_path)?;

        // Extract tuple field types
        let close_paren = rest.rfind(')')?;
        let tuple_content = &rest[paren_pos + 1..close_paren];
        let field_types: Vec<String> = if tuple_content.trim().is_empty() {
            vec![]
        } else {
            split_type_list(tuple_content)
        };

        return Some(ParsedLine::Variant {
            enum_name,
            variant: BevyEnumVariant {
                name: variant_name,
                kind: BevyVariantKind::Tuple(field_types),
            },
        });
    }

    let path = if let Some(eq_pos) = rest.find(" = ") {
        &rest[..eq_pos]
    } else {
        rest
    };

    let parts: Vec<&str> = path.rsplitn(2, "::").collect();
    if parts.len() < 2 {
        return Some(ParsedLine::Skip);
    }

    let variant_name = parts[0].to_string();
    let enum_path = parts[1];

    if !variant_name
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
    {
        return Some(ParsedLine::Skip);
    }

    let enum_name = extract_type_name(enum_path)?;

    Some(ParsedLine::Variant {
        enum_name,
        variant: BevyEnumVariant {
            name: variant_name,
            kind: BevyVariantKind::Unit,
        },
    })
}

/// Split a comma-separated type list, respecting nested generics
fn split_type_list(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut current = String::new();

    for c in s.chars() {
        match c {
            '<' | '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            '>' | ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    result.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        result.push(trimmed);
    }

    result
}

fn parse_parameters(params_str: &str) -> (SelfKind, Vec<BevyParameter>) {
    let mut self_kind = SelfKind::None;
    let mut parameters = Vec::new();

    if params_str.trim().is_empty() {
        return (self_kind, parameters);
    }

    let mut depth = 0;
    let mut current = String::new();
    let mut params = Vec::new();

    for c in params_str.chars() {
        match c {
            '<' | '(' | '[' => {
                depth += 1;
                current.push(c);
            }
            '>' | ')' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                params.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        params.push(current.trim().to_string());
    }

    for param in params {
        let param = param.trim();

        // Check for self parameter
        if param == "self" {
            self_kind = SelfKind::Owned;
            continue;
        }
        if param == "&self" {
            self_kind = SelfKind::Ref;
            continue;
        }
        if param == "&mut self" {
            self_kind = SelfKind::RefMut;
            continue;
        }
        if param.starts_with("self:") {
            // Handle self: Box<Self>, etc.
            self_kind = SelfKind::Owned;
            continue;
        }

        // Parse regular parameter: "name: Type"
        if let Some(colon_pos) = param.find(':') {
            let name = param[..colon_pos].trim().to_string();
            let param_type = param[colon_pos + 1..].trim().to_string();
            parameters.push(BevyParameter { name, param_type });
        }
    }

    (self_kind, parameters)
}

/// Find the position of the matching closing parenthesis
fn find_matching_paren(s: &str, open_pos: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s[open_pos..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_pos + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Find the last occurrence of "::" at depth 0 (not inside angle brackets)
/// Returns the position of the first ':' in "::"
fn find_last_path_separator(s: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut last_pos = None;
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        match chars[i] {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ':' if depth == 0 && i + 1 < chars.len() && chars[i + 1] == ':' => {
                last_pos = Some(i);
            }
            _ => {}
        }
    }

    last_pos
}

/// Check if a type name is a standard library type that shouldn't be tracked
fn is_std_type(base_name: &str) -> bool {
    matches!(
        base_name,
        "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "Arc"
            | "Rc"
            | "String"
            | "str"
            | "HashMap"
            | "HashSet"
            | "BTreeMap"
            | "BTreeSet"
            | "Cow"
            | "RefCell"
            | "Cell"
            | "Mutex"
            | "RwLock"
            | "PhantomData"
            | "NonZero"
            | "Duration"
            | "Instant"
            | "Range"
            | "RangeInclusive"
    )
}

/// Check if generic parameters indicate a generic template (not a concrete specialization)
/// Generic templates have single uppercase letters as parameters: <T>, <P>, <A>, etc.
/// Concrete specializations have actual type names: <Virtual>, <Fixed>, <Mesh>, etc.
/// Also filters lifetime parameters like <'a>, <'_> which can't be exposed to Python.
fn is_generic_template_params(simplified_generics: &str) -> bool {
    let inner = simplified_generics
        .trim_start_matches('<')
        .trim_end_matches('>');

    for param in inner.split(',') {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }

        // Lifetime parameters (e.g., 'a, '_, 'static) - can't be exposed to Python
        if param.starts_with('\'') {
            return true;
        }

        // Single uppercase letter = generic template parameter
        if param.len() == 1
            && param
                .chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
        {
            return true;
        }

        // Common generic parameter names
        if matches!(
            param,
            "T" | "A"
                | "P"
                | "S"
                | "M"
                | "K"
                | "V"
                | "E"
                | "F"
                | "D"
                | "R"
                | "W"
                | "N"
                | "Item"
                | "Output"
                | "Error"
                | "Target"
                | "Source"
        ) {
            return true;
        }
    }

    false
}

/// Check if a type name is a generic template (e.g., "Extrusion<P>", "Time<T>")
/// Works on the already-extracted type name, not a full path.
fn is_type_generic_template(type_name: &str) -> bool {
    if let Some(bracket_pos) = type_name.find('<') {
        let generics = &type_name[bracket_pos..];
        is_generic_template_params(generics)
    } else {
        false
    }
}

/// Check if a string has balanced angle brackets
fn has_balanced_brackets(s: &str) -> bool {
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Extract the simple type name from a full path (strips generics)
fn extract_type_name(path: &str) -> Option<String> {
    // Handle generic types: "Type<T>" -> "Type"
    let path = path.split('<').next()?;

    // Get last segment
    let name = path.rsplit("::").next()?;

    if name.chars().next()?.is_lowercase() && !name.contains('_') {
        return None;
    }

    if is_std_type(name) {
        return None;
    }

    Some(name.to_string())
}

/// Extract the type name with generics preserved (for specialized impls)
/// E.g., "bevy_time::time::Time<bevy_time::virt::Virtual>" -> "Time<Virtual>"
/// Returns None for types that should be skipped (std types, generic templates, malformed)
#[allow(dead_code)] // Used in tests, kept for API completeness
fn extract_type_name_with_generics(path: &str) -> Option<String> {
    extract_type_name_with_generics_impl(path, false)
}

/// Extract the type name with generics, allowing generic templates like Time<T>
/// Used for inherent impls where we need to track generic base types for method inheritance
fn extract_type_name_with_generics_for_impl(path: &str) -> Option<String> {
    extract_type_name_with_generics_impl(path, true)
}

/// Implementation for type name extraction with generics
/// If `allow_generic_templates` is true, types like Time<T> are kept (for inherent impls)
fn extract_type_name_with_generics_impl(
    path: &str,
    allow_generic_templates: bool,
) -> Option<String> {
    if !has_balanced_brackets(path) {
        return None;
    }

    if let Some(bracket_pos) = path.find('<') {
        // Get base type name (before generics)
        let base_path = &path[..bracket_pos];
        let base_name = base_path.rsplit("::").next()?;

        // Skip if it looks like a lowercase module name
        if base_name.chars().next()?.is_lowercase() && !base_name.contains('_') {
            return None;
        }

        // Skip standard library types
        if is_std_type(base_name) {
            return None;
        }

        // Extract the generic parameters
        let generics = &path[bracket_pos..];

        // Simplify the generic parameters (extract just the type names)
        let simplified_generics = simplify_generics(generics);

        // Skip generic templates unless we're extracting for inherent impls
        // Generic templates have single-letter type params like <T>, <P>, <A>
        if !allow_generic_templates && is_generic_template_params(&simplified_generics) {
            return None;
        }

        Some(format!("{}{}", base_name, simplified_generics))
    } else {
        // No generics, use regular extraction
        extract_type_name(path)
    }
}

/// Simplify generic parameters by extracting just the type names
/// E.g., "<bevy_time::virt::Virtual>" -> "<Virtual>"
fn simplify_generics(generics: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    let mut current_path = String::new();

    for ch in generics.chars() {
        match ch {
            '<' => {
                depth += 1;
                result.push('<');
            }
            '>' => {
                // Emit the simplified current path
                if !current_path.is_empty() {
                    let simple_name = current_path
                        .rsplit("::")
                        .next()
                        .unwrap_or(&current_path)
                        .to_string();
                    result.push_str(&simple_name);
                    current_path.clear();
                }
                depth -= 1;
                result.push('>');
            }
            ',' => {
                // Emit the simplified current path
                if !current_path.is_empty() {
                    let simple_name = current_path
                        .rsplit("::")
                        .next()
                        .unwrap_or(&current_path)
                        .to_string();
                    result.push_str(&simple_name);
                    current_path.clear();
                }
                result.push_str(", ");
            }
            ' ' => {
                // Skip whitespace
            }
            _ => {
                if depth > 0 {
                    current_path.push(ch);
                }
            }
        }
    }

    result
}
