//! Backend-neutral system-parameter IR and the shared access-declaration walk.
//!
//! Each backend parses Python annotations into its own parameter type, then
//! lowers that type into [`ParamSpec`] (the interpreter work stays in the
//! backend; everything after lowering is shared). One implementation then owns:
//!
//! - [`build_declared_access`]: the whole `System::initialize` access walk,
//!   including the infrastructure reads, so the declared access set cannot
//!   drift between backends.
//! - [`to_param_accesses`]: lowering to [`ParamAccess`] for the intra-system
//!   conflict check ([`validate_access`](super::access_validation::validate_access)).
//! - [`conflict_error_message`] and [`condition_rejection_message`]: the
//!   user-facing error texts, identical on both backends.
//! - [`condition_param_rejection`]: the run-condition read-only gate.
//! - [`describe_param_specs`]: a canonical rendering used by the cross-backend
//!   differential tests.
//!
//! Backend key types are opaque here; [`KeyResolver`] turns them into
//! `ComponentId`s against a concrete `World`.

use std::hash::Hash;

use bevy::ecs::{
    component::ComponentId,
    query::{FilteredAccess, FilteredAccessSet},
    world::World,
};

use super::{
    access_sets::{QueryParamAccess, build_resource_access},
    access_validation::{ComponentAccess, ComponentAccessConflict, ParamAccess, QueryFilters},
};

/// The key types a backend uses to name components, resources, assets and
/// messages before they are resolved to `ComponentId`s. Opaque to the shared
/// walk; only the backend's [`KeyResolver`] interprets them.
pub trait BackendKeys {
    type ComponentKey;
    type ResourceKey;
    type AssetKey;
    type MessageKey;
}

/// One data component of a Query/View parameter.
pub struct ComponentSpec<K: BackendKeys> {
    pub key: K::ComponentKey,
    /// Display name for conflict messages and for the differential rendering.
    pub name: String,
    /// Label used in intra-system disjointness filters. Must come from the
    /// same namespace as the filter labels below; the backend chooses it
    /// (for example, class-name or type-identity strings).
    pub label: String,
    pub mutable: bool,
    pub optional: bool,
}

/// One filter entry (With/Without/Changed/Added) of a Query/View parameter.
pub struct FilterSpec<K: BackendKeys> {
    pub key: K::ComponentKey,
    /// Disjointness label, same namespace as [`ComponentSpec::label`].
    pub label: String,
}

/// Access-relevant shape of a Query or View parameter.
pub struct QuerySpec<K: BackendKeys> {
    pub components: Vec<ComponentSpec<K>>,
    pub with: Vec<FilterSpec<K>>,
    pub without: Vec<FilterSpec<K>>,
    pub changed: Vec<FilterSpec<K>>,
    pub added: Vec<FilterSpec<K>>,
    /// Has/AnyOf components: resolved so the runtime query builder finds their
    /// ids registered, but they contribute no access and no filter. `Has`
    /// matches regardless of the component's presence; `AnyOf` has OR
    /// semantics that a conjunctive `With` cannot express. Omitting both is
    /// conservative: it only costs parallelism, never soundness.
    pub resolve_only: Vec<K::ComponentKey>,
}

impl<K: BackendKeys> Default for QuerySpec<K> {
    fn default() -> Self {
        Self {
            components: Vec::new(),
            with: Vec::new(),
            without: Vec::new(),
            changed: Vec::new(),
            added: Vec::new(),
            resolve_only: Vec::new(),
        }
    }
}

/// Backend-neutral shape of one system parameter.
pub enum ParamSpec<K: BackendKeys> {
    Query(QuerySpec<K>),
    View(QuerySpec<K>),
    Res {
        key: K::ResourceKey,
        /// Conflict-validation key. `None` opts the parameter out of the
        /// intra-system conflict check (used for infrastructure resources
        /// that are only ever read by an adapter-specific runtime path).
        vkey: Option<usize>,
        /// Display name for conflict messages.
        name: String,
        mutable: bool,
    },
    Assets {
        key: K::AssetKey,
        /// Conflict-validation key; distinct asset collections must produce
        /// distinct strings.
        vkey: String,
        /// Display name for conflict messages.
        name: String,
        mutable: bool,
    },
    MessageReader {
        key: K::MessageKey,
    },
    MessageWriter {
        key: K::MessageKey,
    },
    MessageMutator {
        key: K::MessageKey,
    },
    World,
    Commands,
    Local,
    Observer,
}

/// Resolved ids for a `Res`/`ResMut` parameter.
#[derive(Default, Clone)]
pub struct ResolvedResource {
    /// The resource's own id; routed to reads or writes by the parameter's
    /// mutability. `None` declares nothing (best-effort resolution failed).
    pub primary: Option<ComponentId>,
    /// Additional resources the backend's runtime path reads while accessing
    /// this parameter (e.g. registry/storage indirections). Always reads:
    /// mutation, when any, happens behind those resources' own locks.
    pub aux_reads: Vec<ComponentId>,
}

