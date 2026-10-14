use pybevy_core::public_error;
use pyo3::{prelude::*, types::PyType};

use crate::pyo3::{
    dataclass_fields::dataclass_field_names,
    from_json::enums::{enum_owner_type, enum_variant_names},
};

pub(crate) fn writable_field_names(instance: &Bound<'_, PyAny>) -> Vec<String> {
    let class = instance.get_type();
    if let Ok(Some(names)) = dataclass_field_names(&class) {
        return names.into_iter().collect();
    }
    // Probe a fresh owned copy so a read-only setter error cannot touch the live component.
    let probe = match instance.call_method0("__copy__").or_else(|_| class.call0()) {
        Ok(copy) => copy,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    let Ok(dir_list) = class.dir() else {
        return names;
    };
    for attr_obj in dir_list.iter() {
        let Ok(name) = attr_obj.extract::<String>() else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        let Ok(attr) = class.getattr(name.as_str()) else {
            continue;
        };
        let descriptor = attr
            .get_type()
            .name()
            .ok()
            .map(|kind| kind.to_string_lossy().into_owned())
            .is_some_and(|kind| kind == "getset_descriptor" || kind == "property");
        if !descriptor {
            continue;
        }
        // Assigning the field's own value distinguishes a getter from a getter/setter.
        if probe
            .getattr(name.as_str())
            .and_then(|value| probe.setattr(name.as_str(), value))
            .is_ok()
        {
            names.push(name);
        }
    }
    names.sort();
    names.dedup();
    names
}

fn declares_component_field(py_type: &Bound<'_, PyType>, name: &str) -> bool {
    py_type.getattr(name).is_ok_and(|attr| {
        attr.get_type().name().ok().is_some_and(|kind| {
            matches!(
                kind.to_string_lossy().as_ref(),
                "getset_descriptor" | "property"
            )
        })
    })
}

pub(crate) fn unknown_set_fields<'a>(
    instance: &Bound<'_, PyAny>,
    field_names: impl IntoIterator<Item = &'a String>,
) -> Vec<String> {
    if instance.getattr("__dict__").is_ok() {
        return Vec::new();
    }
    let class = instance.get_type();
    field_names
        .into_iter()
        .filter(|name| {
            instance.getattr(name.as_str()).is_err() && !declares_component_field(&class, name)
        })
        .cloned()
        .collect()
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (i, left_char) in left.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, right_char) in right.iter().enumerate() {
            let cost = usize::from(left_char != right_char);
            current.push(std::cmp::min(
                std::cmp::min(previous[j + 1] + 1, current[j] + 1),
                previous[j] + cost,
            ));
        }
        previous = current;
    }
    previous[right.len()]
}

fn close_field_suggestion(typo: &str, candidates: &[String]) -> Option<String> {
    let typo = typo.to_lowercase();
    let mut best: Option<(usize, &String)> = None;
    for candidate in candidates {
        let distance = levenshtein_distance(&typo, &candidate.to_lowercase());
        if (1..=2).contains(&distance) && best.is_none_or(|(known, _)| distance < known) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate.clone())
}

pub(crate) fn unknown_field_error(
    component: &str,
    unknown: &[String],
    writable: &[String],
) -> String {
    let suggestion = if unknown.len() == 1 {
        close_field_suggestion(&unknown[0], writable)
    } else {
        None
    };
    if let Some(candidate) = suggestion {
        return format!(
            "Failed to set '{component}': unknown field '{}'. Did you mean '{}'?",
            unknown[0], candidate
        );
    }
    let subject = if unknown.len() == 1 {
        format!("unknown field '{}'", unknown[0])
    } else {
        format!("unknown fields: {}", unknown.join(", "))
    };
    if writable.is_empty() {
        return format!(
            "Failed to set '{component}': {subject}. This component exposes no writable fields."
        );
    }
    if writable.len() <= 8 {
        return format!(
            "Failed to set '{component}': {subject}. Valid writable fields: {}.",
            writable.join(", ")
        );
    }
    format!(
        "Failed to set '{component}': {subject}. See GET /api/v1/components/{component}/schema for the writable fields."
    )
}

pub(crate) fn component_unknown_field_error(
    component: &str,
    unknown: &[String],
    instance: &Bound<'_, PyAny>,
) -> String {
    let variants = component_enum_variant_names(instance);
    if variants.is_empty() {
        return unknown_field_error(component, unknown, &writable_field_names(instance));
    }
    let subject = if unknown.len() == 1 {
        format!("unknown field '{}'", unknown[0])
    } else {
        format!("unknown fields: {}", unknown.join(", "))
    };
    public_error::mcp_enum_component_unknown_fields(component, &subject, &variants)
}

