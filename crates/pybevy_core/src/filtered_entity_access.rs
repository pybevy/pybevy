//! Unified entity access for read-only and read-write queries.
//!
//! Wraps both `FilteredEntityRef` (read-only) and `FilteredEntityMut` (read-write)
//! behind a single enum so that extraction code (`ExtractFn`, `ComponentBridge::extract`)
//! works unchanged for both query types.

use bevy::ecs::{
    change_detection::MutUntyped,
    component::ComponentId,
    entity::Entity,
    ptr::Ptr,
    world::{FilteredEntityMut, FilteredEntityRef},
};

/// Unified access to either a read-only or read-write filtered entity.
///
/// Used by `ExtractFn` and `ComponentBridge::extract()` so that the same extraction
/// code path works for both `QueryState<FilteredEntityRef>` (read-only) and
/// `QueryState<FilteredEntityMut>` (read-write) queries.
pub enum FilteredEntityAccess<'w, 's> {
    /// Read-only access — query has no `Mut[T]` components.
    Ref(FilteredEntityRef<'w, 's>),
    /// Read-write access — query has at least one `Mut[T]` component.
    Mut(FilteredEntityMut<'w, 's>),
}

impl<'w, 's> FilteredEntityAccess<'w, 's> {
    /// Returns the entity ID.
    #[inline(always)]
    pub fn id(&self) -> Entity {
        match self {
            Self::Ref(r) => r.id(),
            Self::Mut(m) => m.id(),
        }
    }

    /// Read-only component access by ID. Available on both variants.
    #[inline(always)]
    pub fn get_by_id(&self, component_id: ComponentId) -> Option<Ptr<'_>> {
        match self {
            Self::Ref(r) => r.get_by_id(component_id),
            Self::Mut(m) => m.get_by_id(component_id),
        }
    }

    /// Mutable component access by ID. Only available on the `Mut` variant.
    ///
    /// # Panics
    ///
    /// Panics if called on the `Ref` variant. This should never happen because
    /// read-only queries guarantee `AccessMode::Read` for every component.
    #[inline(always)]
    pub fn get_mut_by_id(&mut self, component_id: ComponentId) -> Option<MutUntyped<'_>> {
        match self {
            Self::Ref(_) => panic!(
                "get_mut_by_id called on FilteredEntityRef — \
                 read-only queries must never request mutable access"
            ),
            Self::Mut(m) => m.get_mut_by_id(component_id),
        }
    }
}

impl<'w, 's> From<FilteredEntityRef<'w, 's>> for FilteredEntityAccess<'w, 's> {
    #[inline(always)]
    fn from(r: FilteredEntityRef<'w, 's>) -> Self {
        Self::Ref(r)
    }
}

impl<'w, 's> From<FilteredEntityMut<'w, 's>> for FilteredEntityAccess<'w, 's> {
    #[inline(always)]
    fn from(m: FilteredEntityMut<'w, 's>) -> Self {
        Self::Mut(m)
    }
}
