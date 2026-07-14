use bevy::ecs::component::Component;

/// Fixed-size wrapper structs for storing custom Python components.
///
/// These wrappers enable View API and Numba integration for custom components
/// with primitive-only fields by storing component data as contiguous byte arrays
/// instead of Python object pointers.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component, Default)]
pub struct ComponentWrapper8 {
    pub data: [u8; 8],
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component, Default)]
pub struct ComponentWrapper16 {
    pub data: [u8; 16],
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component, Default)]
pub struct ComponentWrapper32 {
    pub data: [u8; 32],
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component)]
pub struct ComponentWrapper64 {
    pub data: [u8; 64],
}

impl Default for ComponentWrapper64 {
    fn default() -> Self {
        Self { data: [0; 64] }
    }
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component)]
pub struct ComponentWrapper128 {
    pub data: [u8; 128],
}

impl Default for ComponentWrapper128 {
    fn default() -> Self {
        Self { data: [0; 128] }
    }
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component)]
pub struct ComponentWrapper256 {
    pub data: [u8; 256],
}

impl Default for ComponentWrapper256 {
    fn default() -> Self {
        Self { data: [0; 256] }
    }
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component)]
pub struct ComponentWrapper512 {
    pub data: [u8; 512],
}

impl Default for ComponentWrapper512 {
    fn default() -> Self {
        Self { data: [0; 512] }
    }
}

#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, Component)]
pub struct ComponentWrapper1024 {
    pub data: [u8; 1024],
}

impl Default for ComponentWrapper1024 {
    fn default() -> Self {
        Self { data: [0; 1024] }
    }
}

/// Size variants for wrapper components
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrapperSize {
    W8,
    W16,
    W32,
    W64,
    W128,
    W256,
    W512,
    W1024,
}

impl WrapperSize {
    /// Select the appropriate wrapper size for a given data size.
    /// Rounds up to the nearest power of 2.
    pub fn for_size(size: usize) -> Option<Self> {
        match size {
            0..=8 => Some(WrapperSize::W8),
            9..=16 => Some(WrapperSize::W16),
            17..=32 => Some(WrapperSize::W32),
            33..=64 => Some(WrapperSize::W64),
            65..=128 => Some(WrapperSize::W128),
            129..=256 => Some(WrapperSize::W256),
            257..=512 => Some(WrapperSize::W512),
            513..=1024 => Some(WrapperSize::W1024),
            _ => None, // Too large for wrapper storage
        }
    }

    /// Get the size in bytes of this wrapper
    pub const fn size_bytes(&self) -> usize {
        match self {
            WrapperSize::W8 => 8,
            WrapperSize::W16 => 16,
            WrapperSize::W32 => 32,
            WrapperSize::W64 => 64,
            WrapperSize::W128 => 128,
            WrapperSize::W256 => 256,
            WrapperSize::W512 => 512,
            WrapperSize::W1024 => 1024,
        }
    }

    /// Get the `std::alloc::Layout` for this wrapper size.
    /// Used when creating Bevy ComponentDescriptors for custom components.
    pub fn mem_layout(&self) -> std::alloc::Layout {
        match self {
            WrapperSize::W8 => std::alloc::Layout::new::<ComponentWrapper8>(),
            WrapperSize::W16 => std::alloc::Layout::new::<ComponentWrapper16>(),
            WrapperSize::W32 => std::alloc::Layout::new::<ComponentWrapper32>(),
            WrapperSize::W64 => std::alloc::Layout::new::<ComponentWrapper64>(),
            WrapperSize::W128 => std::alloc::Layout::new::<ComponentWrapper128>(),
            WrapperSize::W256 => std::alloc::Layout::new::<ComponentWrapper256>(),
            WrapperSize::W512 => std::alloc::Layout::new::<ComponentWrapper512>(),
            WrapperSize::W1024 => std::alloc::Layout::new::<ComponentWrapper1024>(),
        }
    }

    /// Get the alignment requirement of this wrapper
    pub const fn alignment(&self) -> usize {
        8 // All wrappers use 8-byte alignment
    }