/// Resolved ids for a message parameter. A backend may map one message key to
/// several ids (typed buffer plus auxiliary input resources) or collapse all
/// messages into one queue resource; the shared walk only merges the lists.
#[derive(Default, Clone)]
pub struct ResolvedMessage {
    pub reads: Vec<ComponentId>,
    pub writes: Vec<ComponentId>,
}

/// Resolves backend keys to `ComponentId`s during the access walk.
///
/// Implementations must REGISTER ids (not just look them up) wherever the
/// backend can, so access is declared even when the value is inserted after
/// schedule initialization; an undeclared access would let conflicting systems
/// race. Best-effort lookups that can fail return `None`/empty and declare
/// nothing.
pub trait KeyResolver<K: BackendKeys> {
    fn component_id(&mut self, world: &mut World, key: &K::ComponentKey) -> Option<ComponentId>;
    fn resource_ids(&mut self, world: &mut World, key: &K::ResourceKey) -> ResolvedResource;
    fn asset_id(&mut self, world: &mut World, key: &K::AssetKey) -> Option<ComponentId>;
    fn message_ids(
        &mut self,
        world: &mut World,
        key: &K::MessageKey,
        write: bool,
    ) -> ResolvedMessage;
    /// Resources the backend's run scaffold touches on EVERY run regardless of
    /// the parameter list (generation guard, profiling epilogue, error sinks).
    /// The walk appends these to the declared reads unconditionally so a
    /// backend cannot forget them; the differential tests pin the common set.
    fn infrastructure_reads(&mut self, world: &mut World) -> Vec<ComponentId>;
}

/// Everything `System::initialize` needs from the access walk.
pub struct DeclaredAccess {
    /// The access set to return from `initialize`. Empty when `needs_world`.
    pub set: FilteredAccessSet,
    /// True when a `World` parameter is present (exclusive system).
    pub needs_world: bool,
    pub resources_to_read: Vec<ComponentId>,
    pub resources_to_write: Vec<ComponentId>,
}

fn push_unique(ids: &mut Vec<ComponentId>, id: ComponentId) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}

/// The shared `System::initialize` access walk.
///
/// One `FilteredAccess` per Query/View parameter, so a filter on one query
/// never narrows another parameter's declared access (that would fabricate
/// disjointness and let the scheduler parallelize conflicting systems).
/// Resource-like access is gathered into a single shared `FilteredAccess`
/// appended at the end.
///
/// Exclusive systems (any `World` parameter) declare NO access, exactly like
/// Bevy's `ExclusiveFunctionSystem` (its initialize returns
/// `FilteredAccessSet::new()`). The NON_SEND | EXCLUSIVE flags alone make the
/// executor run them with nothing else in flight, which is what soundly covers
/// `run_unsafe`'s `world.world_mut()`. Declaring read_all+write_all here
/// instead gives the schedule an asymmetric conflict graph next to Bevy's
/// empty-access exclusive systems and wedges MultiThreadedExecutor's
/// ready-queue accounting: three or more exclusive systems mixing both shapes
/// in one schedule die on its `ready_systems.is_clear()` assertion. The walk
/// still runs first, so ids and caches are registered for run time.
pub fn build_declared_access<K: BackendKeys>(
    world: &mut World,
    params: &[ParamSpec<K>],
    resolver: &mut impl KeyResolver<K>,
) -> DeclaredAccess {
    let mut param_accesses: Vec<FilteredAccess> = Vec::new();
    let mut resources_to_read: Vec<ComponentId> = Vec::new();
    let mut resources_to_write: Vec<ComponentId> = Vec::new();
    let mut needs_world = false;

    for param in params {
        match param {
            ParamSpec::Query(spec) | ParamSpec::View(spec) => {
                let mut access = QueryParamAccess::default();

                for comp in &spec.components {
                    let Some(id) = resolver.component_id(world, &comp.key) else {
                        continue;
                    };
                    // A required component gates the access, so it also
                    // contributes a `With` (added by `build`). An optional
                    // component may be absent while the query still matches,
                    // so it contributes access without a `With`.
                    match (comp.mutable, comp.optional) {
                        (true, false) => access.writes.push(id),
                        (false, false) => access.reads.push(id),
                        (true, true) => access.optional_writes.push(id),
                        (false, true) => access.optional_reads.push(id),
                    }
                }
                for f in &spec.with {
                    if let Some(id) = resolver.component_id(world, &f.key) {
                        access.with.push(id);
                    }
                }
                for f in &spec.without {
                    if let Some(id) = resolver.component_id(world, &f.key) {
                        access.without.push(id);
                    }
                }
                // Changed/Added archetypally imply the component is present,
                // and the tick check reads its change ticks, so declare reads.
                for f in &spec.changed {
                    if let Some(id) = resolver.component_id(world, &f.key) {
                        access.reads.push(id);
                    }
                }
                for f in &spec.added {
                    if let Some(id) = resolver.component_id(world, &f.key) {
                        access.reads.push(id);
                    }
                }
                // Has/AnyOf: no access, no filter (see `QuerySpec::resolve_only`).
                for key in &spec.resolve_only {
                    let _ = resolver.component_id(world, key);
                }

                param_accesses.push(access.build());
            }
            ParamSpec::Res { key, mutable, .. } => {
                let resolved = resolver.resource_ids(world, key);
                if let Some(id) = resolved.primary {
                    if *mutable {
                        push_unique(&mut resources_to_write, id);
                    } else {
                        push_unique(&mut resources_to_read, id);
                    }
                }
                for id in resolved.aux_reads {
                    push_unique(&mut resources_to_read, id);
                }
            }
            ParamSpec::Assets { key, mutable, .. } => {
                if let Some(id) = resolver.asset_id(world, key) {
                    if *mutable {
                        push_unique(&mut resources_to_write, id);
                    } else {
                        push_unique(&mut resources_to_read, id);
                    }
                }
            }
            ParamSpec::MessageReader { key }
            | ParamSpec::MessageWriter { key }
            | ParamSpec::MessageMutator { key } => {
                let write = matches!(
                    param,
                    ParamSpec::MessageWriter { .. } | ParamSpec::MessageMutator { .. }
                );
                let resolved = resolver.message_ids(world, key, write);
                for id in resolved.writes {
                    push_unique(&mut resources_to_write, id);
                }
                for id in resolved.reads {
                    push_unique(&mut resources_to_read, id);
                }
            }
            ParamSpec::World => {
                needs_world = true;
            }
            // A `Commands` param defers every structural mutation into its
            // queue (applied at apply_deferred); entity reservation goes
            // through the atomic entity allocator. Local and Observer carry no
            // world access. All three declare nothing.
            ParamSpec::Commands | ParamSpec::Local | ParamSpec::Observer => {}
        }
    }

    for id in resolver.infrastructure_reads(world) {
        push_unique(&mut resources_to_read, id);
    }

    let set = if needs_world {
        FilteredAccessSet::default()
    } else {
        let mut set = FilteredAccessSet::default();
        for access in param_accesses {
            set.add(access);
        }
        set.add(build_resource_access(
            &resources_to_read,
            &resources_to_write,
        ));
        set
    };

    DeclaredAccess {
        set,
        needs_world,
        resources_to_read,
        resources_to_write,
    }
}

