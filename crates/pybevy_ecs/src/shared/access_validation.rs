use std::collections::{HashMap, HashSet};

/// Filter information for disjointness checking.
///
/// Two queries are disjoint if one requires a component (With) that the other
/// excludes (Without). This matches Bevy's `FilteredAccess::is_ruled_out_by()` logic.
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    /// Component types that must be present (With\[...\])
    pub with: HashSet<String>,
    /// Component types that must be absent (Without\[...\])
    pub without: HashSet<String>,
}

impl QueryFilters {
    /// Check if two filter sets prove that queries are disjoint.
    ///
    /// Two queries are disjoint if:
    /// - Query A has With\[X\] AND Query B has Without\[X\], OR
    /// - Query A has Without\[X\] AND Query B has With\[X\]
    pub fn is_disjoint_from(&self, other: &QueryFilters) -> bool {
        for comp in &self.with {
            if other.without.contains(comp) {
                return true;
            }
        }
        for comp in &self.without {
            if other.with.contains(comp) {
                return true;
            }
        }
        false
    }
}

/// A single component access within a system parameter.
pub struct ComponentAccess<K> {
    pub key: K,
    pub name: String,
    pub mutable: bool,
}

/// What access a single system parameter requires.
///
/// Callers convert their parameter types into this enum so the validation
/// algorithm can check for conflicts without knowing the concrete types.
pub enum ParamAccess<K> {
    /// Query or View with component accesses and filters for disjointness checking
    Components {
        accesses: Vec<ComponentAccess<K>>,
        filters: QueryFilters,
    },
    /// Resource access (Res/ResMut)
    Resource {
        key: usize,
        name: String,
        mutable: bool,
    },
    /// Assets access (Res\<Assets\<T\>\> / ResMut\<Assets\<T\>\>)
    Assets {
        key: String,
        name: String,
        mutable: bool,
    },
    /// Exclusive world access
    World,
    /// No conflict (Commands, Local, MessageWriter, etc.)
    None,
}

/// Conflict information for component access validation errors.
pub struct ComponentAccessConflict {
    pub param_idx: usize,
    pub mutable: bool,
    pub comp_name: String,
    pub existing_idx: usize,
    pub existing_mut: bool,
    pub existing_name: String,
}

/// Validate that a set of system parameter accesses don't conflict.
///
/// Checks for:
/// - Multiple mutable accesses to the same component (unless queries are disjoint via filters)
/// - Mixed mutable/immutable access to the same component (unless disjoint)
/// - Multiple mutable accesses to the same resource
/// - World parameter conflicting with any other parameter
///
/// The `accesses` slice is indexed by parameter position — the index is used
/// in conflict error messages.
pub fn validate_access<K: std::hash::Hash + Eq + Clone>(
    accesses: &[ParamAccess<K>],
) -> Result<(), ComponentAccessConflict> {
    // Track component access across all parameters.
    // Uses Vec to support N-way disjoint checking: each new query is checked
    // against ALL previous queries for that component, not just the first.
    let mut component_access: HashMap<K, Vec<(usize, bool, String, QueryFilters)>> =
        HashMap::new();

    // Track resource access (for Res/ResMut conflicts)
    let mut resource_access: HashMap<usize, (usize, bool, String)> = HashMap::new();

    // Track assets access (for Res<Assets<T>> / ResMut<Assets<T>> conflicts)
    let mut assets_access: HashMap<String, (usize, bool, String)> = HashMap::new();

    // Track if World parameter exists (World is exclusive with everything)
    let mut world_param_idx: Option<usize> = None;

    for (param_idx, param) in accesses.iter().enumerate() {
        match param {
            ParamAccess::Components {
                accesses: comp_accesses,
                filters: current_filters,
            } => {
                for comp in comp_accesses {
                    // Check against ALL previous accesses to this component
                    if let Some(existing_accesses) = component_access.get(&comp.key) {
                        for (existing_idx, existing_mut, existing_name, existing_filters) in
                            existing_accesses
                        {
                            if (comp.mutable || *existing_mut)
                                && !current_filters.is_disjoint_from(existing_filters)
                            {
                                return Err(ComponentAccessConflict {
                                    param_idx,
                                    mutable: comp.mutable,
                                    comp_name: comp.name.clone(),
                                    existing_idx: *existing_idx,
                                    existing_mut: *existing_mut,
                                    existing_name: existing_name.clone(),
                                });
                            }
                        }
                    }

                    // Always record this access for future checks
                    component_access.entry(comp.key.clone()).or_default().push((
                        param_idx,
                        comp.mutable,
                        comp.name.clone(),
                        current_filters.clone(),
                    ));
                }

                // Check for World conflict
                if let Some(world_idx) = world_param_idx {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: false,
                        comp_name: "Query".to_string(),
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                    });
                }
            }

            ParamAccess::Resource {
                key,
                name,
                mutable,
            } => {
                if let Some((existing_idx, existing_mut, existing_name)) =
                    resource_access.get(key)
                {
                    if *mutable || *existing_mut {
                        return Err(ComponentAccessConflict {
                            param_idx,
                            mutable: *mutable,
                            comp_name: name.clone(),
                            existing_idx: *existing_idx,
                            existing_mut: *existing_mut,
                            existing_name: existing_name.clone(),
                        });
                    }
                } else {
                    resource_access.insert(*key, (param_idx, *mutable, name.clone()));
                }

                // Check for World conflict
                if let Some(world_idx) = world_param_idx {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: *mutable,
                        comp_name: if *mutable { "ResMut" } else { "Res" }.to_string(),
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                    });
                }
            }

            ParamAccess::Assets {
                key,
                name,
                mutable,
            } => {
                if let Some((existing_idx, existing_mut, existing_name)) =
                    assets_access.get(key)
                {
                    if *mutable || *existing_mut {
                        return Err(ComponentAccessConflict {
                            param_idx,
                            mutable: *mutable,
                            comp_name: format!("Assets<{}>", name),
                            existing_idx: *existing_idx,
                            existing_mut: *existing_mut,
                            existing_name: format!("Assets<{}>", existing_name),
                        });
                    }
                } else {
                    assets_access.insert(key.clone(), (param_idx, *mutable, name.clone()));
                }

                // Check for World conflict
                if let Some(world_idx) = world_param_idx {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: *mutable,
                        comp_name: format!("Assets<{}>", name),
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                    });
                }
            }

            ParamAccess::World => {
                // World is exclusive - conflicts with everything
                if let Some(world_idx) = world_param_idx {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: true,
                        comp_name: "World".to_string(),
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                    });
                }

                // Check if any other parameters exist
                if !component_access.is_empty()
                    || !resource_access.is_empty()
                    || !assets_access.is_empty()
                {
                    let (existing_idx, existing_name) =
                        if let Some(entries) = component_access.values().next() {
                            let (idx, _, name, _) = &entries[0];
                            (*idx, name.clone())
                        } else if let Some((idx, _, name)) = resource_access.values().next() {
                            (*idx, name.clone())
                        } else if let Some((idx, _, name)) = assets_access.values().next() {
                            (*idx, name.clone())
                        } else {
                            unreachable!()
                        };

                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: true,
                        comp_name: "World".to_string(),
                        existing_idx,
                        existing_mut: false,
                        existing_name,
                    });
                }

                world_param_idx = Some(param_idx);
            }

            ParamAccess::None => {}
        }
    }

    Ok(())
}
