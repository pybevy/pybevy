//! View API bridge trait for runtime type dispatch
//!
//! This module provides the `ViewBridge` struct and related traits that allow feature crates
//! to register their View API support without the core crate needing to import bytecode VM types.
//!
//! # Architecture
//!
//! Feature crates implement `ViewFieldAccess` trait and provide a `ViewBridge` when registering
//! their component bridges. The main crate uses these function pointers for View API operations.
//!
//! # Safety Model
//!
//! The function pointers in `ViewBridge` are:
//! - Static functions (no lifetime concerns)
//! - Send + Sync (raw function pointers)
//! - Called synchronously with proper validity tracking
//!
//! Field offsets are compile-time verified via `offset_of!` macro.

/// Field metadata for View API access
///
/// Contains the byte offset and type metadata of a field within a component struct.
/// Used for direct memory access in the View API bytecode VM.
#[derive(Debug, Clone, Copy)]
pub struct FieldOffset {
    /// Byte offset of the field within the component struct
    pub offset: usize,
    /// Size in bytes of a single element (4 for f32, 1 for bool, etc.)
    pub element_size: usize,
    /// View API dtype string (e.g., "f4" for f32, "u1" for bool)
    pub dtype: &'static str,
}

/// Trait for components that support View API field access
///
/// This trait provides compile-time verified field offsets for the View API.
/// Typically implemented using a macro that generates lookups from `offset_of!`.
///
/// # Example
///
/// ```ignore
/// impl ViewFieldAccess for PyPointLight {
///     fn field_offset(field_name: &str) -> Option<FieldOffset> {
///         match field_name {
///             "intensity" => Some(FieldOffset { offset: offset_of!(PointLight, intensity), element_size: 4, dtype: "f4" }),
///             "range" => Some(FieldOffset { offset: offset_of!(PointLight, range), element_size: 4, dtype: "f4" }),
///             _ => None,
///         }
///     }
///
///     fn field_names() -> &'static [&'static str] {
///         &["intensity", "range"]
///     }
/// }
/// ```
pub trait ViewFieldAccess {
    /// Get the byte offset for a field by name
    ///
    /// Returns `None` if the field doesn't exist or isn't accessible via View API.
    fn field_offset(field_name: &str) -> Option<FieldOffset>;

    /// List all field names that support View API access
    ///
    /// Used for generating helpful error messages when a field is not found.
    fn field_names() -> &'static [&'static str];
}

/// Bridge struct containing function pointers for View API operations
///
/// This allows feature crates to provide View API support without the core
/// crate needing to import component-specific types.
///
/// # Function Pointer Safety
///
/// All function pointers are to static functions and are:
/// - Send + Sync safe (raw function pointers)
/// - Lifetime-safe (no references captured)
/// - Thread-safe (called with proper synchronization)
#[derive(Clone, Copy)]
pub struct ViewBridge {
    /// Field offset lookup function
    ///
    /// Returns the byte offset of a field within the component struct.
    pub field_offset: fn(field_name: &str) -> Option<FieldOffset>,

    /// Field names function for error messages
    ///
    /// Returns a list of all fields that support View API access.
    pub field_names: fn() -> &'static [&'static str],

    /// Component ID registration function
    ///
    /// This is called during View query building to register the component
    /// with the world and get its ComponentId.
    pub component_id: fn(world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId,

    /// Column data pointer function
    ///
    /// Gets a raw pointer to the component data in a Column.
    /// This allows Dynamic components to provide column access for View API
    /// without requiring the main crate to know the concrete type.
    ///
    /// # Safety
    ///
    /// The returned pointer is valid only for the lifetime of the column.
    /// Caller must ensure proper synchronization for mutable access.
    pub column_data_ptr: fn(column: &bevy::ecs::storage::Column, entity_count: usize) -> *const u8,
}

impl std::fmt::Debug for ViewBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewBridge")
            .field("field_names", &(self.field_names)())
            .finish()
    }
}
