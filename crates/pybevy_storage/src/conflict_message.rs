//! Interpreter-neutral system access conflict diagnostics.

use std::fmt;

pub struct SystemAccessConflictMessage<'a> {
    pub system: &'a str,
    pub category: &'a str,
    pub parameter: usize,
    pub mutable: bool,
    pub name: &'a str,
    pub existing_parameter: usize,
    pub existing_mutable: bool,
    pub existing_name: &'a str,
    pub exclusive_read: bool,
    pub remedy: &'a str,
}

impl fmt::Display for SystemAccessConflictMessage<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let access = |mutable| if mutable { "mutable" } else { "immutable" };
        write!(
            f,
            "System '{}' has conflicting {} access:\n- Parameter {} requests {} access to {}\n- Parameter {} already has {} access to {}",
            self.system,
            self.category,
            self.parameter,
            access(self.mutable),
            self.name,
            self.existing_parameter,
            access(self.existing_mutable),
            self.existing_name
        )?;
        if self.exclusive_read {
            write!(
                f,
                "\n{} is stored as a Python object, which is read through a mutable borrow, so even a read-only parameter declares exclusive access to it.",
                self.name
            )?;
        }
        write!(f, "\n{}", self.remedy)
    }
}

pub const CONFLICT_ASSETS: &str = "Two parameters cannot address the same asset collection when either declares write access. Take it once, as ResMut[Assets[...]] if you need to write, or split the parameters across systems.";
pub const CONFLICT_MESSAGES: &str = "A MessageWriter cannot share its channel with another reader or writer in the same system. Split the send and the read into separate systems.";
pub const CONFLICT_WORLD: &str = "World is exclusive: a system taking World cannot take any other data parameter. Reach the other parameters through the World itself, or move them to a separate system.";
pub const CONFLICT_RESOURCES: &str = "Two parameters cannot address the same resource when either declares write access. Take it once, as ResMut[...] if you need to write, or split the parameters across systems.";
pub const CONFLICT_RESOURCE_QUERY: &str = "Query and Res parameters address the same resource entity. Add Without[IsResource] as the query filter to exclude resource entities where representable, or split the parameters across systems.";
pub const CONFLICT_QUERIES: &str = "Different With[...] filters do not prove two queries are disjoint, because one entity could hold both markers. Add Without[...] to each query to prove exclusion (Query[tuple[Mut[Transform], Bat], Without[BatWing]] and Query[tuple[Mut[Transform], BatWing], Without[Bat]]), or split the queries across systems.";
/// A query marker that only one of two conflicting queries requires.
pub struct DisjointMarker<'a> {
    pub name: &'a str,
    /// Written as an explicit `With[...]` filter rather than implied by data.
    pub explicit_with: bool,
}

/// Remedy for two parameters addressing one shared `@material` collection.
pub fn conflict_assets_shared_view(backing: &str) -> String {
    format!(
        "@material classes backed by {backing} are logical views over one shared Assets<{backing}> collection, so these parameters address the same resource. \
Any number of read-only parameters may share it, but a mutable parameter must be the only one: at most one ResMut[Assets[...]], with no Res[Assets[...]] or Assets[...] beside it. \
Split the reads and writes across systems, or add each material in its own system, store the handles in a resource, and order those systems with chain()."
    )
}

