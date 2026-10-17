use std::collections::{HashMap, HashSet};

/// Filter information for disjointness checking.
///
/// Two queries are disjoint if one requires a component (With) that the other
/// excludes (Without). This matches Bevy's `FilteredAccess::is_ruled_out_by()` logic.
#[derive(Debug, Clone)]
pub struct QueryFilters<K> {
    /// Component types that must be present (With\[...\])
    pub with: HashSet<K>,
    /// Component types that must be absent (Without\[...\])
    pub without: HashSet<K>,
}

impl<K> Default for QueryFilters<K> {
    fn default() -> Self {
        Self {
            with: HashSet::new(),
            without: HashSet::new(),
        }
    }
}

impl<K: std::hash::Hash + Eq> QueryFilters<K> {
    /// Check if two filter sets prove that queries are disjoint.
    ///
    /// Two queries are disjoint if:
    /// - Query A has With\[X\] AND Query B has Without\[X\], OR
    /// - Query A has Without\[X\] AND Query B has With\[X\]
    pub fn is_disjoint_from(&self, other: &QueryFilters<K>) -> bool {
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
    /// Python-object reads can require exclusive access.
    pub exclusive: bool,
    /// What the caller wrote.
    pub mutable: bool,
}

/// What access a single system parameter requires.
///
/// Callers convert their parameter types into this enum so the validation
/// algorithm can check for conflicts without knowing the concrete types.
pub enum ParamAccess<K> {
    /// A `ParamSet`: its members may conflict with each other because only
    /// one is borrowed at a time, but each still conflicts with parameters
    /// outside the set.
    Set(Vec<ParamAccess<K>>),
    /// Query or View with component accesses and filters for disjointness checking
    Components {
        accesses: Vec<ComponentAccess<K>>,
        filters: QueryFilters<K>,
        /// Named entries of `filters.with`, in signature order, for remedies.
        markers: Vec<QueryMarker<K>>,
    },
    /// Resource access (Res/ResMut)
    Resource {
        key: K,
        marker: K,
        name: String,
        /// See [`ComponentAccess::exclusive`].
        exclusive: bool,
        /// What the caller wrote.
        mutable: bool,
    },
    /// Assets access (Res\<Assets\<T\>\> / ResMut\<Assets\<T\>\>)
    Assets {
        key: K,
        marker: K,
        name: String,
        /// Display name of the physical asset type behind `key`.
        backing_name: String,
        mutable: bool,
    },
    /// Buffered message channel access. Readers are shared; writers are mutable.
    Message {
        key: String,
        name: String,
        mutable: bool,
    },
    /// Exclusive world access
    World,
    /// No conflict (Commands, Local, observer trigger, etc.)
    None,
}

/// A component a query requires, named for conflict remedies.
#[derive(Debug, Clone)]
pub struct QueryMarker<K> {
    pub key: K,
    pub name: String,
    /// Written as an explicit `With[...]` filter rather than implied by data.
    pub explicit_with: bool,
}

/// A marker only one of two conflicting queries requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerHint {
    pub name: String,
    pub explicit_with: bool,
}

/// Markers that tell two conflicting queries apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryHints {
    /// Required by the earlier parameter only.
    pub existing_only: Option<MarkerHint>,
    /// Required by the later parameter only.
    pub current_only: Option<MarkerHint>,
}

/// Selects the remedy, so a conflict with no queries does not get the query one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    Queries(Box<QueryHints>),
    Resources,
    ResourceQuery,
    Assets,
    AssetsSharedView { backing_name: String },
    Messages,
    WorldExclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictCategory {
    Component,
    Asset,
    Message,
}

/// Conflict information for component access validation errors.
pub struct ComponentAccessConflict {
    pub param_idx: usize,
    pub mutable: bool,
    pub comp_name: String,
    pub existing_idx: usize,
    pub existing_mut: bool,
    pub existing_name: String,
    pub kind: ConflictKind,
    pub category: ConflictCategory,
    /// A read-only parameter that still declares exclusive access.
    pub exclusive_read: bool,
}