/// Lower specs to [`ParamAccess`] for the intra-system conflict check.
///
/// `component_vkey` maps a component key to the validation key: a stable
/// pre-world key at construction/registration time (name string or type
/// pointer), or the resolved `ComponentId` when a `World` is available.
/// `message_vkey` supplies the corresponding channel identity and display name.
///
/// Disjointness filters include an implicit `With` for every queried
/// component (matching Bevy, where `Query<&T>` implies `With<T>` for access
/// checking), the explicit With/Without filters, and Changed/Added as `With`
/// (both archetypally require the component present).
pub fn to_param_accesses<K: BackendKeys, VK: Hash + Eq + Clone>(
    params: &[ParamSpec<K>],
    mut component_vkey: impl FnMut(&K::ComponentKey) -> VK,
    mut message_vkey: impl FnMut(&K::MessageKey) -> (String, String),
) -> Vec<ParamAccess<VK>> {
    params
        .iter()
        .map(|param| match param {
            ParamSpec::Query(spec) | ParamSpec::View(spec) => {
                let accesses = spec
                    .components
                    .iter()
                    .map(|c| ComponentAccess {
                        key: component_vkey(&c.key),
                        name: c.name.clone(),
                        mutable: c.mutable,
                    })
                    .collect();

                let mut filters = QueryFilters::default();
                for c in &spec.components {
                    filters.with.insert(c.label.clone());
                }
                for f in &spec.with {
                    filters.with.insert(f.label.clone());
                }
                for f in &spec.without {
                    filters.without.insert(f.label.clone());
                }
                for f in spec.changed.iter().chain(&spec.added) {
                    filters.with.insert(f.label.clone());
                }

                ParamAccess::Components { accesses, filters }
            }
            ParamSpec::Res {
                vkey: Some(vkey),
                name,
                mutable,
                ..
            } => ParamAccess::Resource {
                key: *vkey,
                name: name.clone(),
                mutable: *mutable,
            },
            ParamSpec::Res { vkey: None, .. } => ParamAccess::None,
            ParamSpec::Assets {
                vkey,
                name,
                mutable,
                ..
            } => ParamAccess::Assets {
                key: vkey.clone(),
                name: name.clone(),
                mutable: *mutable,
            },
            ParamSpec::MessageReader { key }
            | ParamSpec::MessageWriter { key }
            | ParamSpec::MessageMutator { key } => {
                let (key, name) = message_vkey(key);
                ParamAccess::Message {
                    key,
                    name,
                    mutable: matches!(
                        param,
                        ParamSpec::MessageWriter { .. } | ParamSpec::MessageMutator { .. }
                    ),
                }
            }
            ParamSpec::World => ParamAccess::World,
            ParamSpec::Commands | ParamSpec::Local | ParamSpec::Observer => ParamAccess::None,
        })
        .collect()
}

