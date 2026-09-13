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
pub const CONFLICT_ASSETS_SHARED_VIEW: &str = "Every @material class is a logical view over one underlying Assets<ShaderMaterial> collection, so two Assets[...] parameters for different @material types address the same resource. Use one system per material type.";
pub const CONFLICT_MESSAGES: &str = "A MessageWriter cannot share its channel with another reader or writer in the same system. Split the send and the read into separate systems.";
pub const CONFLICT_WORLD: &str = "World is exclusive: a system taking World cannot take any other data parameter. Reach the other parameters through the World itself, or move them to a separate system.";
pub const CONFLICT_RESOURCES: &str = "Two parameters cannot address the same resource when either declares write access. Take it once, as ResMut[...] if you need to write, or split the parameters across systems.";
pub const CONFLICT_RESOURCE_QUERY: &str = "Query and Res parameters address the same resource entity. Add Without[IsResource] as the query filter to exclude resource entities where representable, or split the parameters across systems.";
pub const CONFLICT_QUERIES: &str = "Different With[...] filters do not prove two queries are disjoint, because one entity could hold both markers. Add Without[...] to each query to prove exclusion (Query[tuple[Mut[Transform], Bat], Without[BatWing]] and Query[tuple[Mut[Transform], BatWing], Without[Bat]]), or split the queries across systems.";