/// Validate that a set of system parameter accesses don't conflict.
///
/// Checks for:
/// - Multiple mutable accesses to the same component (unless queries are disjoint via filters)
/// - Mixed mutable/immutable access to the same component (unless disjoint)
/// - Multiple mutable accesses to the same resource
/// - A message writer conflicting with another reader or writer for its channel
/// - World parameter conflicting with any other parameter
///
type ComponentAccessList<K> = Vec<Recorded<K>>;

/// The `accesses` slice is indexed by parameter position; the index is used
/// in conflict error messages.
pub fn validate_access<K: std::hash::Hash + Eq + Clone>(
    accesses: &[ParamAccess<K>],
) -> Result<(), ComponentAccessConflict> {
    // Track component access across all parameters.
    // Uses Vec to support N-way disjoint checking: each new query is checked
    // against ALL previous queries for that component, not just the first.
    let mut component_access: HashMap<K, ComponentAccessList<K>> = HashMap::new();

    // Track assets access (for Res<Assets<T>> / ResMut<Assets<T>> conflicts)
    let mut assets_access: HashMap<K, (Slot, bool, String)> = HashMap::new();

    // Track buffered message channels (reader=shared, writer=mutable).
    let mut message_access: HashMap<String, (Slot, bool, String)> = HashMap::new();

    // Track if World parameter exists (World is exclusive with everything)
    let mut world_param_idx: Option<usize> = None;

    let mut flattened = Vec::with_capacity(accesses.len());
    for (param_idx, param) in accesses.iter().enumerate() {
        match param {
            ParamAccess::Set(members) => {
                for (member, access) in members.iter().enumerate() {
                    flatten_set(
                        Slot {
                            param: param_idx,
                            member: Some(member),
                        },
                        access,
                        &mut flattened,
                    );
                }
            }
            _ => flattened.push((
                Slot {
                    param: param_idx,
                    member: None,
                },
                param,
            )),
        }
    }

    for (position, &(slot, param)) in flattened.iter().enumerate() {
        let param_idx = slot.param;
        match param {
            ParamAccess::Set(_) => unreachable!("parameter sets are flattened above"),
            ParamAccess::Components {
                accesses: comp_accesses,
                filters: current_filters,
                markers: current_markers,
            } => {
                for comp in comp_accesses {
                    // Check against ALL previous accesses to this component
                    if let Some(existing_accesses) = component_access.get(&comp.key) {
                        for existing in existing_accesses {
                            if !slot.alternates_with(existing.slot)
                                && (comp.exclusive || existing.exclusive)
                                && !current_filters.is_disjoint_from(&existing.filters)
                            {
                                return Err(ComponentAccessConflict {
                                    param_idx,
                                    mutable: comp.mutable,
                                    comp_name: comp.name.clone(),
                                    existing_idx: existing.slot.param,
                                    existing_mut: existing.mutable,
                                    existing_name: existing.name.clone(),
                                    kind: if existing.is_query {
                                        ConflictKind::Queries(Box::new(QueryHints {
                                            existing_only: distinguishing_marker(
                                                &existing.markers,
                                                current_filters,
                                            ),
                                            current_only: distinguishing_marker(
                                                current_markers,
                                                &existing.filters,
                                            ),
                                        }))
                                    } else {
                                        ConflictKind::ResourceQuery
                                    },
                                    category: ConflictCategory::Component,
                                    exclusive_read: (comp.exclusive && !comp.mutable)
                                        || (existing.exclusive && !existing.mutable),
                                });
                            }
                        }
                    }

                    // Always record this access for future checks
                    component_access
                        .entry(comp.key.clone())
                        .or_default()
                        .push(Recorded {
                            slot,
                            exclusive: comp.exclusive,
                            mutable: comp.mutable,
                            name: comp.name.clone(),
                            filters: current_filters.clone(),
                            markers: current_markers.clone(),
                            is_query: true,
                        });
                }

                // Check for World conflict
                if let Some(world_idx) = world_param_idx {
                    let (name, mutable) = query_description(comp_accesses);
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable,
                        comp_name: name,
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                        kind: ConflictKind::WorldExclusive,
                        category: ConflictCategory::Component,
                        exclusive_read: false,
                    });
                }
            }

            ParamAccess::Resource {
                key,
                marker,
                name,
                exclusive,
                mutable,
            } => {
                let resource_filters = QueryFilters {
                    with: HashSet::from([marker.clone()]),
                    without: HashSet::new(),
                };
                if let Some(existing_accesses) = component_access.get(key) {
                    for existing in existing_accesses {
                        if !slot.alternates_with(existing.slot)
                            && (*exclusive || existing.exclusive)
                            && !resource_filters.is_disjoint_from(&existing.filters)
                        {
                            return Err(ComponentAccessConflict {
                                param_idx,
                                mutable: *mutable,
                                comp_name: name.clone(),
                                existing_idx: existing.slot.param,
                                existing_mut: existing.mutable,
                                existing_name: existing.name.clone(),
                                kind: if existing.is_query {
                                    ConflictKind::ResourceQuery
                                } else {
                                    ConflictKind::Resources
                                },
                                category: ConflictCategory::Component,
                                exclusive_read: (*exclusive && !*mutable)
                                    || (existing.exclusive && !existing.mutable),
                            });
                        }
                    }
                }
                component_access
                    .entry(key.clone())
                    .or_default()
                    .push(Recorded {
                        slot,
                        exclusive: *exclusive,
                        mutable: *mutable,
                        name: name.clone(),
                        filters: resource_filters,
                        markers: Vec::new(),
                        is_query: false,
                    });

                // Check for World conflict
                if let Some(world_idx) = world_param_idx {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: *mutable,
                        comp_name: name.clone(),
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                        kind: ConflictKind::WorldExclusive,
                        category: ConflictCategory::Component,
                        exclusive_read: false,
                    });
                }
            }

            ParamAccess::Assets {
                key,
                marker,
                name,
                backing_name,
                mutable,
            } => {
                if let Some((existing_slot, existing_mut, existing_name)) = assets_access.get(key) {
                    if slot.alternates_with(*existing_slot) {
                        if *mutable && !*existing_mut {
                            assets_access.insert(key.clone(), (slot, true, name.clone()));
                        }
                    } else if *mutable || *existing_mut {
                        return Err(ComponentAccessConflict {
                            param_idx,
                            mutable: *mutable,
                            comp_name: format!("Assets<{}>", name),
                            existing_idx: existing_slot.param,
                            existing_mut: *existing_mut,
                            existing_name: format!("Assets<{}>", existing_name),
                            kind: if name == existing_name {
                                ConflictKind::Assets
                            } else {
                                ConflictKind::AssetsSharedView {
                                    backing_name: backing_name.clone(),
                                }
                            },
                            category: ConflictCategory::Asset,
                            exclusive_read: false,
                        });
                    }
                } else {
                    assets_access.insert(key.clone(), (slot, *mutable, name.clone()));
                }

                let resource_filters = QueryFilters {
                    with: HashSet::from([marker.clone()]),
                    without: HashSet::new(),
                };
                if let Some(existing_accesses) = component_access.get(key) {
                    for existing in existing_accesses {
                        if !slot.alternates_with(existing.slot)
                            && (*mutable || existing.exclusive)
                            && !resource_filters.is_disjoint_from(&existing.filters)
                        {
                            return Err(ComponentAccessConflict {
                                param_idx,
                                mutable: *mutable,
                                comp_name: format!("Assets<{name}>"),
                                existing_idx: existing.slot.param,
                                existing_mut: existing.mutable,
                                existing_name: existing.name.clone(),
                                kind: ConflictKind::ResourceQuery,
                                category: ConflictCategory::Component,
                                exclusive_read: existing.exclusive && !existing.mutable,
                            });
                        }
                    }
                }
                component_access
                    .entry(key.clone())
                    .or_default()
                    .push(Recorded {
                        slot,
                        exclusive: *mutable,
                        mutable: *mutable,
                        name: format!("Assets<{name}>"),
                        filters: resource_filters,
                        markers: Vec::new(),
                        is_query: false,
                    });

                // Check for World conflict
                if let Some(world_idx) = world_param_idx {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: *mutable,
                        comp_name: format!("Assets<{}>", name),
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                        kind: ConflictKind::WorldExclusive,
                        category: ConflictCategory::Asset,
                        exclusive_read: false,
                    });
                }
            }

            ParamAccess::Message { key, name, mutable } => {
                if let Some((existing_slot, existing_mut, existing_name)) = message_access.get(key)
                {
                    if slot.alternates_with(*existing_slot) {
                        if *mutable && !*existing_mut {
                            message_access.insert(key.clone(), (slot, true, name.clone()));
                        }
                    } else if *mutable || *existing_mut {
                        return Err(ComponentAccessConflict {
                            param_idx,
                            mutable: *mutable,
                            comp_name: name.clone(),
                            existing_idx: existing_slot.param,
                            existing_mut: *existing_mut,
                            existing_name: existing_name.clone(),
                            kind: ConflictKind::Messages,
                            category: ConflictCategory::Message,
                            exclusive_read: false,
                        });
                    }
                } else {
                    message_access.insert(key.clone(), (slot, *mutable, name.clone()));
                }

                if let Some(world_idx) = world_param_idx {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: *mutable,
                        comp_name: name.clone(),
                        existing_idx: world_idx,
                        existing_mut: true,
                        existing_name: "World".to_string(),
                        kind: ConflictKind::WorldExclusive,
                        category: ConflictCategory::Message,
                        exclusive_read: false,
                    });
                }
            }

            ParamAccess::World => {
                if let Some((existing_idx, (category, existing_name, existing_mut))) = flattened
                    [..position]
                    .iter()
                    .filter(|(existing, _)| !slot.alternates_with(*existing))
                    .find_map(|&(existing, param)| {
                        describe_access(param).map(|access| (existing.param, access))
                    })
                {
                    return Err(ComponentAccessConflict {
                        param_idx,
                        mutable: true,
                        comp_name: "World".to_string(),
                        existing_idx,
                        existing_mut,
                        existing_name,
                        kind: ConflictKind::WorldExclusive,
                        category,
                        exclusive_read: false,
                    });
                }
                world_param_idx = Some(param_idx);
            }

            ParamAccess::None => {}
        }
    }

    Ok(())
}