/// The user-facing conflict message, identical on both backends.
pub fn conflict_error_message(func_name: &str, conflict: &ComponentAccessConflict) -> String {
    let category = if conflict.comp_name.starts_with("Message<")
        || conflict.existing_name.starts_with("Message<")
    {
        "message"
    } else {
        "component"
    };
    format!(
        "System '{}' has conflicting {} access:\n\
         - Parameter {} requests {} access to {}\n\
         - Parameter {} already has {} access to {}\n\
         Rust's borrowing rules forbid multiple mutable references or \
         mixing mutable and immutable references to the same data.",
        func_name,
        category,
        conflict.param_idx,
        if conflict.mutable {
            "mutable"
        } else {
            "immutable"
        },
        conflict.comp_name,
        conflict.existing_idx,
        if conflict.existing_mut {
            "mutable"
        } else {
            "immutable"
        },
        conflict.existing_name
    )
}

/// Describe why a parameter is rejected in a run condition, or `None` if the
/// parameter is read-only and therefore allowed. A run condition may only read
/// the world: it is evaluated under Bevy's read-only system contract and its
/// deferred operations (e.g. `Commands`) are never applied.
pub fn condition_param_rejection<K: BackendKeys>(param: &ParamSpec<K>) -> Option<&'static str> {
    match param {
        ParamSpec::World => Some("World (exclusive world access)"),
        ParamSpec::Commands => Some("Commands (queued mutations are never applied to conditions)"),
        ParamSpec::MessageWriter { .. } => {
            Some("MessageWriter (writing messages mutates the world)")
        }
        ParamSpec::MessageMutator { .. } => {
            Some("MessageMutator (reading, mutating, and writing messages mutates the world)")
        }
        ParamSpec::MessageReader { .. } => Some(
            "MessageReader (advancing the read cursor mutates reader state; \
             read messages in a regular system instead)",
        ),
        ParamSpec::Res { mutable: true, .. } => Some("ResMut (mutable resource access)"),
        ParamSpec::Assets { mutable: true, .. } => Some("mutable Assets (mutable asset access)"),
        ParamSpec::Query(spec) if spec.components.iter().any(|c| c.mutable) => {
            Some("Query with a Mut component (mutable component access)")
        }
        ParamSpec::View(spec) if spec.components.iter().any(|c| c.mutable) => {
            Some("View with a Mut component (mutable component access)")
        }
        _ => None,
    }
}

/// The user-facing run-condition rejection message, identical on both backends.
pub fn condition_rejection_message(
    condition_name: &str,
    param_idx: usize,
    param_name: &str,
    kind: &str,
) -> String {
    format!(
        "Run condition '{condition_name}' parameter {param_idx} ('{param_name}') is {kind}. \
         Run conditions require read-only world access: they are evaluated under \
         Bevy's read-only system contract and any deferred operations are never \
         applied. Use read-only parameters such as Res, read-only Query/View, \
         Local, or read-only Assets."
    )
}

/// Canonical rendering of a lowered parameter list, for the cross-backend
/// differential tests: both backends lower the same Python system signature
/// and must render identically. Component names are Python class names (or
/// bridge names) and are comparable across backends; filter and message keys
/// are adapter-specific, so filters render as
/// counts and resource/asset/message names are omitted.
pub fn describe_param_specs<K: BackendKeys>(params: &[ParamSpec<K>]) -> Vec<String> {
    params
        .iter()
        .map(|param| match param {
            ParamSpec::Query(spec) => describe_query_spec("query", spec),
            ParamSpec::View(spec) => describe_query_spec("view", spec),
            ParamSpec::Res { vkey, mutable, .. } => format!(
                "res mutable={} vkey={}",
                mutable,
                if vkey.is_some() { "present" } else { "none" }
            ),
            ParamSpec::Assets { mutable, .. } => format!("assets mutable={mutable}"),
            ParamSpec::MessageReader { .. } => "message_reader".to_string(),
            ParamSpec::MessageWriter { .. } => "message_writer".to_string(),
            ParamSpec::MessageMutator { .. } => "message_mutator".to_string(),
            ParamSpec::World => "world".to_string(),
            ParamSpec::Commands => "commands".to_string(),
            ParamSpec::Local => "local".to_string(),
            ParamSpec::Observer => "observer".to_string(),
        })
        .collect()
}

