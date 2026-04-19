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
    let mut component_access: HashMap<K, Vec<(usize, bool, String, QueryFilters)>> = HashMap::new();

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

            ParamAccess::Resource { key, name, mutable } => {
                if let Some((existing_idx, existing_mut, existing_name)) = resource_access.get(key)
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

            ParamAccess::Assets { key, name, mutable } => {
                if let Some((existing_idx, existing_mut, existing_name)) = assets_access.get(key) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn filters(with: &[&str], without: &[&str]) -> QueryFilters {
        QueryFilters {
            with: with.iter().map(|s| s.to_string()).collect(),
            without: without.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn comp(key: &str, mutable: bool) -> ComponentAccess<String> {
        ComponentAccess {
            key: key.to_string(),
            name: key.to_string(),
            mutable,
        }
    }

    fn query(components: Vec<ComponentAccess<String>>, f: QueryFilters) -> ParamAccess<String> {
        ParamAccess::Components {
            accesses: components,
            filters: f,
        }
    }

    fn res(key: usize, name: &str, mutable: bool) -> ParamAccess<String> {
        ParamAccess::Resource {
            key,
            name: name.to_string(),
            mutable,
        }
    }

    fn assets(key: &str, mutable: bool) -> ParamAccess<String> {
        ParamAccess::Assets {
            key: key.to_string(),
            name: key.to_string(),
            mutable,
        }
    }

    #[test]
    fn empty_filters_not_disjoint() {
        let a = QueryFilters::default();
        let b = QueryFilters::default();
        assert!(!a.is_disjoint_from(&b));
    }

    #[test]
    fn with_vs_without_is_disjoint() {
        let a = filters(&["Transform"], &[]);
        let b = filters(&[], &["Transform"]);
        assert!(a.is_disjoint_from(&b));
    }

    #[test]
    fn without_vs_with_is_disjoint() {
        let a = filters(&[], &["Transform"]);
        let b = filters(&["Transform"], &[]);
        assert!(a.is_disjoint_from(&b));
    }

    #[test]
    fn same_with_not_disjoint() {
        let a = filters(&["Transform"], &[]);
        let b = filters(&["Transform"], &[]);
        assert!(!a.is_disjoint_from(&b));
    }

    #[test]
    fn unrelated_filters_not_disjoint() {
        let a = filters(&["Transform"], &[]);
        let b = filters(&["Velocity"], &[]);
        assert!(!a.is_disjoint_from(&b));
    }

    #[test]
    fn partial_overlap_disjoint() {
        // A: With[Player, Transform], Without[]
        // B: With[Enemy], Without[Player]
        // Player in A's with, Player in B's without → disjoint
        let a = filters(&["Player", "Transform"], &[]);
        let b = filters(&["Enemy"], &["Player"]);
        assert!(a.is_disjoint_from(&b));
    }

    #[test]
    fn empty_params_ok() {
        assert!(validate_access::<String>(&[]).is_ok());
    }

    #[test]
    fn single_readonly_query_ok() {
        let params = [query(
            vec![comp("Transform", false)],
            filters(&["Transform"], &[]),
        )];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn none_params_ignored() {
        let params = [ParamAccess::<String>::None, ParamAccess::None];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn two_readonly_same_component_ok() {
        let params = [
            query(vec![comp("Transform", false)], filters(&["Transform"], &[])),
            query(vec![comp("Transform", false)], filters(&["Transform"], &[])),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn read_and_mut_same_component_conflicts() {
        let params = [
            query(vec![comp("Transform", false)], filters(&["Transform"], &[])),
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert_eq!(err.existing_idx, 0);
        assert!(err.mutable);
        assert_eq!(err.comp_name, "Transform");
    }

    #[test]
    fn two_mut_same_component_conflicts() {
        let params = [
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert_eq!(err.existing_idx, 0);
    }

    #[test]
    fn mut_overlap_with_disjoint_filters_ok() {
        // Query[Mut[Transform], With[Player]] vs Query[Mut[Transform], With[Enemy]]
        // Disjoint because Player in A's With, Player absent from B → not disjoint
        // BUT: With[Player] vs Without[Player] would be disjoint
        // Actually need: A With[Player], B Without[Player]
        let params = [
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Player"], &[]),
            ),
            query(
                vec![comp("Transform", true)],
                filters(&["Transform"], &["Player"]),
            ),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn mut_overlap_non_disjoint_filters_conflicts() {
        // Both have With[Player] — not disjoint
        let params = [
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Player"], &[]),
            ),
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Player"], &[]),
            ),
        ];
        assert!(validate_access(&params).is_err());
    }

    #[test]
    fn different_components_ok() {
        let params = [
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
            query(vec![comp("Velocity", true)], filters(&["Velocity"], &[])),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn multi_component_query_partial_overlap_conflicts() {
        // Query[Mut[Transform], Velocity] vs Query[Mut[Transform], Health]
        // Transform is mut in both → conflict
        let params = [
            query(
                vec![comp("Transform", true), comp("Velocity", false)],
                filters(&["Transform", "Velocity"], &[]),
            ),
            query(
                vec![comp("Transform", true), comp("Health", false)],
                filters(&["Transform", "Health"], &[]),
            ),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.comp_name, "Transform");
    }

    #[test]
    fn three_way_conflict_detected() {
        // A: read Transform, B: read Transform, C: mut Transform
        // A-B ok, A-C conflict (or B-C conflict — C is param_idx=2)
        let params = [
            query(vec![comp("Transform", false)], filters(&["Transform"], &[])),
            query(vec![comp("Transform", false)], filters(&["Transform"], &[])),
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 2);
        // Conflicts with first reader (idx 0)
        assert_eq!(err.existing_idx, 0);
    }

    #[test]
    fn two_res_same_resource_ok() {
        let params = [res(1, "Time", false), res(1, "Time", false)];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn res_and_resmut_same_resource_conflicts() {
        let params = [res(1, "Time", false), res(1, "Time", true)];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert_eq!(err.existing_idx, 0);
        assert!(err.mutable);
    }

    #[test]
    fn resmut_and_res_same_resource_conflicts() {
        let params = [res(1, "Time", true), res(1, "Time", false)];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert_eq!(err.existing_idx, 0);
    }

    #[test]
    fn two_resmut_same_resource_conflicts() {
        let params = [res(1, "Time", true), res(1, "Time", true)];
        assert!(validate_access(&params).is_err());
    }

    #[test]
    fn different_resources_ok() {
        let params = [res(1, "Time", true), res(2, "Score", true)];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn two_res_assets_same_type_ok() {
        let params = [assets("Mesh", false), assets("Mesh", false)];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn res_and_resmut_assets_same_type_conflicts() {
        let params = [assets("Mesh", false), assets("Mesh", true)];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert!(err.comp_name.contains("Mesh"));
    }

    #[test]
    fn different_asset_types_ok() {
        let params = [assets("Mesh", true), assets("StandardMaterial", true)];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn world_alone_ok() {
        let params = [ParamAccess::<String>::World];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn world_with_none_ok() {
        let params = [ParamAccess::<String>::World, ParamAccess::None];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn world_after_query_conflicts() {
        let params = [
            query(vec![comp("Transform", false)], filters(&["Transform"], &[])),
            ParamAccess::World,
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert_eq!(err.comp_name, "World");
    }

    #[test]
    fn query_after_world_conflicts() {
        let params = [
            ParamAccess::World,
            query(vec![comp("Transform", false)], filters(&["Transform"], &[])),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert_eq!(err.existing_name, "World");
    }

    #[test]
    fn world_after_res_conflicts() {
        let params = [res(1, "Time", false), ParamAccess::World];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.comp_name, "World");
    }

    #[test]
    fn res_after_world_conflicts() {
        let params = [ParamAccess::World, res(1, "Time", false)];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.existing_name, "World");
    }

    #[test]
    fn world_after_assets_conflicts() {
        let params = [assets("Mesh", false), ParamAccess::World];
        assert!(validate_access(&params).is_err());
    }

    #[test]
    fn assets_after_world_conflicts() {
        let params = [ParamAccess::World, assets("Mesh", false)];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.existing_name, "World");
    }

    #[test]
    fn two_worlds_conflict() {
        let params = [ParamAccess::<String>::World, ParamAccess::World];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 1);
        assert_eq!(err.existing_idx, 0);
        assert_eq!(err.comp_name, "World");
        assert_eq!(err.existing_name, "World");
    }

    #[test]
    fn query_plus_res_plus_commands_ok() {
        let params = [
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
            res(1, "Time", false),
            ParamAccess::None, // Commands
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn query_plus_different_assets_ok() {
        let params = [
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
            assets("Mesh", true),
            assets("StandardMaterial", false),
            res(1, "Time", false),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn complex_disjoint_system_ok() {
        // Simulates: fn sys(
        //   q1: Query[Mut[Transform], With[Player]],
        //   q2: Query[Mut[Transform], Without[Player]],
        //   time: Res[Time],
        //   commands: Commands,
        // )
        let params = [
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Player"], &[]),
            ),
            query(
                vec![comp("Transform", true)],
                filters(&["Transform"], &["Player"]),
            ),
            res(1, "Time", false),
            ParamAccess::None,
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn conflict_reports_correct_indices() {
        // params: [None, Res(Time, ro), None, ResMut(Time)]
        let params = [
            ParamAccess::<String>::None,
            res(1, "Time", false),
            ParamAccess::None,
            res(1, "Time", true),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.existing_idx, 1);
        assert_eq!(err.param_idx, 3);
    }

    #[test]
    fn n_way_disjoint_three_queries_ok() {
        // Three mutually-disjoint queries on the same component:
        //   Query[Mut[Transform], With[Player], Without[Enemy]]
        //   Query[Mut[Transform], With[Enemy], Without[Player]]
        //   Query[Mut[Transform], Without[Player], Without[Enemy]]
        let params = [
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Player"], &["Enemy"]),
            ),
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Enemy"], &["Player"]),
            ),
            query(
                vec![comp("Transform", true)],
                filters(&["Transform"], &["Player", "Enemy"]),
            ),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn n_way_third_query_conflicts_with_first() {
        // Two disjoint + third overlaps with first:
        //   Query[Mut[Transform], With[Player]]
        //   Query[Mut[Transform], Without[Player]]  — disjoint with first
        //   Query[Mut[Transform], With[Player]]      — conflicts with first
        let params = [
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Player"], &[]),
            ),
            query(
                vec![comp("Transform", true)],
                filters(&["Transform"], &["Player"]),
            ),
            query(
                vec![comp("Transform", true)],
                filters(&["Transform", "Player"], &[]),
            ),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(err.param_idx, 2);
        assert_eq!(err.existing_idx, 0);
    }
}
