//! Interpreter-neutral wording for stable public Python errors.
//!
//! Backend adapters still choose the native exception class. Put messages here
//! when both PyO3 and RustPython expose the same invalid operation so wording
//! cannot drift independently.

use std::fmt::{Debug, Display};

pub use pybevy_storage::{
    conflict_message::{
        CONFLICT_ASSETS, CONFLICT_ASSETS_SHARED_VIEW, CONFLICT_MESSAGES, CONFLICT_QUERIES,
        CONFLICT_RESOURCE_QUERY, CONFLICT_RESOURCES, CONFLICT_WORLD, SystemAccessConflictMessage,
    },
    storage_error::{
        EXPRESSION_MAX_DEPTH, NESTED_EXECUTION, enum_variant_changed, expression_too_deep,
    },
};

pub fn mcp_scalar_object(type_name: &str) -> String {
    format!("expected {type_name} value, got object")
}

pub const CONSTRUCTOR_ROTATION: &str =
    "Rot2() requires finite cos and sin with cos*cos + sin*sin within 0.0002 of 1";

pub fn constructor_partial_group(ty: &str, group: &[&str], missing: &[&str]) -> String {
    format!(
        "{ty}() requires ({}) together; missing: {}",
        group.join(", "),
        missing.join(", ")
    )
}

pub fn constructor_argument_type(ty: &str, param: &str, expected: &str, got: &str) -> String {
    format!("{ty}() argument '{param}' must be {expected}, not {got}")
}

pub fn constructor_form_conflict(ty: &str, given: &[&str], alternate: &[&str]) -> String {
    format!(
        "{ty}() cannot combine ({}) with ({})",
        given.join(", "),
        alternate.join(", ")
    )
}

pub const ADD_SYSTEMS_SCHEDULE_TYPE: &str =
    "add_systems() schedule parameter must be Stage, OnEnter(), OnExit(), or OnTransition()";

pub fn plugin_key_type(qualified_name: impl Display, received_type: impl Display) -> String {
    format!("{qualified_name}.__pybevy_plugin_key__ must be str, got {received_type}")
}

pub fn duplicate_plugin_identity(qualified_name: impl Display, key: impl Display) -> String {
    format!("plugin {qualified_name} with __pybevy_plugin_key__='{key}' was added more than once")
}
pub const ANISOTROPY_TEXTURE_UNAVAILABLE: &str = "anisotropy_texture is unavailable on macOS and iOS: Metal's 16-sampler-per-stage \
     limit is exceeded. anisotropy_strength and anisotropy_rotation still work.";
pub const MULTI_LAYER_MATERIAL_TEXTURES_UNAVAILABLE: &str = "clearcoat textures are unavailable on macOS and iOS: Metal's 16-sampler-per-stage \
     limit is exceeded. The matching non-texture fields still work.";
pub const SPECULAR_TEXTURES_UNAVAILABLE: &str = "specular textures are unavailable on macOS and iOS: Metal's 16-sampler-per-stage \
     limit is exceeded. The matching non-texture fields still work.";
pub const TRANSMISSION_TEXTURES_UNAVAILABLE: &str = "transmission textures are unavailable on macOS and iOS: Metal's 16-sampler-per-stage \
     limit is exceeded. The matching non-texture fields still work.";
pub const ANIMATION_GRAPH_NODE_READ_ONLY: &str =
    "Cannot modify a node obtained from AnimationGraph.get(); use get_mut() instead";
pub const ANIMATION_GRAPH_NODE_MISSING: &str = "the node is no longer in the animation graph";
pub const RESOURCE_COMPONENT_INSERT: &str =
    "resources cannot be inserted as ordinary entity components";
pub const RESOURCE_COMPONENT_REMOVE: &str =
    "resources cannot be removed as ordinary entity components";
pub const RESOURCE_COMPONENT_SPAWN: &str =
    "resources cannot be spawned as ordinary entity components";
pub const RESOURCE_ENTITY_DESPAWN: &str =
    "resource entities cannot be despawned; use remove_resource() to remove the resource";
pub const RESOURCE_ENTITY_REPARENT: &str = "resource entities cannot take part in a parent-child relationship; despawning the parent \
     would discard the resource value";
pub const HIERARCHY_SELF_PARENT: &str = "an entity cannot be its own parent";
pub const HIERARCHY_CYCLE: &str = "the requested parent would create a hierarchy cycle";
pub const WORLD_SERIALIZATION_TYPE_REGISTRY_MISSING: &str =
    "cannot serialize a World without AppTypeRegistry";
pub const IS_RESOURCE_COMPONENT_REMOVE: &str =
    "IsResource cannot be removed; use remove_resource() to remove the resource";
pub const RESOURCE_VIEW_DATA: &str = "resources are supported by Query, not View";
pub const RESOURCE_VIEW_FILTER: &str = "resource filters are supported by Query, not View";
pub const NEXT_STATE_CONSTRUCTION: &str = "NextState cannot be constructed directly; access `world.resource(NextState[MyState])` or declare `next_state: ResMut[NextState[MyState]]` in a system";
pub const DURATION_NEGATIVE: &str = "Duration cannot be negative";
pub const DURATION_NON_FINITE: &str = "Duration must be finite";
pub const DURATION_OVERFLOW: &str = "Duration is too large";
pub const DURATION_ZERO: &str = "Duration must be greater than zero";
pub const FREQUENCY_NON_FINITE: &str = "Frequency must be finite";
pub const FREQUENCY_NON_POSITIVE: &str = "Frequency must be greater than zero";
pub const FREQUENCY_OUT_OF_RANGE: &str =
    "Frequency produces a timestep outside the supported duration range";
pub const SPEED_NON_FINITE: &str = "Speed must be finite";
pub const RELATIVE_SPEED_NON_FINITE: &str = "Relative speed must be finite";
pub const RELATIVE_SPEED_NEGATIVE: &str = "Relative speed cannot be negative";
pub const RELATIVE_SPEED_OUT_OF_RANGE: &str =
    "Relative speed produces a frame delta outside the supported duration range";
pub const COLOR_INTERPOLATION_MISMATCH: &str =
    "cannot interpolate Color values from different color spaces";