/// Where one flattened access came from: its parameter, and for a `ParamSet`
/// member, the member's position in the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    param: usize,
    member: Option<usize>,
}

impl Slot {
    /// Different members of one `ParamSet` are borrowed one at a time, so
    /// they may conflict; every other pair is checked.
    fn alternates_with(self, other: Slot) -> bool {
        self.param == other.param
            && matches!((self.member, other.member), (Some(a), Some(b)) if a != b)
    }
}

/// Expand a `ParamSet` member, keeping its slot; a nested set stays one member.
fn flatten_set<'a, K>(
    slot: Slot,
    param: &'a ParamAccess<K>,
    out: &mut Vec<(Slot, &'a ParamAccess<K>)>,
) {
    match param {
        ParamAccess::Set(members) => {
            for member in members {
                flatten_set(slot, member, out);
            }
        }
        _ => out.push((slot, param)),
    }
}

fn query_description<K>(accesses: &[ComponentAccess<K>]) -> (String, bool) {
    let mutable = accesses.iter().any(|access| access.mutable);
    let name = match accesses {
        [one] => one.name.clone(),
        [] => "Query".to_string(),
        _ => format!(
            "tuple[{}]",
            accesses
                .iter()
                .map(|access| {
                    if access.mutable {
                        format!("Mut[{}]", access.name)
                    } else {
                        access.name.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    (name, mutable)
}

fn describe_access<K>(param: &ParamAccess<K>) -> Option<(ConflictCategory, String, bool)> {
    match param {
        ParamAccess::Components { accesses, .. } => {
            let (name, mutable) = query_description(accesses);
            Some((ConflictCategory::Component, name, mutable))
        }
        ParamAccess::Resource { name, mutable, .. } => {
            Some((ConflictCategory::Component, name.clone(), *mutable))
        }
        ParamAccess::Assets { name, mutable, .. } => {
            Some((ConflictCategory::Asset, format!("Assets<{name}>"), *mutable))
        }
        ParamAccess::Message { name, mutable, .. } => {
            Some((ConflictCategory::Message, name.clone(), *mutable))
        }
        ParamAccess::World => Some((ConflictCategory::Component, "World".to_string(), true)),
        ParamAccess::Set(_) | ParamAccess::None => None,
    }
}

/// One recorded access, so a later conflict can report what it asked for.
struct Recorded<K> {
    slot: Slot,
    exclusive: bool,
    mutable: bool,
    name: String,
    filters: QueryFilters<K>,
    markers: Vec<QueryMarker<K>>,
    is_query: bool,
}

/// First marker of `own` that `other` does not require, preferring explicit
/// `With[...]` filters over required data components.
fn distinguishing_marker<K: std::hash::Hash + Eq>(
    own: &[QueryMarker<K>],
    other: &QueryFilters<K>,
) -> Option<MarkerHint> {
    let explicit = own.iter().filter(|marker| marker.explicit_with);
    let implied = own.iter().filter(|marker| !marker.explicit_with);
    explicit
        .chain(implied)
        .find(|marker| !other.with.contains(&marker.key))
        .map(|marker| MarkerHint {
            name: marker.name.clone(),
            explicit_with: marker.explicit_with,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters(with: &[&str], without: &[&str]) -> QueryFilters<String> {
        QueryFilters {
            with: with.iter().map(|s| s.to_string()).collect(),
            without: without.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn comp(key: &str, mutable: bool) -> ComponentAccess<String> {
        ComponentAccess {
            key: key.to_string(),
            name: key.to_string(),
            exclusive: mutable,
            mutable,
        }
    }

    fn query(
        components: Vec<ComponentAccess<String>>,
        f: QueryFilters<String>,
    ) -> ParamAccess<String> {
        let mut with = f.with.iter().cloned().collect::<Vec<_>>();
        with.sort();
        let markers = with
            .into_iter()
            .map(|key| QueryMarker {
                explicit_with: !components.iter().any(|c| c.key == key),
                name: key.clone(),
                key,
            })
            .collect();
        ParamAccess::Components {
            accesses: components,
            filters: f,
            markers,
        }
    }

    fn res(key: usize, name: &str, mutable: bool) -> ParamAccess<String> {
        ParamAccess::Resource {
            key: key.to_string(),
            marker: "resource marker".to_string(),
            name: name.to_string(),
            exclusive: mutable,
            mutable,
        }
    }

    fn assets(key: &str, mutable: bool) -> ParamAccess<String> {
        logical_assets(key, key, mutable)
    }

    fn logical_assets(key: &str, name: &str, mutable: bool) -> ParamAccess<String> {
        ParamAccess::Assets {
            key: key.to_string(),
            marker: "resource marker".to_string(),
            name: name.to_string(),
            backing_name: key.to_string(),
            mutable,
        }
    }

    fn hint(name: &str, explicit_with: bool) -> Option<MarkerHint> {
        Some(MarkerHint {
            name: name.to_string(),
            explicit_with,
        })
    }

    #[test]
    fn shared_view_conflict_names_backing_type() {
        let params = [
            logical_assets("StorageShaderMaterial", "Glow", true),
            logical_assets("StorageShaderMaterial", "Palette", false),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(
            err.kind,
            ConflictKind::AssetsSharedView {
                backing_name: "StorageShaderMaterial".to_string()
            }
        );
        assert_eq!(err.existing_name, "Assets<Glow>");
        assert_eq!(err.comp_name, "Assets<Palette>");
    }

    #[test]
    fn shared_view_read_only_parameters_are_compatible() {
        let params = [
            logical_assets("ShaderMaterial", "Glow", false),
            logical_assets("ShaderMaterial", "Palette", false),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn query_conflict_prefers_explicit_filters_unique_to_each_query() {
        // Query[Mut[Text], With[Hud]] vs Query[Mut[Text], With[Banner]]
        let params = [
            query(vec![comp("Text", true)], filters(&["Text", "Hud"], &[])),
            query(vec![comp("Text", true)], filters(&["Text", "Banner"], &[])),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(
            err.kind,
            ConflictKind::Queries(Box::new(QueryHints {
                existing_only: hint("Hud", true),
                current_only: hint("Banner", true),
            }))
        );
    }

    #[test]
    fn query_conflict_marks_data_markers_as_implied() {
        // Query[tuple[Mut[Transform], Bat]] vs Query[tuple[Mut[Transform], BatWing]]
        let params = [
            query(
                vec![comp("Transform", true), comp("Bat", false)],
                filters(&["Transform", "Bat"], &[]),
            ),
            query(
                vec![comp("Transform", true), comp("BatWing", false)],
                filters(&["Transform", "BatWing"], &[]),
            ),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(
            err.kind,
            ConflictKind::Queries(Box::new(QueryHints {
                existing_only: hint("Bat", false),
                current_only: hint("BatWing", false),
            }))
        );
    }

    #[test]
    fn query_conflict_without_distinguishing_markers_has_no_hint() {
        let params = [
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
            query(vec![comp("Transform", true)], filters(&["Transform"], &[])),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(
            err.kind,
            ConflictKind::Queries(Box::new(QueryHints {
                existing_only: None,
                current_only: None,
            }))
        );
    }

    #[test]
    fn query_conflict_with_one_sided_marker() {
        // Query[Mut[Text], With[Hud]] vs Query[Mut[Text]]
        let params = [
            query(vec![comp("Text", true)], filters(&["Text", "Hud"], &[])),
            query(vec![comp("Text", true)], filters(&["Text"], &[])),
        ];
        let err = validate_access(&params).unwrap_err();
        assert_eq!(
            err.kind,
            ConflictKind::Queries(Box::new(QueryHints {
                existing_only: hint("Hud", true),
                current_only: None,
            }))
        );
    }

    fn message(key: &str, mutable: bool) -> ParamAccess<String> {
        ParamAccess::Message {
            key: key.to_string(),
            name: format!("Message<{key}>"),
            mutable,
        }
    }

    #[test]
    fn param_set_members_may_conflict_with_each_other() {
        let params = [ParamAccess::Set(vec![
            query(vec![comp("Transform", true)], QueryFilters::default()),
            query(vec![comp("Transform", false)], QueryFilters::default()),
            res(1, "Time", true),
            res(1, "Time", false),
            assets("Image", false),
            assets("Image", true),
            message("Tick", false),
            message("Tick", true),
        ])];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn param_set_member_conflicts_with_a_parameter_outside_the_set() {
        let params = [
            ParamAccess::Set(vec![
                assets("Image", false),
                query(vec![comp("Transform", true)], QueryFilters::default()),
            ]),
            query(vec![comp("Transform", false)], QueryFilters::default()),
        ];
        let error = validate_access(&params).unwrap_err();
        assert_eq!((error.existing_idx, error.param_idx), (0, 1));

        let upgraded = [
            ParamAccess::Set(vec![assets("Image", false), assets("Image", true)]),
            assets("Image", false),
        ];
        let error = validate_access(&upgraded).unwrap_err();
        assert_eq!((error.existing_idx, error.param_idx), (0, 1));
        assert!(error.existing_mut);
    }

    #[test]
    fn separate_param_sets_conflict_with_each_other() {
        let params = [
            ParamAccess::Set(vec![res(1, "Time", true)]),
            ParamAccess::Set(vec![res(1, "Time", false)]),
        ];
        assert!(validate_access(&params).is_err());
    }

    #[test]
    fn one_parameter_conflicting_with_itself_is_rejected() {
        let lone = [query(
            vec![comp("Transform", true), comp("Transform", false)],
            QueryFilters::default(),
        )];
        let error = validate_access(&lone).unwrap_err();
        assert_eq!((error.existing_idx, error.param_idx), (0, 0));
        assert!(error.existing_mut && !error.mutable);

        let member = [ParamAccess::Set(vec![
            res(1, "Time", false),
            query(
                vec![comp("Transform", true), comp("Transform", false)],
                QueryFilters::default(),
            ),
        ])];
        let error = validate_access(&member).unwrap_err();
        assert_eq!((error.existing_idx, error.param_idx), (0, 0));
        assert_eq!(error.comp_name, "Transform");
    }

    #[test]
    fn world_conflicts_with_a_param_set() {
        let params = [
            ParamAccess::Set(vec![message("Tick", false)]),
            ParamAccess::World,
        ];
        let error = validate_access(&params).unwrap_err();
        assert_eq!((error.existing_idx, error.param_idx), (0, 1));
    }

    #[test]
    fn empty_filters_not_disjoint() {
        let a = QueryFilters::<String>::default();
        let b = QueryFilters::<String>::default();
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
    fn two_readers_of_same_message_are_compatible() {
        assert!(validate_access(&[message("Tick", false), message("Tick", false)]).is_ok());
    }

    #[test]
    fn reader_and_writer_of_same_message_conflict() {
        let error = validate_access(&[message("Tick", false), message("Tick", true)]).unwrap_err();
        assert_eq!(error.existing_idx, 0);
        assert_eq!(error.param_idx, 1);
        assert_eq!(error.comp_name, "Message<Tick>");
    }

    #[test]
    fn two_writers_of_same_message_conflict() {
        assert!(validate_access(&[message("Tick", true), message("Tick", true)]).is_err());
    }

    #[test]
    fn unrelated_message_channels_are_compatible() {
        assert!(validate_access(&[message("Tick", true), message("Damage", false)]).is_ok());
    }

    #[test]
    fn message_and_world_conflict_in_either_order() {
        assert!(validate_access(&[message("Tick", false), ParamAccess::World]).is_err());
        assert!(validate_access(&[ParamAccess::World, message("Tick", false)]).is_err());
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
        // Both have With[Player]: not disjoint
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
        // A-B ok, A-C conflict (or B-C conflict: C is param_idx=2)
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
    fn res_conflicts_with_mutable_query_of_same_resource_component() {
        let params = [
            res(1, "Time", false),
            query(vec![comp("1", true)], filters(&["1"], &[])),
        ];
        assert!(validate_access(&params).is_err());
    }

    #[test]
    fn read_query_and_res_of_same_resource_component_are_compatible() {
        let params = [
            res(1, "Time", false),
            query(vec![comp("1", false)], filters(&["1"], &[])),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn res_is_disjoint_from_mutable_query_without_resource_marker() {
        let params = [
            res(1, "Time", false),
            query(vec![comp("1", true)], filters(&["1"], &["resource marker"])),
        ];
        assert!(validate_access(&params).is_ok());
    }

    #[test]
    fn resource_marker_disjointness_uses_identity_not_display_name() {
        let params = [
            res(1, "Time", false),
            query(
                vec![comp("1", true)],
                filters(&["1"], &["user component named IsResource"]),
            ),
        ];
        assert!(validate_access(&params).is_err());
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
        //   Query[Mut[Transform], Without[Player]]  (disjoint with first)
        //   Query[Mut[Transform], With[Player]]      (conflicts with first)
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
