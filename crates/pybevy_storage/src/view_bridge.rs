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

use bevy::ecs::{component::ComponentId, storage::Column, world::World};

/// Primitive field types supported by the View API and bytecode VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldType {
    F32,
    F64,
    I32,
    I64,
    U32,
    U64,
    Bool,
    Vec2,
    Vec3,
    Vec4,
}

impl FieldType {
    /// Size in bytes of this field type.
    pub const fn size_bytes(&self) -> usize {
        match self {
            FieldType::F32 => 4,
            FieldType::F64 => 8,
            FieldType::I32 => 4,
            FieldType::I64 => 8,
            FieldType::U32 => 4,
            FieldType::U64 => 8,
            FieldType::Bool => 1,
            FieldType::Vec2 => 8,
            FieldType::Vec3 => 12,
            FieldType::Vec4 => 16,
        }
    }

    /// NumPy dtype string for this field type (e.g., "f4" for F32, "u1" for Bool).
    pub const fn to_numpy_dtype_str(self) -> &'static str {
        match self {
            FieldType::F32 => "f4",
            FieldType::F64 => "f8",
            FieldType::I32 => "i4",
            FieldType::I64 => "i8",
            FieldType::U32 => "u4",
            FieldType::U64 => "u8",
            FieldType::Bool => "u1",
            FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => "f4",
        }
    }
}

/// Field metadata for View API access.
///
/// Contains the byte offset and type of a field within a component struct.
/// Used for direct memory access in the View API bytecode VM.
#[derive(Debug, Clone, Copy)]
pub struct FieldOffset {
    /// Byte offset of the field within the component struct
    pub offset: usize,
    /// Type of the field
    pub field_type: FieldType,
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
///             "intensity" => Some(FieldOffset { offset: offset_of!(PointLight, intensity), field_type: FieldType::F32 }),
///             "range" => Some(FieldOffset { offset: offset_of!(PointLight, range), field_type: FieldType::F32 }),
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
    pub component_id: fn(world: &mut World) -> ComponentId,

    /// Column data pointer function
    ///
    /// Gets a raw pointer to the component data in a Column.
    /// This allows Dynamic components to provide column access for View API
    /// without requiring the main crate to know the concrete type.
    ///
    /// # Safety
    ///
    /// `entity_count` must equal the column's actual length.
    /// The returned pointer is valid only for the lifetime of the column.
    /// Caller must ensure proper synchronization for mutable access.
    pub column_data_ptr: unsafe fn(column: &Column, entity_count: usize) -> *const u8,
}

impl std::fmt::Debug for ViewBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewBridge")
            .field("field_names", &(self.field_names)())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_sizes_match_native_types() {
        assert_eq!(FieldType::F32.size_bytes(), 4);
        assert_eq!(FieldType::F64.size_bytes(), 8);
        assert_eq!(FieldType::I32.size_bytes(), 4);
        assert_eq!(FieldType::I64.size_bytes(), 8);
        assert_eq!(FieldType::U32.size_bytes(), 4);
        assert_eq!(FieldType::U64.size_bytes(), 8);
        assert_eq!(FieldType::Bool.size_bytes(), 1);
    }

    #[test]
    fn test_vec_field_type_sizes_match_layout() {
        // Vec2 = 2 * f32 = 8 bytes, Vec3 = 3 * f32 = 12, Vec4 = 4 * f32 = 16
        assert_eq!(FieldType::Vec2.size_bytes(), 8);
        assert_eq!(FieldType::Vec3.size_bytes(), 12);
        assert_eq!(FieldType::Vec4.size_bytes(), 16);
    }
}
