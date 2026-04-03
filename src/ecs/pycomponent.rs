//! Generic component storage supporting both owned and borrowed instances
//!
//! This module re-exports the unified storage mechanism from pybevy_core.
//! See pybevy_core's pycomponent module for implementation details.
//!
//! # Safety Model
//!
//! PyBevy components exist in two modes:
//!
//! ## 1. Owned Components (Python-created)
//!
//! Created via Python constructors like `Transform()`, these components are:
//! - **Stored in**: `Box<T>` on the heap (stable memory address)
//! - **Validity tracked by**: `ValidityFlag` (Arc<AtomicU8>)
//! - **Valid when**: Before spawn/insert or drop
//! - **Invalidated by**: `Drop` implementation when component is consumed or goes out of scope
//!
//! ### Key Safety Guarantee: Field Borrow Invalidation
//!
//! When you access a field like `bloom.prefilter`, you get a **borrowed reference** into the
//! Box, not a clone. This enables field mutations to persist:
//!
//! ```python
//! bloom = Bloom()
//! bloom.prefilter.threshold = 32.0  # Borrows field from Box
//! assert bloom.prefilter.threshold == 32.0  # ✅ Mutation persisted
//! ```
//!
//! **Safety mechanism**: The field borrow shares the component's `ValidityFlag`. When the
//! component is dropped or consumed (via spawn/insert), the Drop impl invalidates the flag,
//! making all outstanding field borrows fail their validity checks:
//!
//! ```python
//! bloom = Bloom()
//! prefilter = bloom.prefilter  # Borrows into Box
//! del bloom                     # Drop invalidates validity flag
//! prefilter.threshold = 10.0    # ❌ RuntimeError: component accessed outside system
//! ```
//!
//! This prevents use-after-free by ensuring field borrows cannot outlive their parent component.
//!
//! ## 2. Borrowed Components (ECS-stored)
//!
//! Retrieved via `Query` iteration, these components are:
//! - **Stored in**: Bevy's ECS component storage (archetype tables)
//! - **Validity tracked by**: `ValidityFlagWithMode` (Arc<AtomicU8> + AccessMode)
//! - **Valid when**: During system execution only
//! - **Invalidated by**: `ValidityGuard` RAII when system completes
//!
//! The `ValidityFlagWithMode` additionally tracks whether the component was accessed via
//! `Query[T]` (Read) or `Query[Mut[T]]` (Write), enforcing Rust's borrow rules at runtime:
//!
//! ```python
//! def system(query: Query[Transform]):  # Read-only query
//!     for transform in query:
//!         transform.translation.x = 10.0  # ❌ RuntimeError: cannot modify read-only component
//! ```
//!
//! ## Clone Safety
//!
//! **CRITICAL**: Each cloned owned component gets a **NEW** `ValidityFlag`:
//!
//! ```rust,ignore
//! ComponentStorageInner::Owned { data, validity: _ } => {  // Ignore old validity
//!     Self {
//!         inner: ComponentStorageInner::Owned {
//!             data: Box::new((**data).clone()),
//!             validity: ValidityFlag::new_write(),  // NEW flag for independence
//!         },
//!     }
//! }
//! ```
//!
//! This prevents false invalidation where dropping one clone would invalidate another.

// Re-export from pybevy_core - the single source of truth for storage types
#[allow(unused_imports)]
pub use pybevy_core::ComponentStorage;