/// Golden rendering for the cross-backend lowering corpus.
///
/// Each backend's test suite constructs this exact parameter list with its
/// native descriptor types and Python classes named Position, Velocity,
/// Player, Frozen, Health, Shield, Transform, Visibility and Score:
///
/// 1. Query: Mut\[Position\], Optional\[Velocity\], With\[Player\],
///    Without\[Frozen\], Changed\[Health\], Has\[Shield\]
/// 2. View: Mut\[Transform\], Changed\[Visibility\]
/// 3. Res\[Score\] (immutable)
/// 4. Assets (mutable)
/// 5. MessageReader
/// 6. MessageWriter
/// 7. MessageMutator
/// 8. World
/// 9. Commands
/// 10. Local
/// 11. On (observer)
///
/// The test lowers the list and asserts [`describe_param_specs`] equals this
/// golden. Both branches carry this file byte-identically, so a change that
/// shifts either backend's lowering fails that backend's suite against the
/// same expectation: the drift tripwire for the extraction campaign.
pub const LOWERING_CORPUS_GOLDEN: &[&str] = &[
    "query components=[Position(mut), Velocity(read,opt)] \
     with=1 without=1 changed=1 added=0 resolve_only=1",
    "view components=[Transform(mut)] with=0 without=0 changed=1 added=0 resolve_only=0",
    "res mutable=false vkey=present",
    "assets mutable=true",
    "message_reader",
    "message_writer",
    "message_mutator",
    "world",
    "commands",
    "local",
    "observer",
];

