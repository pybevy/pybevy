use std::collections::{HashMap, HashSet};

use crate::bevy_parser::types::{BevyCrate, BevyMethod};

/// Every `From`/`TryFrom` impl in the parsed crates, by type short name.
#[derive(Debug, Default)]
pub struct ConversionIndex {
    sources_by_target: HashMap<String, HashSet<String>>,
    targets_by_source: HashMap<String, HashSet<String>>,
}

impl ConversionIndex {
    pub fn build<'a>(crates: impl IntoIterator<Item = &'a BevyCrate>) -> Self {
        let mut index = Self::default();
        for bevy_crate in crates {
            for item in bevy_crate.items.values() {
                let target = base_name(&item.name);
                for method in &item.methods {
                    index.record(target, method);
                }
            }
            for (key, methods) in &bevy_crate.foreign_methods {
                let target = key.rsplit("::").next().unwrap_or(key);
                for method in methods {
                    index.record(target, method);
                }
            }
        }
        index
    }

    fn record(&mut self, target: &str, method: &BevyMethod) {
        let Some(source) = conversion_source(method) else {
            return;
        };
        if source == target {
            return;
        }
        self.sources_by_target
            .entry(target.to_string())
            .or_default()
            .insert(source.clone());
        self.targets_by_source
            .entry(source)
            .or_default()
            .insert(target.to_string());
    }

    /// Bevy types `target` converts from.
    pub fn sources_of(&self, target: &str) -> impl Iterator<Item = &str> {
        self.sources_by_target
            .get(target)
            .into_iter()
            .flatten()
            .map(String::as_str)
    }

    /// Whether a Python `from_<x>`, `into_<x>` or `to_<x>` member on `owner`
    /// names a Bevy `From`/`TryFrom` impl between `owner` and `x`.
    pub fn explains(&self, owner: &str, member: &str) -> bool {
        if let Some(source) = member.strip_prefix("from_") {
            return self
                .sources_of(owner)
                .any(|candidate| snake_case(candidate) == source);
        }
        let Some(target) = member
            .strip_prefix("into_")
            .or_else(|| member.strip_prefix("to_"))
        else {
            return false;
        };
        self.targets_by_source
            .get(owner)
            .into_iter()
            .flatten()
            .any(|candidate| snake_case(candidate) == target)
    }
}

/// `fn from(value: S) -> Self` inside `impl From<S>` (or `TryFrom<S>`).
fn conversion_source(method: &BevyMethod) -> Option<String> {
    let from_trait = method.from_trait.as_deref()?;
    let trait_name = from_trait.rsplit("::").next().unwrap_or(from_trait);
    let expected = match trait_name {
        "From" => "from",
        "TryFrom" => "try_from",
        _ => return None,
    };
    if method.name != expected || method.parameters.len() != 1 {
        return None;
    }
    let source = method.parameters[0]
        .param_type
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim();
    let source = base_name(source).rsplit("::").next().unwrap_or(source);
    source
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
        .then(|| source.to_string())
        .filter(|source| source != "Self")
}

fn base_name(name: &str) -> &str {
    name.split('<').next().unwrap_or(name).trim()
}

/// Python spelling of a type in a conversion name: `LinearRgba` ->
/// `linear_rgba`, `IVec2` -> `ivec2`, `Mat3A` -> `mat3a`.
pub fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;
    for ch in name.chars() {
        if ch.is_uppercase() && previous_lower {
            out.push('_');
        }
        previous_lower = ch.is_lowercase();
        out.extend(ch.to_lowercase());
    }
    out
}
