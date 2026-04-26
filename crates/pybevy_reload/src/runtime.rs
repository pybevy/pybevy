use std::{any::TypeId, collections::HashSet};

use bevy::ecs::world::World;

/// Error type for reload operations.
/// Wraps the error message and whether it's a load failure (old systems keep running).
#[allow(dead_code)]
pub struct ReloadError {
    pub message: String,
    pub is_load_failure: bool,
}

impl std::fmt::Display for ReloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Trait for reload operations that depend on the Python runtime.
///
/// The orchestrator (`perform_reload`) handles shared logic:
/// generation tracking, entity cleanup, asset clearing, stats, rollback.
/// This trait handles everything that touches the interpreter.
///
/// The associated type `Defs` is opaque to the orchestrator —
/// it only passes it back to trait methods.
pub trait ReloadRuntime {
    /// Opaque container for loaded definitions.
    type Defs;

    /// Associated type for system handles returned by register_systems.
    /// PyO3 impl uses DynamicSystemHandle; other backends use their own types.
    type SystemHandle: Send + 'static;

    /// Load definitions from the loader function.
    /// Called BEFORE generation increment — on failure, old systems keep running.
    fn load_definitions(&mut self, generation: u32) -> Result<Self::Defs, ReloadError>;

    /// Check if pending definitions require escalation from Partial to Full.
    /// Returns the reason string if escalation is needed.
    fn requires_escalation(&self, defs: &Self::Defs) -> Option<&'static str>;

    /// Extract plugin names from pending definitions (for delta detection).
    fn plugin_names(&self, defs: &Self::Defs) -> Vec<String>;

    /// Extract system names from pending definitions (for delta detection).
    fn system_names(&self, defs: &Self::Defs) -> std::collections::HashSet<String>;

    /// Register loaded systems into Bevy schedules with the given generation.
    fn register_systems(
        &mut self,
        world: &mut World,
        defs: Self::Defs,
        generation: u32,
    ) -> Result<Vec<Self::SystemHandle>, ReloadError>;

    /// Insert resources into the world (Full reload only).
    fn register_resources(
        &mut self,
        world: &mut World,
        defs: &Self::Defs,
    ) -> Result<(), ReloadError>;

    /// Re-register message types with updated class pointers.
    fn register_messages(
        &mut self,
        world: &mut World,
        defs: &Self::Defs,
        generation: u32,
    ) -> Result<(), ReloadError>;

    /// Clear old observers and register new ones (Full reload only).
    fn register_observers(
        &mut self,
        world: &mut World,
        defs: &Self::Defs,
    ) -> Result<(), ReloadError>;

    /// Register system handles for a generation and gut old-generation systems.
    /// Called by the orchestrator after register_systems succeeds.
    fn register_handles(
        &mut self,
        world: &mut World,
        generation: u32,
        handles: Vec<Self::SystemHandle>,
    );

    /// Prune old-generation message registrations.
    fn prune_messages(&mut self, world: &mut World, keep_after_generation: u32);

    /// Clear custom runtime resources from the world (Full reload only).
    fn clear_custom_resources(&mut self, world: &mut World, verbose: bool);

    /// Collect TypeIds of bridged native resources currently in the world.
    /// Called once before first user code to capture the Bevy-plugin baseline.
    fn snapshot_native_resources(&self, world: &World) -> HashSet<TypeId>;

    /// Reset/remove bridged native resources based on the initial snapshot.
    /// Initial resources get `reset_to_default()`, user-only resources get `remove()`.
    fn clear_native_resources(&self, world: &mut World, initial: &HashSet<TypeId>, verbose: bool);

    /// Detect removed/renamed systems by comparing new system names against the known set.
    /// Returns names of systems that were removed.
    fn detect_system_delta(
        &mut self,
        world: &mut World,
        new_systems: std::collections::HashSet<String>,
    ) -> Vec<String>;

    /// Clear system parameter cache.
    fn clear_param_cache(&mut self);

    /// Trigger garbage collection in the runtime.
    fn trigger_gc(&mut self);

    /// Get runtime GC tracked object count (for overlay stats).
    fn gc_object_count(&self) -> usize {
        0
    }

    /// Print an error to stderr using the runtime's error formatting.
    fn print_error(&self, error: &ReloadError);
}