fn component_enum_variant_names(instance: &Bound<'_, PyAny>) -> Vec<String> {
    let current_type = instance.get_type();
    let owner = enum_owner_type(instance);
    let variants = enum_variant_names(&owner);
    if variants.is_empty() || !owner.is(&current_type) {
        return super::enum_names::accepted_variant_names(&owner, variants);
    }

    let Ok(owner_name) = owner.name() else {
        return Vec::new();
    };
    let Ok(repr) = instance.repr() else {
        return Vec::new();
    };
    let repr = repr.to_string_lossy();
    let prefix = format!("{}.", owner_name.to_string_lossy());
    if variants.iter().any(|variant| {
        repr.strip_prefix(&prefix).is_some_and(|suffix| {
            suffix == variant
                || suffix
                    .strip_prefix(variant)
                    .is_some_and(|rest| rest.starts_with('('))
        })
    }) {
        super::enum_names::accepted_variant_names(&owner, variants)
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn levenshtein_distance_measures_edits() {
        assert_eq!(levenshtein_distance("translation", "translation"), 0);
        assert_eq!(levenshtein_distance("traslation", "translation"), 1);
        assert_eq!(levenshtein_distance("align_conntent", "align_content"), 1);
        assert_eq!(levenshtein_distance("pos", "scale"), 5);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        // Case-sensitive by contract; case folding happens in close_field_suggestion.
        assert_eq!(levenshtein_distance("ABC", "abc"), 3);
    }

    #[test]
    fn close_field_suggestion_requires_a_genuinely_close_name() {
        let fields: Vec<String> = ["rotation", "scale", "translation"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(
            close_field_suggestion("traslation", &fields).as_deref(),
            Some("translation")
        );
        assert_eq!(
            close_field_suggestion("Translaton", &fields).as_deref(),
            Some("translation")
        );
        assert_eq!(close_field_suggestion("pos", &fields), None);
        assert_eq!(close_field_suggestion("bogus", &fields), None);
    }

    #[test]
    fn close_field_suggestion_breaks_ties_alphabetically() {
        let fields: Vec<String> = ["abcd", "abce"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(
            close_field_suggestion("abcf", &fields).as_deref(),
            Some("abcd")
        );
    }

    #[test]
    fn unknown_field_error_names_field_component_and_close_suggestion() {
        let writable: Vec<String> = ["rotation", "scale", "translation"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let unknown = vec!["traslation".to_string()];
        assert_eq!(
            unknown_field_error("Transform", &unknown, &writable),
            "Failed to set 'Transform': unknown field 'traslation'. Did you mean 'translation'?"
        );
    }

    #[test]
    fn unknown_field_error_lists_small_writable_inventories() {
        let writable: Vec<String> = ["affects_lightmapped_meshes", "brightness", "color"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let unknown = vec!["bogus".to_string()];
        assert_eq!(
            unknown_field_error("AmbientLight", &unknown, &writable),
            "Failed to set 'AmbientLight': unknown field 'bogus'. Valid writable fields: affects_lightmapped_meshes, brightness, color."
        );
    }

    #[test]
    fn unknown_field_error_points_to_schema_for_large_inventories() {
        let writable: Vec<String> = (0..12).map(|index| format!("field_{index}")).collect();
        let unknown = vec!["bogus".to_string()];
        assert_eq!(
            unknown_field_error("Node", &unknown, &writable),
            "Failed to set 'Node': unknown field 'bogus'. See GET /api/v1/components/Node/schema for the writable fields."
        );
    }

    #[test]
    fn unknown_field_error_reports_multiple_unknowns_without_suggestion() {
        let writable: Vec<String> = ["color", "intensity"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let unknown = vec!["bogus".to_string(), "worse".to_string()];
        assert_eq!(
            unknown_field_error("PointLight", &unknown, &writable),
            "Failed to set 'PointLight': unknown fields: bogus, worse. Valid writable fields: color, intensity."
        );
    }

    #[test]
    fn unknown_field_error_names_components_without_writable_fields() {
        let unknown = vec!["bogus".to_string()];
        assert_eq!(
            unknown_field_error("Camera3d", &unknown, &[]),
            "Failed to set 'Camera3d': unknown field 'bogus'. This component exposes no writable fields."
        );
    }
}