pub const BLOOM_MAX_MIP_DIMENSION_ZERO: &str = "Bloom.max_mip_dimension must be at least 1, got 0";

pub fn viewport_conversion_failed(error: impl Display) -> String {
    format!("Viewport conversion failed: {error}")
}

pub fn mesh_operation_failed(operation: impl Display, error: impl Display) -> String {
    format!("Mesh.{operation}() failed: {error}")
}

pub fn shader_stage_invalid(stage: impl Display) -> String {
    format!("Invalid shader stage: {stage}. Must be 'vertex', 'fragment', or 'compute'")
}

pub fn invalid_hex_color(error: impl Display) -> String {
    format!("Invalid hex color: {error}")
}

pub fn gamepad_settings_failed(kind: impl Display, error: impl Display) -> String {
    format!("failed to create {kind} gamepad settings: {error}")
}

pub fn plugin_not_a_plugin(plugin_name: &str, mro: &str) -> String {
    format!(
        "Expected a Plugin instance or type, but got '{plugin_name}'\n\
         \n\
         Inheritance chain: {mro}\n\
         \n\
         Possible causes:\n\
         • The class does not inherit from Plugin (ensure 'class {plugin_name}(Plugin):')\n\
         • Missing '@plugin' decorator (add @plugin above the class)\n\
         • The Plugin class was not imported correctly (check 'from pybevy.app import Plugin')\n\
         \n\
         Example:\n\
         from pybevy.app import Plugin\n\
         from pybevy.decorators import plugin\n\
         \n\
         @plugin\n\
         class MyPlugin(Plugin):\n\
             def build(self, app):\n\
                 pass"
    )
}

pub fn plugin_build_error(plugin_name: &str, detail: impl Display) -> String {
    format!("Failed to build plugin '{plugin_name}': {detail}")
}

pub fn plugin_missing_decorator(plugin_name: &str) -> String {
    format!(
        "Plugin class '{plugin_name}' must be decorated with @plugin decorator\n\
         \n\
         Add the @plugin decorator above your plugin class:\n\
         \n\
         from pybevy.app import Plugin\n\
         from pybevy.decorators import plugin\n\
         \n\
         @plugin  # <- Add this!\n\
         class {plugin_name}(Plugin):\n\
             def build(self, app):\n\
                 pass"
    )
}
pub const EXPECTED_ASSET_ID_OR_HANDLE: &str = "expected an AssetId or Handle";
pub const ASSET_BRIDGE_NOT_FOUND: &str = "Asset bridge not found for type";
pub const ASSET_ACCESS_REGISTRY_MISSING: &str = "PyBevy asset access registry is missing from the World; initialize PyBevyPlugin before running Python systems";
pub const ASSET_EVENT_TYPE_REQUIRED: &str =
    "AssetEvent requires an asset type; use AssetEvent[Image]";
pub const ASSET_EVENT_READ_ONLY: &str =
    "AssetEvent messages are read-only (generated by the asset system)";
pub const ASSET_EVENT_NO_DEFAULT: &str = "AssetEvent messages have no default value";
pub const ASSET_LOAD_FAILED_TYPE_REQUIRED: &str =
    "AssetLoadFailedEvent requires an asset type; use AssetLoadFailedEvent[Image]";
pub const ASSET_LOAD_FAILED_READ_ONLY: &str =
    "AssetLoadFailedEvent messages are read-only (generated by the asset system)";
pub const ASSET_LOAD_FAILED_NO_DEFAULT: &str =
    "AssetLoadFailedEvent messages have no default value";
pub const RESOURCE_BRIDGE_NOT_FOUND: &str = "Resource bridge not found for dynamic type";
pub const ASSET_SERVER_MANUAL_INSERT: &str =
    "AssetServer cannot be manually inserted. It is provided by AssetPlugin.";
pub const ASSET_SERVER_MANUAL_REMOVE: &str =
    "AssetServer cannot be manually removed. It is managed by AssetPlugin.";
pub const ANY_OF_TUPLE_REQUIRED: &str =
    "AnyOf query data requires a tuple: AnyOf[tuple[A, B, Mut[C]]]";
pub const ANY_OF_EMPTY: &str = "AnyOf requires at least one query-data item";
pub const OR_FILTER_TUPLE_REQUIRED: &str =
    "Or requires a tuple of query filters: Or[tuple[With[A], Changed[B]]]";
pub const OR_FILTER_ITEM_REQUIRED: &str =
    "Or items must be query filters such as With[A], Without[B], Changed[C], Added[D], or Or[...]";
pub const OR_FILTER_EMPTY: &str = "Or requires at least one query filter";
pub const OR_IS_FILTER: &str = "Or[...] is a query filter, not query data; place it after the data tuple: Query[tuple[...], Or[tuple[With[A], With[B]]]]";
pub const ANY_OF_VIEW_UNSUPPORTED: &str = "AnyOf[...] query data is not supported in View. Use Query for optional per-entity component values.";
pub const OR_VIEW_UNSUPPORTED: &str =
    "Or[...] is not supported in View. Use Query for disjunctive filters.";
pub const SHADER_DEFS_WITHOUT_NAMES: &str =
    "shader_defs contains enabled bits without matching shader_def_names entries";

pub const TIME_CONTEXT_TYPE_REQUIRED: &str =
    "Time[...] expects one of the Fixed, Real, or Virtual marker types";

pub const TIMER_ELAPSED_PAST_DURATION: &str = "the remaining time is undefined while Timer elapsed is past the duration; \
     call set_elapsed() with a value within the duration";

pub fn too_many_shader_def_names(actual: usize) -> String {
    format!("shader_def_names accepts at most 32 entries; got {actual}")
}

pub const UNSUPPORTED_GIZMO_LINE_STYLE: &str =
    "the native GizmoLineStyle variant is not supported by this PyBevy build";
pub const GIZMOS_PLUGIN_REQUIRED: &str = "Gizmos requires DefaultGizmoConfigGroup; add GizmoPlugin before using the Gizmos system parameter";
pub const DEFAULT_GIZMO_CONFIG_MISSING: &str =
    "DefaultGizmoConfigGroup is not registered; add GizmoPlugin before accessing its config";