fn describe_query_spec<K: BackendKeys>(kind: &str, spec: &QuerySpec<K>) -> String {
    let comps: Vec<String> = spec
        .components
        .iter()
        .map(|c| {
            format!(
                "{}({}{})",
                c.name,
                if c.mutable { "mut" } else { "read" },
                if c.optional { ",opt" } else { "" }
            )
        })
        .collect();
    format!(
        "{} components=[{}] with={} without={} changed={} added={} resolve_only={}",
        kind,
        comps.join(", "),
        spec.with.len(),
        spec.without.len(),
        spec.changed.len(),
        spec.added.len(),
        spec.resolve_only.len()
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy::ecs::{
        component::Component,
        query::{FilteredAccessSet, Without},
        world::World,
    };

    use super::{super::access_validation::validate_access, *};

    #[derive(Component)]
    struct T;
    #[derive(Component)]
    struct U;
    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;
    #[derive(Component)]
    struct R1;
    #[derive(Component)]
    struct R2;
    #[derive(Component)]
    struct Infra1;
    #[derive(Component)]
    struct Infra2;

    struct MockKeys;
    impl BackendKeys for MockKeys {
        type ComponentKey = &'static str;
        type ResourceKey = &'static str;
        type AssetKey = &'static str;
        type MessageKey = &'static str;
    }

    #[derive(Default)]
    struct MockResolver {
        components: HashMap<&'static str, ComponentId>,
        resources: HashMap<&'static str, ResolvedResource>,
        assets: HashMap<&'static str, ComponentId>,
        messages: HashMap<&'static str, ResolvedMessage>,
        infra: Vec<ComponentId>,
        component_calls: Vec<&'static str>,
    }

    impl KeyResolver<MockKeys> for MockResolver {
        fn component_id(&mut self, _world: &mut World, key: &&'static str) -> Option<ComponentId> {
            self.component_calls.push(key);
            self.components.get(key).copied()
        }

        fn resource_ids(&mut self, _world: &mut World, key: &&'static str) -> ResolvedResource {
            self.resources.get(key).cloned().unwrap_or_default()
        }

        fn asset_id(&mut self, _world: &mut World, key: &&'static str) -> Option<ComponentId> {
            self.assets.get(key).copied()
        }

        fn message_ids(
            &mut self,
            _world: &mut World,
            key: &&'static str,
            _write: bool,
        ) -> ResolvedMessage {
            self.messages.get(key).cloned().unwrap_or_default()
        }

        fn infrastructure_reads(&mut self, _world: &mut World) -> Vec<ComponentId> {
            self.infra.clone()
        }
    }

    /// A world with the test component zoo registered, plus a resolver mapping
    /// string keys to the registered ids.
    fn setup() -> (World, MockResolver) {
        let mut world = World::new();
        let mut resolver = MockResolver::default();
        resolver
            .components
            .insert("T", world.register_component::<T>());
        resolver
            .components
            .insert("U", world.register_component::<U>());
        resolver
            .components
            .insert("A", world.register_component::<A>());
        resolver
            .components
            .insert("B", world.register_component::<B>());
        resolver.infra = vec![
            world.register_component::<Infra1>(),
            world.register_component::<Infra2>(),
        ];
        (world, resolver)
    }

    fn comp(key: &'static str, mutable: bool, optional: bool) -> ComponentSpec<MockKeys> {
        ComponentSpec {
            key,
            name: key.to_string(),
            label: key.to_string(),
            mutable,
            optional,
        }
    }

    fn filter(key: &'static str) -> FilterSpec<MockKeys> {
        FilterSpec {
            key,
            label: key.to_string(),
        }
    }

    fn set_of(access: FilteredAccess) -> FilteredAccessSet {
        let mut set = FilteredAccessSet::default();
        set.add(access);
        set
    }

    #[test]
    fn query_without_conflicts_with_native_writer_in_same_archetypes() {
        let (mut world, mut resolver) = setup();
        let spec = QuerySpec {
            components: vec![comp("T", true, false)],
            without: vec![filter("A")],
            ..Default::default()
        };
        let declared = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::Query(spec)],
            &mut resolver,
        );

        let native = world.query_filtered::<&mut T, Without<A>>();
        let native = set_of(native.component_access().clone());
        assert!(!declared.set.is_compatible(&native));
        assert!(!declared.needs_world);
    }

    #[test]
    fn query_with_filter_stays_disjoint_from_native_without() {
        let (mut world, mut resolver) = setup();
        let spec = QuerySpec {
            components: vec![comp("T", true, false)],
            with: vec![filter("A")],
            ..Default::default()
        };
        let declared = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::Query(spec)],
            &mut resolver,
        );

        let native = world.query_filtered::<&mut T, Without<A>>();
        let native = set_of(native.component_access().clone());
        assert!(declared.set.is_compatible(&native));
    }

    #[test]
    fn changed_filter_declares_read_and_conflicts_with_writer() {
        let (mut world, mut resolver) = setup();
        let b_id = resolver.components["B"];
        let spec = QuerySpec {
            components: vec![comp("A", true, false)],
            changed: vec![filter("B")],
            ..Default::default()
        };
        let declared = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::Query(spec)],
            &mut resolver,
        );
        assert!(declared.set.combined_access().has_read(b_id));

        let native = world.query_filtered::<&mut B, ()>();
        let native = set_of(native.component_access().clone());
        assert!(!declared.set.is_compatible(&native));
    }

    #[test]
    fn optional_component_adds_access_but_no_with() {
        let (mut world, mut resolver) = setup();
        let spec = QuerySpec {
            components: vec![comp("U", true, false), comp("T", false, true)],
            ..Default::default()
        };
        let declared = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::Query(spec)],
            &mut resolver,
        );

        // Would wrongly be compatible if optional T contributed `With<T>`.
        let native = world.query_filtered::<&mut U, Without<T>>();
        let native = set_of(native.component_access().clone());
        assert!(!declared.set.is_compatible(&native));
    }

    #[test]
    fn resolve_only_keys_are_resolved_but_declare_nothing() {
        let (mut world, mut resolver) = setup();
        let a_id = resolver.components["A"];
        let spec = QuerySpec {
            components: vec![comp("T", false, false)],
            resolve_only: vec!["A"],
            ..Default::default()
        };
        let declared = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::Query(spec)],
            &mut resolver,
        );

        assert!(resolver.component_calls.contains(&"A"));
        assert!(!declared.set.combined_access().has_read(a_id));
    }

    #[test]
    fn res_routes_by_mutability_and_dedupes_aux_reads() {
        let (mut world, mut resolver) = setup();
        let r1 = world.register_component::<R1>();
        let r2 = world.register_component::<R2>();
        let aux = world.register_component::<A>();
        resolver.resources.insert(
            "r1",
            ResolvedResource {
                primary: Some(r1),
                aux_reads: vec![aux, aux],
            },
        );
        resolver.resources.insert(
            "r2",
            ResolvedResource {
                primary: Some(r2),
                aux_reads: vec![aux],
            },
        );

        let params = [
            ParamSpec::<MockKeys>::Res {
                key: "r1",
                vkey: Some(1),
                name: "R1".to_string(),
                mutable: false,
            },
            ParamSpec::<MockKeys>::Res {
                key: "r2",
                vkey: Some(2),
                name: "R2".to_string(),
                mutable: true,
            },
        ];
        let declared = build_declared_access(&mut world, &params, &mut resolver);

        assert!(declared.resources_to_read.contains(&r1));
        assert!(declared.resources_to_write.contains(&r2));
        assert_eq!(
            declared
                .resources_to_read
                .iter()
                .filter(|&&id| id == aux)
                .count(),
            1
        );
        assert!(declared.set.combined_access().has_read(r1));
        assert!(declared.set.combined_access().has_write(r2));
    }

    #[test]
    fn assets_route_by_mutability() {
        let (mut world, mut resolver) = setup();
        let mesh = world.register_component::<R1>();
        resolver.assets.insert("Mesh", mesh);

        let params = [ParamSpec::<MockKeys>::Assets {
            key: "Mesh",
            vkey: "Mesh".to_string(),
            name: "Mesh".to_string(),
            mutable: true,
        }];
        let declared = build_declared_access(&mut world, &params, &mut resolver);
        assert!(declared.set.combined_access().has_write(mesh));
    }

    #[test]
    fn message_reader_and_writer_merge_and_dedupe() {
        let (mut world, mut resolver) = setup();
        let queue = world.register_component::<R1>();
        let aux = world.register_component::<R2>();
        resolver.messages.insert(
            "KeyboardInput",
            ResolvedMessage {
                reads: vec![queue, aux],
                writes: vec![],
            },
        );
        resolver.messages.insert(
            "Collision",
            ResolvedMessage {
                reads: vec![],
                writes: vec![queue],
            },
        );

        let params = [
            ParamSpec::<MockKeys>::MessageReader {
                key: "KeyboardInput",
            },
            ParamSpec::<MockKeys>::MessageReader {
                key: "KeyboardInput",
            },
            ParamSpec::<MockKeys>::MessageWriter { key: "Collision" },
        ];
        let declared = build_declared_access(&mut world, &params, &mut resolver);

        assert_eq!(
            declared
                .resources_to_read
                .iter()
                .filter(|&&id| id == queue)
                .count(),
            1
        );
        assert!(declared.resources_to_write.contains(&queue));
        assert!(declared.set.combined_access().has_read(aux));
    }

    #[test]
    fn cross_system_same_message_reader_writer_are_scheduler_incompatible() {
        let mut world = World::new();
        let store = world.register_component::<R1>();
        let channel = world.register_component::<R2>();
        let mut reader_resolver = MockResolver::default();
        reader_resolver.messages.insert(
            "Tick",
            ResolvedMessage {
                reads: vec![store, channel],
                writes: vec![],
            },
        );
        let reader = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::MessageReader { key: "Tick" }],
            &mut reader_resolver,
        );
        let mut writer_resolver = MockResolver::default();
        writer_resolver.messages.insert(
            "Tick",
            ResolvedMessage {
                reads: vec![store],
                writes: vec![channel],
            },
        );
        let writer = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::MessageWriter { key: "Tick" }],
            &mut writer_resolver,
        );

        assert!(!reader.set.is_compatible(&writer.set));
    }

    #[test]
    fn message_mutator_is_exclusive_per_channel() {
        let mut world = World::new();
        let store = world.register_component::<R1>();
        let channel = world.register_component::<R2>();
        let mut mutator_resolver = MockResolver::default();
        mutator_resolver.messages.insert(
            "Tick",
            ResolvedMessage {
                reads: vec![store],
                writes: vec![channel],
            },
        );
        let mutator = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::MessageMutator { key: "Tick" }],
            &mut mutator_resolver,
        );
        let mut reader_resolver = MockResolver::default();
        reader_resolver.messages.insert(
            "Tick",
            ResolvedMessage {
                reads: vec![store, channel],
                writes: vec![],
            },
        );
        let reader = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::MessageReader { key: "Tick" }],
            &mut reader_resolver,
        );

        assert!(mutator.set.combined_access().has_write(channel));
        assert!(!mutator.set.is_compatible(&reader.set));

        let mut second_mutator_resolver = MockResolver::default();
        second_mutator_resolver.messages.insert(
            "Tick",
            ResolvedMessage {
                reads: vec![store],
                writes: vec![channel],
            },
        );
        let second_mutator = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::MessageMutator { key: "Tick" }],
            &mut second_mutator_resolver,
        );
        assert!(!mutator.set.is_compatible(&second_mutator.set));
    }

    #[test]
    fn cross_system_unrelated_message_channels_are_scheduler_compatible() {
        let mut world = World::new();
        let store = world.register_component::<R1>();
        let channel_a = world.register_component::<R2>();
        let channel_b = world.register_component::<A>();
        let mut first_resolver = MockResolver::default();
        first_resolver.messages.insert(
            "A",
            ResolvedMessage {
                reads: vec![store],
                writes: vec![channel_a],
            },
        );
        let first = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::MessageWriter { key: "A" }],
            &mut first_resolver,
        );
        let mut second_resolver = MockResolver::default();
        second_resolver.messages.insert(
            "B",
            ResolvedMessage {
                reads: vec![store],
                writes: vec![channel_b],
            },
        );
        let second = build_declared_access(
            &mut world,
            &[ParamSpec::<MockKeys>::MessageWriter { key: "B" }],
            &mut second_resolver,
        );

        assert!(first.set.is_compatible(&second.set));
    }

    #[test]
    fn world_param_returns_empty_set_but_walk_still_runs() {
        let (mut world, mut resolver) = setup();
        let r1 = world.register_component::<R1>();
        resolver.resources.insert(
            "r1",
            ResolvedResource {
                primary: Some(r1),
                aux_reads: vec![],
            },
        );

        let params = [
            ParamSpec::<MockKeys>::Res {
                key: "r1",
                vkey: Some(1),
                name: "R1".to_string(),
                mutable: false,
            },
            ParamSpec::<MockKeys>::World,
        ];
        let declared = build_declared_access(&mut world, &params, &mut resolver);

        assert!(declared.needs_world);
        assert!(!declared.set.combined_access().has_any_read());
        assert!(!declared.set.combined_access().has_any_write());
        // Registrations still ran: ids are ready for run time.
        assert!(declared.resources_to_read.contains(&r1));
    }

    #[test]
    fn infrastructure_reads_always_declared() {
        let (mut world, mut resolver) = setup();
        let infra = resolver.infra.clone();
        let declared = build_declared_access::<MockKeys>(&mut world, &[], &mut resolver);
        for id in infra {
            assert!(declared.resources_to_read.contains(&id));
            assert!(declared.set.combined_access().has_read(id));
        }
    }

    fn mut_query(key: &'static str, extra: QuerySpec<MockKeys>) -> ParamSpec<MockKeys> {
        ParamSpec::Query(QuerySpec {
            components: vec![comp(key, true, false)],
            ..extra
        })
    }

    #[test]
    fn validation_two_mut_queries_conflict() {
        let params = [
            mut_query("T", QuerySpec::default()),
            mut_query("T", QuerySpec::default()),
        ];
        let accesses = to_param_accesses(&params, |k| *k, |k| ((*k).to_string(), (*k).to_string()));
        assert!(validate_access(&accesses).is_err());
    }

    #[test]
    fn validation_with_without_disjointness_accepted() {
        let params = [
            mut_query(
                "T",
                QuerySpec {
                    with: vec![filter("A")],
                    ..Default::default()
                },
            ),
            mut_query(
                "T",
                QuerySpec {
                    without: vec![filter("A")],
                    ..Default::default()
                },
            ),
        ];
        let accesses = to_param_accesses(&params, |k| *k, |k| ((*k).to_string(), (*k).to_string()));
        assert!(validate_access(&accesses).is_ok());
    }

    #[test]
    fn validation_changed_implies_with_for_disjointness() {
        // Changed[A] archetypally requires A present, so it may prove
        // disjointness against Without[A] exactly like an explicit With[A].
        let params = [
            mut_query(
                "T",
                QuerySpec {
                    changed: vec![filter("A")],
                    ..Default::default()
                },
            ),
            mut_query(
                "T",
                QuerySpec {
                    without: vec![filter("A")],
                    ..Default::default()
                },
            ),
        ];
        let accesses = to_param_accesses(&params, |k| *k, |k| ((*k).to_string(), (*k).to_string()));
        assert!(validate_access(&accesses).is_ok());
    }

    #[test]
    fn validation_res_vkey_none_is_exempt() {
        let params = [
            ParamSpec::<MockKeys>::Res {
                key: "server",
                vkey: None,
                name: "AssetServer".to_string(),
                mutable: false,
            },
            ParamSpec::<MockKeys>::World,
        ];
        let accesses = to_param_accesses(&params, |k| *k, |k| ((*k).to_string(), (*k).to_string()));
        // vkey: None lowers to ParamAccess::None, so World sees no conflict.
        assert!(validate_access(&accesses).is_ok());
    }

    #[test]
    fn validation_world_conflicts_with_query() {
        let params = [
            ParamSpec::<MockKeys>::World,
            mut_query("T", QuerySpec::default()),
        ];
        let accesses = to_param_accesses(&params, |k| *k, |k| ((*k).to_string(), (*k).to_string()));
        let err = validate_access(&accesses).unwrap_err();
        assert_eq!(err.existing_name, "World");
    }

    #[test]
    fn condition_rejects_world_commands_messages_and_mut_access() {
        let rejected: [ParamSpec<MockKeys>; 7] = [
            ParamSpec::World,
            ParamSpec::Commands,
            ParamSpec::MessageWriter { key: "M" },
            ParamSpec::MessageReader { key: "M" },
            ParamSpec::MessageMutator { key: "M" },
            ParamSpec::Res {
                key: "r",
                vkey: Some(1),
                name: "R".to_string(),
                mutable: true,
            },
            mut_query("T", QuerySpec::default()),
        ];
        for param in &rejected {
            assert!(condition_param_rejection(param).is_some());
        }

        let allowed: [ParamSpec<MockKeys>; 4] = [
            ParamSpec::Local,
            ParamSpec::Res {
                key: "r",
                vkey: Some(1),
                name: "R".to_string(),
                mutable: false,
            },
            ParamSpec::Query(QuerySpec {
                components: vec![comp("T", false, false)],
                ..Default::default()
            }),
            ParamSpec::Assets {
                key: "Mesh",
                vkey: "Mesh".to_string(),
                name: "Mesh".to_string(),
                mutable: false,
            },
        ];
        for param in &allowed {
            assert!(condition_param_rejection(param).is_none());
        }
    }

    #[test]
    fn describe_renders_canonical_strings() {
        let params: [ParamSpec<MockKeys>; 6] = [
            ParamSpec::Query(QuerySpec {
                components: vec![comp("Position", true, false), comp("Velocity", false, true)],
                with: vec![filter("Player")],
                without: vec![filter("Frozen")],
                changed: vec![filter("Health")],
                ..Default::default()
            }),
            ParamSpec::Res {
                key: "time",
                vkey: Some(1),
                name: "Time".to_string(),
                mutable: false,
            },
            ParamSpec::Assets {
                key: "Mesh",
                vkey: "Mesh".to_string(),
                name: "Mesh".to_string(),
                mutable: true,
            },
            ParamSpec::MessageReader { key: "Collision" },
            ParamSpec::MessageMutator { key: "Collision" },
            ParamSpec::Commands,
        ];
        assert_eq!(
            describe_param_specs(&params),
            vec![
                "query components=[Position(mut), Velocity(read,opt)] \
                 with=1 without=1 changed=1 added=0 resolve_only=0",
                "res mutable=false vkey=present",
                "assets mutable=true",
                "message_reader",
                "message_mutator",
                "commands",
            ]
        );
    }
}