/// Query remedy naming the user's own markers; `None` when no marker
/// separates the two queries, so the caller falls back to [`CONFLICT_QUERIES`].
pub fn conflict_queries_disjoint(
    existing_parameter: usize,
    existing_only: Option<&DisjointMarker<'_>>,
    parameter: usize,
    current_only: Option<&DisjointMarker<'_>>,
) -> Option<String> {
    let filter = |own: &DisjointMarker<'_>, excluded: &str| {
        if own.explicit_with {
            format!("tuple[With[{}], Without[{excluded}]]", own.name)
        } else {
            format!("Without[{excluded}]")
        }
    };
    let text = match (existing_only, current_only) {
        (Some(existing), Some(current)) => {
            let example = if existing.explicit_with || current.explicit_with {
                format!(
                    " (filters {} and {})",
                    filter(existing, current.name),
                    filter(current, existing.name)
                )
            } else {
                String::new()
            };
            format!(
                "Different With[...] filters do not prove two queries are disjoint, because one entity could hold both {e} and {c}. \
Add Without[{c}] to parameter {existing_parameter} and Without[{e}] to parameter {parameter} to prove exclusion{example}, or split the queries across systems.",
                e = existing.name,
                c = current.name,
            )
        }
        (Some(marker), None) => one_sided_disjoint(parameter, existing_parameter, marker.name),
        (None, Some(marker)) => one_sided_disjoint(existing_parameter, parameter, marker.name),
        (None, None) => return None,
    };
    Some(text)
}

fn one_sided_disjoint(open: usize, requiring: usize, marker: &str) -> String {
    format!(
        "Parameter {open} can also match entities with {marker}, which parameter {requiring} requires. \
Add Without[{marker}] to parameter {open} to prove exclusion (combine several filters as tuple[...]), or split the queries across systems."
    )
}

pub const CONDITION_READ_ONLY_GUIDANCE: &str = "Run conditions use Bevy's read-only world contract and never apply deferred operations. Accepted parameters are Local, read-only Assets, Res for a native engine resource, and read-only Query/View over wrapper-storage components. For exclusive Python values, use in_state(), a wrapper-storage component, or test the value inside the system.";

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn message<'a>(
        category: &'a str,
        parameter: usize,
        mutable: bool,
        name: &'a str,
        existing_parameter: usize,
        existing_mutable: bool,
        existing_name: &'a str,
        exclusive_read: bool,
        remedy: &'static str,
    ) -> SystemAccessConflictMessage<'a> {
        SystemAccessConflictMessage {
            system: "probe_system",
            category,
            parameter,
            mutable,
            name,
            existing_parameter,
            existing_mutable,
            existing_name,
            exclusive_read,
            remedy,
        }
    }

    #[test]
    fn display_renders_access_words_and_remedy_tail() {
        let text = message(
            "component",
            2,
            true,
            "Transform",
            0,
            false,
            "Transform",
            false,
            CONFLICT_QUERIES,
        )
        .to_string();

        assert!(text.starts_with("System 'probe_system' has conflicting component access:\n"));
        assert!(text.contains("- Parameter 2 requests mutable access to Transform\n"));
        assert!(text.contains("- Parameter 0 already has immutable access to Transform\n"));
        assert!(!text.contains("stored as a Python object"));
        // The remedy constant is the tail the formatter was given, so the
        // output must end with exactly that wiring.
        assert!(text.ends_with(CONFLICT_QUERIES));
    }

    #[test]
    fn display_appends_exclusive_read_note_only_when_flagged() {
        let plain = message(
            "component",
            1,
            false,
            "Score",
            0,
            false,
            "Score",
            false,
            CONFLICT_RESOURCES,
        )
        .to_string();
        assert!(!plain.contains("exclusive access to it"));

        let exclusive = message(
            "component",
            1,
            false,
            "Score",
            0,
            false,
            "Score",
            true,
            CONFLICT_RESOURCES,
        )
        .to_string();
        assert!(exclusive.contains(
            "\nScore is stored as a Python object, which is read through a mutable borrow, so even a read-only parameter declares exclusive access to it.\n"
        ));
        assert!(exclusive.ends_with(CONFLICT_RESOURCES));
    }

    #[test]
    fn display_uses_category_between_conflicting_and_access() {
        let text = message(
            "asset",
            0,
            false,
            "Assets<Image>",
            1,
            true,
            "Assets<Image>",
            false,
            CONFLICT_ASSETS,
        )
        .to_string();
        assert!(text.contains("has conflicting asset access:"));
        assert!(text.contains("- Parameter 0 requests immutable access to Assets<Image>\n"));
        assert!(text.contains("- Parameter 1 already has mutable access to Assets<Image>\n"));
    }
}