pub fn unregistered_gizmo_config_group(name: &str) -> String {
    format!("{name} is not a registered gizmo config group")
}

pub fn missing_gizmo_config_group(name: &str) -> String {
    format!(
        "{name} is not present in GizmoConfigStore; add the plugin that owns this gizmo config group"
    )
}
pub const CONDITION_WORLD: &str = "World (exclusive world access)";
pub const CONDITION_COMMANDS: &str = "Commands (queued mutations are never applied to conditions)";
pub const CONDITION_GIZMOS: &str = "Gizmos (drawing mutates the deferred gizmo buffer)";
pub const CONDITION_INPUT: &str = "In (pipe inputs are not valid run-condition parameters)";
pub const CONDITION_MESSAGE_WRITER: &str = "MessageWriter (writing messages mutates the world)";
pub const CONDITION_MESSAGE_MUTATOR: &str =
    "MessageMutator (reading, mutating, and writing messages mutates the world)";
pub const CONDITION_MESSAGE_READER: &str = "MessageReader (advancing the read cursor mutates reader state; read messages in a regular system instead)";
pub const CONDITION_OPAQUE_MESSAGE_READER: &str =
    "MessageReader for a custom Python message (opaque Python access is exclusive)";
pub const CONDITION_RES_MUT: &str = "ResMut (mutable resource access)";
pub const CONDITION_OPAQUE_RESOURCE: &str =
    "Res for a custom Python resource (opaque Python access is exclusive)";
pub const CONDITION_MUTABLE_ASSETS: &str = "mutable Assets (mutable asset access)";
pub const CONDITION_MUTABLE_QUERY: &str = "Query with a Mut component (mutable component access)";
pub const CONDITION_OPAQUE_QUERY: &str =
    "Query with a Python-storage component (opaque Python access is exclusive)";
pub const CONDITION_MUTABLE_VIEW: &str = "View with a Mut component (mutable component access)";
pub const CONDITION_OPAQUE_VIEW: &str = "View with opaque Python component access";

/// The spelling a rejected system parameter annotation was written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemParamWrapper {
    Bare,
    Mut,
    Res,
    ResMut,
}

impl SystemParamWrapper {
    fn written(self, type_name: &str) -> String {
        match self {
            SystemParamWrapper::Bare => type_name.to_string(),
            SystemParamWrapper::Mut => format!("Mut[{type_name}]"),
            SystemParamWrapper::Res => format!("Res[{type_name}]"),
            SystemParamWrapper::ResMut => format!("ResMut[{type_name}]"),
        }
    }
}

/// Interpreter-neutral reason a system parameter annotation cannot be lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemParamRejection {
    MissingAnnotation,
    BareResource,
    MessageType,
    NativeMessageType,
    ComponentType,
    AssetType,
    NakedMut,
    NakedMutAssets,
    Unsupported,
}

/// Valid system parameter spellings, listed when nothing else fits.
const SYSTEM_PARAM_KINDS: &str = "Commands, World, Gizmos, Query[...], Single[...], View[...], \
     Res[...], ResMut[...], Res[Assets[...]], MessageReader[...], MessageWriter[...], \
     MessageMutator[...], Local[...], In[...], and On[...]";

/// The user-facing rejection message for one system parameter, identical on
/// both backends. `type_name` is the public annotation spelling where known,
/// otherwise the annotation's repr.
pub fn system_param_rejection_message(
    system: &str,
    param: &str,
    wrapper: SystemParamWrapper,
    type_name: &str,
    kind: SystemParamRejection,
) -> String {
    let prefix = format!("System function `{system}` parameter `{param}`");
    let written = wrapper.written(type_name);
    match kind {
        SystemParamRejection::MissingAnnotation => format!("{prefix} has no type annotation"),
        SystemParamRejection::BareResource => format!(
            "{prefix} must use Res[{type_name}] for read-only or ResMut[{type_name}] for mutable access"
        ),
        SystemParamRejection::MessageType => format!(
            "{prefix} is `{written}`, but `{type_name}` is a message type. Use MessageReader[{type_name}] to read messages, MessageWriter[{type_name}] to send them, or MessageMutator[{type_name}] to modify them in place"
        ),
        SystemParamRejection::NativeMessageType => format!(
            "{prefix} is `{written}`, but `{type_name}` is a native message type. Use MessageReader[{type_name}] to read messages or MessageWriter[{type_name}] to send them. MessageMutator only supports custom Python messages"
        ),
        SystemParamRejection::ComponentType => format!(
            "{prefix} is `{written}`, but `{type_name}` is a component type. Use Query[{type_name}] for read-only access, Query[Mut[{type_name}]] for mutable access, or Single[{type_name}] when exactly one entity matches"
        ),
        SystemParamRejection::AssetType => format!(
            "{prefix} is `{written}`, but `{type_name}` is an asset type. Use Res[Assets[{type_name}]] for read-only access or ResMut[Assets[{type_name}]] for mutable access"
        ),
        SystemParamRejection::NakedMut => format!(
            "{prefix} is `{written}`, but Mut[...] is only valid inside Query, Single, or View. Use Query[Mut[{type_name}]], Single[Mut[{type_name}]], or View[Mut[{type_name}]]"
        ),
        SystemParamRejection::NakedMutAssets => format!(
            "{prefix} is `{written}`, but Mut[...] is only valid inside Query, Single, or View. Use Res[{type_name}] for read-only access or ResMut[{type_name}] for mutable access"
        ),
        SystemParamRejection::Unsupported => format!(
            "{prefix} has an unsupported system parameter `{written}`. Valid system parameters are {SYSTEM_PARAM_KINDS}"
        ),
    }
}

pub fn active_asset_access(
    operation: &str,
    asset_name: &str,
    origin: &str,
    asset_id: &str,
) -> String {
    format!(
        "Cannot call {operation} while a borrowed {asset_name} asset from {origin} is live ({asset_id}). Drop the asset wrapper or close its view first."
    )
}

pub fn resource_type_not_found(name: impl Display) -> String {
    format!(
        "Resource type `{name}` is not found. Did you call `init_resource` or `insert_resource`?"
    )
}

