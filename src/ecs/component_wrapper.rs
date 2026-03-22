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