    /// Get a mutable pointer to the wrapper's data array.
    /// This centralizes the pattern of casting MutUntyped to the correct wrapper type.
    ///
    /// # Safety
    /// The caller must ensure that `untyped` actually points to a wrapper of this size.
    pub unsafe fn get_mut_ptr(
        &self,
        untyped: &mut bevy::ecs::change_detection::MutUntyped,
    ) -> *mut u8 {
        match self {
            WrapperSize::W8 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper8>() };
                wrapper.data.as_mut_ptr()
            }
            WrapperSize::W16 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper16>() };
                wrapper.data.as_mut_ptr()
            }
            WrapperSize::W32 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper32>() };
                wrapper.data.as_mut_ptr()
            }
            WrapperSize::W64 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper64>() };
                wrapper.data.as_mut_ptr()
            }
            WrapperSize::W128 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper128>() };
                wrapper.data.as_mut_ptr()
            }
            WrapperSize::W256 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper256>() };
                wrapper.data.as_mut_ptr()
            }
            WrapperSize::W512 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper512>() };
                wrapper.data.as_mut_ptr()
            }
            WrapperSize::W1024 => {
                let wrapper = unsafe { untyped.as_mut().deref_mut::<ComponentWrapper1024>() };
                wrapper.data.as_mut_ptr()
            }
        }
    }

    /// Get a const pointer to the wrapper's data array (cast to mutable for lazy proxy).
    /// This centralizes the pattern of casting Ptr to the correct wrapper type.
    ///
    /// # Safety
    /// The caller must ensure that `untyped` actually points to a wrapper of this size.
    pub unsafe fn get_ref_ptr_as_mut(&self, untyped: bevy::ecs::ptr::Ptr) -> *mut u8 {
        match self {
            WrapperSize::W8 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper8>() };
                wrapper.data.as_ptr() as *mut u8
            }
            WrapperSize::W16 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper16>() };
                wrapper.data.as_ptr() as *mut u8
            }
            WrapperSize::W32 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper32>() };
                wrapper.data.as_ptr() as *mut u8
            }
            WrapperSize::W64 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper64>() };
                wrapper.data.as_ptr() as *mut u8
            }
            WrapperSize::W128 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper128>() };
                wrapper.data.as_ptr() as *mut u8
            }
            WrapperSize::W256 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper256>() };
                wrapper.data.as_ptr() as *mut u8
            }
            WrapperSize::W512 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper512>() };
                wrapper.data.as_ptr() as *mut u8
            }
            WrapperSize::W1024 => {
                let wrapper = unsafe { untyped.deref::<ComponentWrapper1024>() };
                wrapper.data.as_ptr() as *mut u8
            }
        }
    }

    /// Get a const pointer from a Column's data slice for this wrapper size.
    /// This centralizes the pattern of calling column.get_data_slice with the correct wrapper type.
    ///
    /// # Safety
    /// The caller must ensure that the column actually contains wrappers of this size.
    pub unsafe fn get_column_data_ptr(
        &self,
        column: &bevy::ecs::storage::Column,
        entity_count: usize,
    ) -> *const u8 {
        match self {
            WrapperSize::W8 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper8>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
            WrapperSize::W16 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper16>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
            WrapperSize::W32 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper32>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
            WrapperSize::W64 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper64>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
            WrapperSize::W128 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper128>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
            WrapperSize::W256 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper256>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
            WrapperSize::W512 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper512>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
            WrapperSize::W1024 => {
                let data_slice =
                    unsafe { column.get_data_slice::<ComponentWrapper1024>(entity_count) };
                data_slice.as_ptr() as *const u8
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrapper_size_selection() {
        assert_eq!(WrapperSize::for_size(1), Some(WrapperSize::W8));
        assert_eq!(WrapperSize::for_size(8), Some(WrapperSize::W8));
        assert_eq!(WrapperSize::for_size(9), Some(WrapperSize::W16));
        assert_eq!(WrapperSize::for_size(16), Some(WrapperSize::W16));
        assert_eq!(WrapperSize::for_size(17), Some(WrapperSize::W32));
        assert_eq!(WrapperSize::for_size(32), Some(WrapperSize::W32));
        assert_eq!(WrapperSize::for_size(33), Some(WrapperSize::W64));
        assert_eq!(WrapperSize::for_size(64), Some(WrapperSize::W64));
        assert_eq!(WrapperSize::for_size(65), Some(WrapperSize::W128));
        assert_eq!(WrapperSize::for_size(128), Some(WrapperSize::W128));
        assert_eq!(WrapperSize::for_size(129), Some(WrapperSize::W256));
        assert_eq!(WrapperSize::for_size(256), Some(WrapperSize::W256));
        assert_eq!(WrapperSize::for_size(257), Some(WrapperSize::W512));
        assert_eq!(WrapperSize::for_size(512), Some(WrapperSize::W512));
        assert_eq!(WrapperSize::for_size(513), Some(WrapperSize::W1024));
        assert_eq!(WrapperSize::for_size(1024), Some(WrapperSize::W1024));
        assert_eq!(WrapperSize::for_size(1025), None);
    }

    #[test]
    fn test_wrapper_size_bytes() {
        assert_eq!(WrapperSize::W8.size_bytes(), 8);
        assert_eq!(WrapperSize::W16.size_bytes(), 16);
        assert_eq!(WrapperSize::W32.size_bytes(), 32);
        assert_eq!(WrapperSize::W64.size_bytes(), 64);
        assert_eq!(WrapperSize::W128.size_bytes(), 128);
        assert_eq!(WrapperSize::W256.size_bytes(), 256);
        assert_eq!(WrapperSize::W512.size_bytes(), 512);
        assert_eq!(WrapperSize::W1024.size_bytes(), 1024);
    }

    #[test]
    fn test_wrapper_alignment() {
        assert_eq!(WrapperSize::W8.alignment(), 8);
        assert_eq!(WrapperSize::W16.alignment(), 8);
        assert_eq!(WrapperSize::W32.alignment(), 8);
        assert_eq!(WrapperSize::W64.alignment(), 8);
        assert_eq!(WrapperSize::W128.alignment(), 8);
        assert_eq!(WrapperSize::W256.alignment(), 8);
        assert_eq!(WrapperSize::W512.alignment(), 8);
        assert_eq!(WrapperSize::W1024.alignment(), 8);
    }

    #[test]
    fn test_wrapper_size_for_zero() {
        assert_eq!(WrapperSize::for_size(0), Some(WrapperSize::W8));
    }

    #[test]
    fn test_wrapper_size_boundary_values() {
        // Exact boundary values
        assert_eq!(WrapperSize::for_size(8), Some(WrapperSize::W8));
        assert_eq!(WrapperSize::for_size(9), Some(WrapperSize::W16));
        assert_eq!(WrapperSize::for_size(16), Some(WrapperSize::W16));
        assert_eq!(WrapperSize::for_size(17), Some(WrapperSize::W32));
        assert_eq!(WrapperSize::for_size(1024), Some(WrapperSize::W1024));
        assert_eq!(WrapperSize::for_size(1025), None);
    }

    #[test]
    fn test_wrapper_size_eq_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(WrapperSize::W8);
        set.insert(WrapperSize::W16);
        set.insert(WrapperSize::W8); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_wrapper_size_debug() {
        assert_eq!(format!("{:?}", WrapperSize::W8), "W8");
        assert_eq!(format!("{:?}", WrapperSize::W1024), "W1024");
    }

    #[test]
    fn test_wrapper_default_zeroed() {
        let w8 = ComponentWrapper8::default();
        assert_eq!(w8.data, [0u8; 8]);

        let w64 = ComponentWrapper64::default();
        assert_eq!(w64.data, [0u8; 64]);

        let w128 = ComponentWrapper128::default();
        assert_eq!(w128.data, [0u8; 128]);

        let w256 = ComponentWrapper256::default();
        assert_eq!(w256.data, [0u8; 256]);

        let w512 = ComponentWrapper512::default();
        assert_eq!(w512.data, [0u8; 512]);

        let w1024 = ComponentWrapper1024::default();
        assert_eq!(w1024.data, [0u8; 1024]);
    }

    #[test]
    fn test_wrapper_struct_sizes() {
        use std::mem::{align_of, size_of};

        assert_eq!(size_of::<ComponentWrapper8>(), 8);
        assert_eq!(align_of::<ComponentWrapper8>(), 8);

        assert_eq!(size_of::<ComponentWrapper16>(), 16);
        assert_eq!(align_of::<ComponentWrapper16>(), 8);

        assert_eq!(size_of::<ComponentWrapper32>(), 32);
        assert_eq!(align_of::<ComponentWrapper32>(), 8);

        assert_eq!(size_of::<ComponentWrapper64>(), 64);
        assert_eq!(align_of::<ComponentWrapper64>(), 8);

        assert_eq!(size_of::<ComponentWrapper128>(), 128);
        assert_eq!(align_of::<ComponentWrapper128>(), 8);

        assert_eq!(size_of::<ComponentWrapper256>(), 256);
        assert_eq!(align_of::<ComponentWrapper256>(), 8);

        assert_eq!(size_of::<ComponentWrapper512>(), 512);
        assert_eq!(align_of::<ComponentWrapper512>(), 8);

        assert_eq!(size_of::<ComponentWrapper1024>(), 1024);
        assert_eq!(align_of::<ComponentWrapper1024>(), 8);
    }
}