pub fn resource_not_present(name: impl Display) -> String {
    format!("Resource type `{name}` not present in the world")
}

pub fn unregistered_message_write(name: impl Display) -> String {
    format!(
        "Unable to write message `{name}`: Message type is not registered; call app.add_message(T) first"
    )
}

pub fn world_serialization_failed(error: impl Display) -> String {
    format!("failed to serialize world: {error}")
}

pub fn world_serialization_skipped_custom_types(
    components: &[String],
    resources: &[String],
) -> String {
    let mut groups = Vec::new();
    if !components.is_empty() {
        groups.push(format!("components [{}]", components.join(", ")));
    }
    if !resources.is_empty() {
        groups.push(format!("resources [{}]", resources.join(", ")));
    }
    format!(
        "DynamicWorld.from_world() skipped custom Python ECS values that Bevy cannot reflect: {}. Wrapper-stored @component values are serializable; Python-object components and custom @resource values are not.",
        groups.join("; ")
    )
}

pub fn world_serialization_skipped_reflected_types(
    components: &[String],
    resources: &[String],
) -> String {
    let mut groups = Vec::new();
    if !components.is_empty() {
        groups.push(format!("components [{}]", components.join(", ")));
    }
    if !resources.is_empty() {
        groups.push(format!("resources [{}]", resources.join(", ")));
    }
    format!(
        "DynamicWorld.from_world() skipped reflected ECS root values whose nested data cannot be serialized: {}.",
        groups.join("; ")
    )
}

pub fn expected_resource_subclass(actual: impl Display) -> String {
    format!(
        "Expected a subclass of `Resource`, but got `{actual}` which is not a subclass of `Resource`"
    )
}

pub fn resource_decorator_required(name: impl Display) -> String {
    format!("Resource class '{name}' must be decorated with @resource decorator")
}

pub fn invalid_asset_type(actual: impl Display) -> String {
    format!("Invalid asset type. Expected a subclass of `Asset`, but got `{actual}`")
}

pub fn non_negative_argument(name: &str, value: isize) -> String {
    format!("{name} must be >= 0, got {value}")
}

pub fn expected_float_sequence(actual: impl Display) -> String {
    format!("values must be a sequence of floats, got {actual}")
}

pub fn entity_does_not_exist(entity: impl Debug) -> String {
    format!("Entity {entity:?} does not exist")
}

pub fn hierarchy_parent_does_not_exist(entity: impl Display) -> String {
    format!("Parent entity {entity} does not exist")
}

pub fn pipe_input_outside_pipe(system: impl Display) -> String {
    format!("System `{system}` uses In[...] but is not a downstream pipe stage")
}

pub fn pipe_target_requires_input(system: impl Display) -> String {
    format!("Pipe target `{system}` must declare In[T] as its first parameter")
}

pub fn pipe_input_must_be_first(system: impl Display) -> String {
    format!("Pipe target `{system}` must declare exactly one In[T] parameter, in first position")
}

pub fn pipe_input_type_mismatch(
    system: impl Display,
    expected: impl Display,
    actual: impl Display,
) -> String {
    format!("Pipe target `{system}` expected {expected}, got {actual}")
}

pub fn invalid_entity_bits(bits: u64) -> String {
    format!("{bits} is not a valid entity bit pattern: the entity index cannot be zero")
}

pub fn custom_component_wrapper_storage(component: impl Display) -> String {
    format!(
        "Component '{component}' uses wrapper storage, which does not support MCP field mutation. Declare it with `@component(storage=\"python\")` to make its fields editable through `set_component`."
    )
}

pub fn injected_system_parameter(type_name: &str, example: &str) -> String {
    format!(
        "{type_name} cannot be constructed directly; declare it as a system parameter annotation, for example `{example}`"
    )
}

pub fn system_annotations_unresolved(system: impl Display, error: impl Display) -> String {
    format!("System function `{system}` annotations could not be resolved: {error}")
}

pub fn system_resource_not_found(
    system: impl Display,
    resource: impl Display,
    custom: bool,
) -> String {
    let message = format!("System `{system}`: resource `{resource}` not found in world");
    if !custom {
        return message;
    }
    format!(
        "{message}. `@resource` registers the type but does not insert an instance; call `app.insert_resource({resource}(...))` before the system runs, or `commands.insert_resource({resource}(...))` from an earlier Startup system"
    )
}

pub fn expected_state_member(got_type: impl Display) -> String {
    format!("expected an @state enum member, got type {got_type}")
}

pub fn expected_state_member_got_state_type(state_type: impl Display) -> String {
    format!("expected an @state enum member, got state type {state_type}; use {state_type}.MEMBER")
}

pub fn state_resource_descriptor_required(state_type: impl Display) -> String {
    format!(
        "State enum `{state_type}` is not itself a Resource; use `State[{state_type}]` for the current state or `NextState[{state_type}]` for pending transitions"
    )
}

/// Longest common substring of `a` and `b`, as (a_start, b_start, len).
fn longest_match(a: &[char], b: &[char]) -> (usize, usize, usize) {
    let (mut best_a, mut best_b, mut best_len) = (0usize, 0usize, 0usize);
    let mut row = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        let mut prev_diag = 0usize;
        for (j, cb) in b.iter().enumerate() {
            let current = row[j + 1];
            row[j + 1] = if ca == cb { prev_diag + 1 } else { 0 };
            if row[j + 1] > best_len {
                best_len = row[j + 1];
                best_a = i + 1 - best_len;
                best_b = j + 1 - best_len;
            }
            prev_diag = current;
        }
    }
    (best_a, best_b, best_len)
}

/// Total matched characters under Ratcliff/Obershelp.
fn matching_chars(a: &[char], b: &[char]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let (i, j, len) = longest_match(a, b);
    if len == 0 {
        return 0;
    }
    len + matching_chars(&a[..i], &b[..j]) + matching_chars(&a[i + len..], &b[j + len..])
}

