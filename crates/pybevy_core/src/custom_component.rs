//! Backend-agnostic custom component registration.
//!
//! This module provides shared infrastructure for registering Python-defined
//! `@component` classes as Bevy components. Both PyO3 and RustPython backends
//! use these functions, providing their own implementations of the
//! [`PythonObjectDescriptor`] trait for backend-specific Python object storage.

use bevy::ecs::{
    component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
    world::World,
};

use super::{component_layout::ComponentStorageType, component_wrapper::WrapperSize};

/// Creates a Bevy [`ComponentDescriptor`] for a wrapper-stored custom component.
///
/// Wrapper components store primitive field data as contiguous byte arrays
/// (`ComponentWrapper8/16/32/...`), enabling View API and Numba integration.
/// Each Python component class gets its own [`ComponentId`] even if multiple
/// classes use the same wrapper size.
pub fn create_wrapper_descriptor(name: String, wrapper_size: WrapperSize) -> ComponentDescriptor {
    let layout = wrapper_size.mem_layout();
    // SAFETY: Layout matches one of the ComponentWrapper* types which are all
    // Copy + Default (no drop needed), and the layout is correct for the wrapper size.
    unsafe {
        ComponentDescriptor::new_with_layout(
            name,
            StorageType::Table,
            layout,
            None, // No drop function needed for Copy types
            true, // Mutable - wrapper components can be modified
            ComponentCloneBehavior::Default,
            None,
        )
    }
}

/// Trait for backend-specific Python object storage descriptors.
///
/// When a custom component uses PyObject storage (for non-primitive field types
/// like lists, dicts, or custom classes), the backend needs to provide a
/// [`ComponentDescriptor`] with the correct layout and drop function for its
/// Python object representation.
///
/// - **PyO3**: Stores `Py<PyAny>` and drops via `OwningPtr::drop_as::<Py<PyAny>>()`
/// - **RustPython**: Would store its own object type with its own drop logic
pub trait PythonObjectDescriptor {
    /// Create a [`ComponentDescriptor`] for storing Python objects in the ECS.
    fn create(name: String) -> ComponentDescriptor;
}

/// Register a custom component with Bevy's ECS using pre-computed storage type.
///
/// This is the backend-agnostic registration function. The caller determines
/// the storage type (wrapper vs pyobject) using backend-specific introspection,
/// then passes it here. For PyObject storage, a backend-specific descriptor
/// is created via the [`PythonObjectDescriptor`] trait.
///
/// # Type Parameters
/// * `D` - The backend's [`PythonObjectDescriptor`] implementation
///
/// # Returns
/// The [`ComponentId`] assigned by Bevy for this component
pub fn register_custom_component_descriptor<D: PythonObjectDescriptor>(
    world: &mut World,
    name: String,
    storage_type: ComponentStorageType,
) -> ComponentId {
    let descriptor = match storage_type {
        ComponentStorageType::Wrapper(wrapper_size) => {
            create_wrapper_descriptor(name, wrapper_size)
        }
        ComponentStorageType::PyObject => D::create(name),
    };
    world.register_component_with_descriptor(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_wrapper_descriptor_w8() {
        let desc = create_wrapper_descriptor("TestW8".to_string(), WrapperSize::W8);
        assert_eq!(desc.storage_type(), StorageType::Table);
    }

    #[test]
    fn test_create_wrapper_descriptor_w32() {
        let desc = create_wrapper_descriptor("TestW32".to_string(), WrapperSize::W32);
        assert_eq!(desc.storage_type(), StorageType::Table);
    }

    #[test]
    fn test_create_wrapper_descriptor_w1024() {
        let desc = create_wrapper_descriptor("TestW1024".to_string(), WrapperSize::W1024);
        assert_eq!(desc.storage_type(), StorageType::Table);
    }

    /// Dummy descriptor for testing the generic registration function
    struct TestObjectDescriptor;

    impl PythonObjectDescriptor for TestObjectDescriptor {
        fn create(name: String) -> ComponentDescriptor {
            // Use a simple u64 layout for testing
            unsafe {
                ComponentDescriptor::new_with_layout(
                    name,
                    StorageType::Table,
                    std::alloc::Layout::new::<u64>(),
                    None,
                    false,
                    ComponentCloneBehavior::Default,
                    None,
                )
            }
        }
    }

    #[test]
    fn test_register_wrapper_component() {
        let mut world = World::new();
        let id = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "WrapperComp".to_string(),
            ComponentStorageType::Wrapper(WrapperSize::W16),
        );
        // Should get a valid ComponentId
        assert!(id.index() > 0);
    }

    #[test]
    fn test_register_pyobject_component() {
        let mut world = World::new();
        let id = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "PyObjComp".to_string(),
            ComponentStorageType::PyObject,
        );
        assert!(id.index() > 0);
    }

    #[test]
    fn test_register_two_components_different_ids() {
        let mut world = World::new();
        let id1 = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "CompA".to_string(),
            ComponentStorageType::Wrapper(WrapperSize::W8),
        );
        let id2 = register_custom_component_descriptor::<TestObjectDescriptor>(
            &mut world,
            "CompB".to_string(),
            ComponentStorageType::Wrapper(WrapperSize::W8),
        );
        // Same wrapper size but different names → different ComponentIds
        assert_ne!(id1, id2);
    }
}