/// Ratcliff/Obershelp similarity: 2 * matched / total.
/// Not edit distance: Bevy renames are mostly insertions, which Levenshtein punishes.
fn similarity(a: &str, b: &str) -> f64 {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let total = ac.len() + bc.len();
    if total == 0 {
        return 0.0;
    }
    2.0 * matching_chars(&ac, &bc) as f64 / total as f64
}

/// Candidates sharing a long run of characters with `unexpected`.
/// Rescues near-misses the ratio cutoff rejects, like `rughness`.
fn substring_relatives<'a>(unexpected: &str, candidates: &[&'a str]) -> Vec<&'a str> {
    const MIN_SHARED: usize = 4;
    let target: Vec<char> = unexpected.chars().collect();
    let mut scored: Vec<(usize, &'a str)> = candidates
        .iter()
        .filter_map(|candidate| {
            let chars: Vec<char> = candidate.chars().collect();
            let (_, _, shared) = longest_match(&target, &chars);
            (shared >= MIN_SHARED).then_some((shared, *candidate))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.len().cmp(&b.1.len())));
    scored.truncate(3);
    scored.into_iter().map(|(_, name)| name).collect()
}

/// Ratio floor for a confident suggestion. Real renames either contain the
/// wrong keyword outright (`roughness` in `perceptual_roughness`) or score
/// far above this; the 0.60-0.75 band is where shared-prefix neighbours like
/// `emissive_intensity` -> `emissive_texture` produce confident nonsense.
const SUGGESTION_CUTOFF: f64 = 0.75;
/// Above this many keywords, listing them all is noise rather than help.
const MAX_LISTED_KEYWORDS: usize = 16;

/// A parsed "unexpected keyword argument" TypeError.
#[derive(Debug, PartialEq, Eq)]
pub struct UnexpectedKeyword<'a> {
    /// Callable name with any `.__new__` / `.__init__` suffix stripped.
    pub callable: &'a str,
    /// The keyword the caller got wrong.
    pub keyword: &'a str,
}

/// Parse either backend's wording for an unexpected-keyword TypeError.
///
/// PyO3 renders `StandardMaterial.__new__() got an unexpected keyword
/// argument 'roughness'`; RustPython renders `StandardMaterial() got an
/// unexpected keyword argument 'roughness'`. One parser handles both so a
/// pattern tuned to one backend cannot silently no-op on the other.
pub fn parse_unexpected_keyword(message: &str) -> Option<UnexpectedKeyword<'_>> {
    const MARKER: &str = "() got an unexpected keyword argument ";
    let (head, tail) = message.split_once(MARKER)?;

    // CPython appends its own suggestion after the keyword for pure-Python
    // callables ("...argument 'helth'. Did you mean 'health'?"), so take the
    // first quoted run rather than assuming the keyword ends the message.
    let tail = tail.trim().strip_prefix('\'')?;
    let keyword = tail.split('\'').next()?;
    if keyword.is_empty() {
        return None;
    }

    let head = head.rsplit(':').next()?.trim();
    let callable = head
        .strip_suffix(".__new__")
        .or_else(|| head.strip_suffix(".__init__"))
        .unwrap_or(head);
    let callable = callable.rsplit(['.', ' ']).next()?.trim();
    if callable.is_empty() {
        return None;
    }
    Some(UnexpectedKeyword { callable, keyword })
}

/// A parsed "not an instance of" TypeError.
#[derive(Debug, PartialEq, Eq)]
pub struct NotAnInstance<'a> {
    /// The type name of what the caller actually passed.
    pub actual: &'a str,
    /// The class the binding layer expected.
    pub expected: &'a str,
}

/// Parse the binding layer's wording for a failed pyclass extraction.
pub fn parse_not_an_instance(message: &str) -> Option<NotAnInstance<'_>> {
    const MARKER: &str = " object is not an instance of ";
    let (head, tail) = message.split_once(MARKER)?;
    let actual = head.rsplit(':').next()?.trim().trim_matches('\'');
    let expected = tail.trim().trim_end_matches('.').trim_matches('\'');
    if actual.is_empty() || expected.is_empty() {
        return None;
    }
    Some(NotAnInstance { actual, expected })
}

/// Whether `message` already carries a variant-constructor hint.
pub fn has_variant_hint(message: &str) -> bool {
    message.contains("variants are constructors")
}

/// Hint for passing a variant class where an instance was expected.
///
/// `variants` must be in declaration order. Returns None when the expected
/// class has no nested variant classes, where the mistake is something else.
pub fn variant_constructor_hint(expected: &str, variants: &[&str]) -> Option<String> {
    let first = variants.first()?;
    Some(format!(
        "{expected} variants are constructors: pass {expected}.{first}() with \
         parentheses, not {expected}.{first}. Variants: {}.",
        variants.join(", ")
    ))
}

/// The closest valid keyword to `unexpected`, if one is close enough.
pub fn suggest_keyword<'a>(unexpected: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let normalize = |value: &str| value.replace('_', "").to_lowercase();
    let target = normalize(unexpected);

    let mut best: Option<(f64, &'a str)> = None;
    for candidate in candidates {
        let exact = normalize(candidate) == target;
        let ratio = if exact {
            1.0
        } else {
            similarity(unexpected, candidate)
        };

        // Widens eligibility without flattening the ranking: scoring every
        // container alike would pick whichever sorted first.
        let contained = (unexpected.len() >= 4 && candidate.contains(unexpected))
            || (candidate.len() >= 4 && unexpected.contains(candidate));
        if !(exact || contained || ratio >= SUGGESTION_CUTOFF) {
            continue;
        }

        if best.is_none_or(|(best_ratio, best_name)| {
            ratio > best_ratio
                || (ratio == best_ratio
                    && (candidate.len(), *candidate) < (best_name.len(), best_name))
        }) {
            best = Some((ratio, candidate));
        }
    }
    best.map(|(_, name)| name)
}

/// Whether `message` already carries a keyword hint, from any tier.
///
/// Guards against double-hinting: CPython suggests for pure-Python callables,
/// and if the binding layer ever suggests at raise time this crate must not
/// append a second one. Covers every tier `unexpected_keyword_hint` emits, so
/// adding a tier there without updating this is not possible by omission.
pub fn has_kwarg_hint(message: &str) -> bool {
    const MARKERS: [&str; 4] = [
        "Did you mean",
        "Valid keyword arguments for",
        "Closest are:",
        "No close match among",
    ];
    MARKERS.iter().any(|marker| message.contains(marker))
}

/// Hint appended to an unexpected-keyword TypeError, or None to stay silent.
///
/// Appended inline after a period, which is what CPython already does for
/// pure-Python callables: `Player.__init__() got an unexpected keyword
/// argument 'helth'. Did you mean 'health'?`. Matching that shape means the
/// same mistake reads the same whether the class is a PyO3 type or an ordinary
/// `@component` dataclass.
///
/// `valid` must be in declaration order. Silence is deliberate when nothing is
/// close and the callable is large: a wrong-confident suggestion is worse than
/// none.
pub fn unexpected_keyword_hint(callable: &str, keyword: &str, valid: &[&str]) -> Option<String> {
    if valid.is_empty() {
        return None;
    }
    if let Some(best) = suggest_keyword(keyword, valid) {
        return Some(format!("Did you mean '{best}'?"));
    }
    if valid.len() <= MAX_LISTED_KEYWORDS {
        return Some(format!(
            "Valid keyword arguments for {callable}(): {}",
            valid.join(", ")
        ));
    }
    let relatives = substring_relatives(keyword, valid);
    if !relatives.is_empty() {
        return Some(format!("Closest are: {}.", relatives.join(", ")));
    }
    Some(format!(
        "No close match among the {} keyword arguments of {callable}(); run help({callable}) for the list.",
        valid.len()
    ))
}

pub fn asset_type_mismatch(actual: impl Display, expected: impl Display) -> String {
    format!("AssetType `{actual}` does not match expected type `{expected}`")
}

pub fn unsupported_texture_format(format: impl Display) -> String {
    format!("Texture format `{format}` has no Python representation")
}

pub fn cubic_hermite_tangent_count(expected: usize, provided: usize) -> String {
    format!("Incorrect number of tangents: expected {expected}, provided {provided}")
}

pub const EMPTY_POINT_CLOUD: &str = "point cloud must contain at least one point";
pub const INFINITE_PLANE_POINTS: &str =
    "infinite plane must be defined by three finite, non-collinear points";
pub const INTEGER_VECTOR_OVERFLOW: &str = "integer vector arithmetic overflow";

pub const COMPONENT_FLOAT_NON_FINITE: &str = "float must be finite";

pub const ASSET_LOADING_TASK_POOL_MISSING: &str =
    "AssetServer loading requires TaskPoolPlugin (IoTaskPool is not initialized)";
pub fn bounding_shrink(kind: &str) -> String {
    format!("{kind} shrink must preserve nonnegative extents")
}

pub const AABB3D_MIN_MAX: &str = "Aabb3d min must not exceed max on any axis";

pub fn enum_variant_missing_field(variant: &str, field: &str) -> String {
    format!("variant '{variant}' requires field '{field}'")
}

pub fn mesh_triangle_list_required(operation: &str, topology: impl Display) -> String {
    format!("{operation} requires PrimitiveTopology.TriangleList (got {topology})")
}

pub fn mesh_indexed_required(operation: &str) -> String {
    format!(
        "{operation} requires indexed geometry; call insert_indices() first, or use compute_flat_normals() on non-indexed geometry"
    )
}

pub fn mesh_index_out_of_range(operation: &str, index: usize, vertices: usize) -> String {
    format!(
        "{operation} found vertex index {index} but the mesh has only {vertices} vertices; check the array passed to insert_indices()"
    )
}

pub fn mesh_degenerate_scale(operation: &str, scale: impl Display) -> String {
    format!("{operation} requires a scale with at most one zero axis (got {scale})")
}

pub fn mesh_attribute_format(name: &str, expected: impl Display, given: impl Display) -> String {
    format!("attribute {name} expects {expected} but the given data is {given}")
}

pub const NON_CONTIGUOUS_ATTRIBUTE_ARRAY: &str = "Attribute arrays must be C-contiguous in memory. The dtype and shape are fine; the layout is not. Pass numpy.ascontiguousarray(values).";

pub fn bounding_half_size(kind: &str, axes: &[f32]) -> String {
    let values = axes
        .iter()
        .map(f32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{kind} half_size must be nonnegative, got ({values})")
}

pub fn bounding_radius(kind: &str, radius: f32) -> String {
    format!("{kind} radius must be nonnegative, got {radius}")
}

pub fn bounding_operation(kind: &str, operation: &str) -> String {
    format!("{kind} {operation} must preserve nonnegative extents")
}

pub const INTEGER_RECT_SIZE_NEGATIVE: &str = "IRect size must be non-negative";

pub const INTEGER_RECT_HALF_SIZE_NEGATIVE: &str = "IRect half_size must be non-negative";

pub fn unsigned_rect_origin(origin: [u32; 2], extent: [u32; 2], half_size: bool) -> String {
    let requirement = if half_size { "half_size" } else { "(size / 2)" };
    let operand = if half_size { "half_size" } else { "size" };
    format!(
        "Origin must always be greater than or equal to {requirement} otherwise the rectangle is undefined! Origin was [{}, {}] and {operand} was [{}, {}]",
        origin[0], origin[1], extent[0], extent[1]
    )
}

pub fn image_encoding_unsupported(format: impl Display, supported: &[String]) -> String {
    if supported.is_empty() {
        format!("{format} encoding is not supported; no image encoders are enabled")
    } else {
        format!(
            "{format} encoding is not supported; use {}",
            supported.join(", ")
        )
    }
}

pub const CASCADE_BOUNDS_EMPTY: &str = "bounds cannot be empty";

pub const CASCADE_OVERLAP_RANGE: &str = "overlap_proportion must be in range [0.0, 1.0)";

pub const CASCADE_MINIMUM_DISTANCE: &str = "minimum_distance must be non-negative";

pub const CONTROL_SERIALIZATION_CYCLE: &str = "cyclic Python value";
pub const CONTROL_SERIALIZATION_DEPTH: &str = "Python value exceeds the serialization depth limit";

pub fn image_extension_unknown(extension: impl Display) -> String {
    format!(
        "cannot infer an image format from the extension '.{extension}'; pass format= explicitly, for example format=ImageFormat.Png"
    )
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    // Fixture strings copied from real backend output, so these pin foreign
    // wording rather than a constant against its own literal.
    const PYO3_MESSAGE: &str =
        "StandardMaterial.__new__() got an unexpected keyword argument 'roughness'";
    const RP2_MESSAGE: &str = "StandardMaterial() got an unexpected keyword argument 'roughness'";

    #[test]
    fn parses_both_backend_shapes() {
        let expected = UnexpectedKeyword {
            callable: "StandardMaterial",
            keyword: "roughness",
        };
        assert_eq!(parse_unexpected_keyword(PYO3_MESSAGE), Some(expected));
        assert_eq!(
            parse_unexpected_keyword(RP2_MESSAGE),
            Some(UnexpectedKeyword {
                callable: "StandardMaterial",
                keyword: "roughness",
            })
        );
    }

    #[test]
    fn parses_both_backend_not_an_instance_shapes() {
        assert_eq!(
            parse_not_an_instance("'type' object is not an instance of 'AlphaMode'"),
            Some(NotAnInstance {
                actual: "type",
                expected: "AlphaMode"
            })
        );
        assert_eq!(
            parse_not_an_instance("TypeError: 'int' object is not an instance of 'AlphaMode'"),
            Some(NotAnInstance {
                actual: "int",
                expected: "AlphaMode"
            })
        );
        assert!(parse_not_an_instance("ValueError: something else").is_none());
    }

    #[test]
    fn parses_with_exception_class_prefix_and_dunder_init() {
        let parsed = parse_unexpected_keyword(
            "TypeError: Player.__init__() got an unexpected keyword argument 'helth'",
        )
        .expect("should parse");
        assert_eq!(parsed.callable, "Player");
        assert_eq!(parsed.keyword, "helth");
    }

    #[test]
    fn parses_cpython_message_that_already_suggests() {
        // CPython suggests for pure-Python callables; parsing must still work so
        // the caller can see there is already a suggestion and stay quiet.
        let parsed = parse_unexpected_keyword(
            "Player.__init__() got an unexpected keyword argument 'helth'. Did you mean 'health'?",
        )
        .expect("should parse");
        assert_eq!(parsed.callable, "Player");
        assert_eq!(parsed.keyword, "helth");
    }

    #[test]
    fn ignores_unrelated_messages() {
        assert!(parse_unexpected_keyword("ValueError: something else").is_none());
        assert!(parse_unexpected_keyword("").is_none());
    }

    #[test]
    fn ratio_scores_the_real_renames() {
        assert!((similarity("roughness", "perceptual_roughness") - 0.62).abs() < 0.01);
        assert!((similarity("shadows_enabled", "shadow_maps_enabled") - 0.88).abs() < 0.01);
    }

    #[test]
    fn suggests_the_real_renames() {
        assert_eq!(
            suggest_keyword("roughness", &["perceptual_roughness", "metallic"]),
            Some("perceptual_roughness")
        );
        assert_eq!(
            suggest_keyword("shadows_enabled", &["shadow_maps_enabled", "color"]),
            Some("shadow_maps_enabled")
        );
    }

    #[test]
    fn does_not_suggest_a_shared_prefix_neighbour() {
        // Bevy 0.19 has no emissive_intensity; emissive is the real target.
        assert_eq!(
            suggest_keyword(
                "emissive_intensity",
                &[
                    "emissive",
                    "emissive_texture",
                    "emissive_channel",
                    "emissive_exposure_weight"
                ],
            ),
            Some("emissive"),
        );
    }

    #[test]
    fn prefers_the_closest_of_several_containing_candidates() {
        // StandardMaterial really does offer all four of these.
        let candidates = [
            "clearcoat_perceptual_roughness",
            "metallic_roughness_channel",
            "metallic_roughness_texture",
            "perceptual_roughness",
        ];
        assert_eq!(
            suggest_keyword("roughness", &candidates),
            Some("perceptual_roughness")
        );
    }

    #[test]
    fn stays_silent_when_nothing_is_close() {
        // RectLight has no shadow parameter at all; guessing would mislead.
        assert_eq!(
            suggest_keyword(
                "shadow_maps_enabled",
                &["color", "intensity", "range", "width", "height"]
            ),
            None
        );
    }

    #[test]
    fn lists_keywords_for_a_small_callable_with_no_match() {
        let hint = unexpected_keyword_hint(
            "RectLight",
            "shadow_maps_enabled",
            &["color", "intensity", "range", "width", "height"],
        );
        assert_eq!(
            hint.as_deref(),
            Some("Valid keyword arguments for RectLight(): color, intensity, range, width, height")
        );
    }

    #[test]
    fn offers_relatives_for_a_near_miss_below_the_cutoff() {
        // `rughness` scores 0.57 against `perceptual_roughness`, under the
        // cutoff, but shares `ughness` with several real keywords.
        let candidates: Vec<&str> = (0..20)
            .map(|i| ["perceptual_roughness", "metallic_roughness_channel"][i % 2])
            .chain(std::iter::once("filler"))
            .collect();
        let hint = unexpected_keyword_hint("StandardMaterial", "rughness", &candidates)
            .expect("should hint");
        assert!(hint.contains("perceptual_roughness"), "{hint}");
    }

    #[test]
    fn points_at_help_for_a_large_callable_with_no_match() {
        let many: Vec<String> = (0..20).map(|i| format!("field_{i}")).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let hint = unexpected_keyword_hint("StandardMaterial", "zzz_nothing_like_this", &refs)
            .expect("should hint");
        assert!(hint.contains("20 keyword arguments"), "{hint}");
        assert!(hint.contains("help(StandardMaterial)"), "{hint}");
    }

    #[test]
    fn recognises_every_tier_it_can_emit() {
        // Each tier must be detectable, or a second hint gets appended when
        // raise-time enrichment and the system hook both fire.
        let many: Vec<String> = (0..20).map(|i| format!("field_{i}")).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let tiers = [
            unexpected_keyword_hint("X", "perceptua_roughness", &["perceptual_roughness"]),
            unexpected_keyword_hint("X", "zzz", &["color", "intensity"]),
            unexpected_keyword_hint("StandardMaterial", "rughness", &{
                let mut v = refs.clone();
                v.push("perceptual_roughness");
                v
            }),
            unexpected_keyword_hint("X", "zzz_nothing_like_this", &refs),
        ];
        for tier in tiers {
            let text = tier.expect("tier should produce a hint");
            assert!(has_kwarg_hint(&text), "not recognised: {text}");
        }
        assert!(!has_kwarg_hint("plain TypeError with no hint"));
    }

    #[test]
    fn hint_is_none_without_candidates() {
        assert_eq!(unexpected_keyword_hint("X", "y", &[]), None);
    }

    #[test]
    fn plugin_not_a_plugin_message_contains_class_name() {
        let msg = plugin_not_a_plugin("MyClass", "MyClass -> object");
        assert!(msg.contains("MyClass"));
        assert!(msg.contains("Inheritance chain: MyClass -> object"));
        assert!(msg.contains("class MyClass(Plugin):"));
    }

    #[test]
    fn plugin_missing_decorator_message_contains_class_name() {
        let msg = plugin_missing_decorator("AudioPlugin");
        assert!(msg.contains("AudioPlugin"));
        assert!(msg.contains("@plugin"));
        assert!(msg.contains("class AudioPlugin(Plugin):"));
    }

    #[test]
    fn plugin_error_messages_with_special_characters() {
        let msg = plugin_not_a_plugin("My_Plugin_123", "base");
        assert!(msg.contains("My_Plugin_123"));
    }

    #[test]
    fn stable_error_wording() {
        assert_eq!(
            invalid_asset_type("<class 'bool'>"),
            "Invalid asset type. Expected a subclass of `Asset`, but got `<class 'bool'>`"
        );
        assert_eq!(entity_does_not_exist(7), "Entity 7 does not exist");
    }

    const REJECTION_KINDS: [SystemParamRejection; 9] = [
        SystemParamRejection::MissingAnnotation,
        SystemParamRejection::BareResource,
        SystemParamRejection::MessageType,
        SystemParamRejection::NativeMessageType,
        SystemParamRejection::ComponentType,
        SystemParamRejection::AssetType,
        SystemParamRejection::NakedMut,
        SystemParamRejection::NakedMutAssets,
        SystemParamRejection::Unsupported,
    ];

    const WRAPPERS: [SystemParamWrapper; 4] = [
        SystemParamWrapper::Bare,
        SystemParamWrapper::Mut,
        SystemParamWrapper::Res,
        SystemParamWrapper::ResMut,
    ];

    fn rejection(wrapper: SystemParamWrapper, kind: SystemParamRejection) -> String {
        system_param_rejection_message("fly", "clicks", wrapper, "Ping", kind)
    }

    #[test]
    fn every_rejection_kind_names_the_system_and_parameter_and_is_distinct() {
        let mut rendered: Vec<String> = REJECTION_KINDS
            .iter()
            .map(|kind| rejection(SystemParamWrapper::Bare, *kind))
            .collect();
        for message in &rendered {
            assert!(
                message.starts_with("System function `fly` parameter `clicks`"),
                "{message}"
            );
        }
        rendered.sort();
        let count = rendered.len();
        rendered.dedup();
        assert_eq!(rendered.len(), count);
    }

    #[test]
    fn wrapper_spelling_reaches_the_remedy_kinds_only() {
        for kind in [
            SystemParamRejection::MessageType,
            SystemParamRejection::NativeMessageType,
            SystemParamRejection::ComponentType,
            SystemParamRejection::AssetType,
            SystemParamRejection::NakedMut,
            SystemParamRejection::NakedMutAssets,
            SystemParamRejection::Unsupported,
        ] {
            assert!(rejection(SystemParamWrapper::Res, kind).contains("`Res[Ping]`"));
            assert!(rejection(SystemParamWrapper::ResMut, kind).contains("`ResMut[Ping]`"));
            assert!(rejection(SystemParamWrapper::Mut, kind).contains("`Mut[Ping]`"));
            assert!(rejection(SystemParamWrapper::Bare, kind).contains("`Ping`"));
        }
    }

    #[test]
    fn annotation_free_kinds_ignore_the_wrapper_spelling() {
        for kind in [
            SystemParamRejection::MissingAnnotation,
            SystemParamRejection::BareResource,
        ] {
            let baseline = rejection(SystemParamWrapper::Bare, kind);
            for wrapper in WRAPPERS {
                assert_eq!(rejection(wrapper, kind), baseline);
            }
        }
    }

    #[test]
    fn each_remedy_names_its_own_valid_spellings() {
        let message = rejection(SystemParamWrapper::Bare, SystemParamRejection::MessageType);
        assert!(message.contains("MessageReader[Ping]"));
        assert!(message.contains("MessageWriter[Ping]"));
        assert!(message.contains("MessageMutator[Ping]"));

        let component = rejection(
            SystemParamWrapper::Bare,
            SystemParamRejection::ComponentType,
        );
        assert!(component.contains("Query[Ping]"));
        assert!(component.contains("Query[Mut[Ping]]"));
        assert!(component.contains("Single[Ping]"));

        let asset = rejection(SystemParamWrapper::Bare, SystemParamRejection::AssetType);
        assert!(asset.contains("Res[Assets[Ping]]"));
        assert!(asset.contains("ResMut[Assets[Ping]]"));

        let naked = rejection(SystemParamWrapper::Mut, SystemParamRejection::NakedMut);
        assert!(naked.contains("Query[Mut[Ping]]"));
        assert!(naked.contains("Single[Mut[Ping]]"));
        assert!(naked.contains("View[Mut[Ping]]"));

        let resource = rejection(SystemParamWrapper::Bare, SystemParamRejection::BareResource);
        assert!(resource.contains("Res[Ping]"));
        assert!(resource.contains("ResMut[Ping]"));

        let unsupported = rejection(SystemParamWrapper::Bare, SystemParamRejection::Unsupported);
        assert!(unsupported.contains(SYSTEM_PARAM_KINDS));
    }
}
