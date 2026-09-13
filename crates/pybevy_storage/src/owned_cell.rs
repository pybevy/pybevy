//! Heap cell for Python-owned values that escaped sub-objects alias
//!
//! Owned component storage lends a sub-object a pointer into the value it owns
//! (`AnimationPlayer.play()` returning an `ActiveAnimation`) while the parent
//! wrapper keeps reading and writing the same value. Parent and child therefore
//! alias one allocation for as long as the shared [`ValidityFlag`] lives.
//!
//! [`ValidityFlag`]: crate::ValidityFlag

use std::{cell::UnsafeCell, fmt};

/// A heap-allocated `T` whose address may be lent out mutably through `&self`.
///
/// Every handle - the owner's own references included - derives from
/// [`UnsafeCell::get`], so a lent pointer carries write permission and survives
/// the owner's later reads and writes. A pointer cast from `&T` would not:
/// its provenance is read-only, and writing through it is undefined behavior
/// under both Stacked Borrows and Tree Borrows however the lifetime works out.
pub struct OwnedCell<T> {
    data: Box<UnsafeCell<T>>,
}

// SAFETY: the cell owns its `T` outright and `Box` keeps its address across the
// move; every alias into it is a raw pointer gated by a thread-affine
// `ValidityFlag`.
unsafe impl<T: Send> Send for OwnedCell<T> {}
// SAFETY: shared access hands out `&T` and pointers whose dereference is gated
// by a thread-affine `ValidityFlag`, which rejects every thread except the one
// that activated it (see `validity_guard`).
unsafe impl<T: Sync> Sync for OwnedCell<T> {}

impl<T> OwnedCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            data: Box::new(UnsafeCell::new(value)),
        }
    }

    #[inline(always)]
    pub(crate) fn get(&self) -> &T {
        // SAFETY: `&self` bounds this reference, and a lent pointer is only
        // dereferenced from the Python wrapper that holds it, never while this
        // reference is alive on the same thread.
        unsafe { &*self.data.get() }
    }

    #[inline(always)]
    pub(crate) fn get_mut(&mut self) -> &mut T {
        // SAFETY: `&mut self` bounds this reference and excludes every other
        // reference derived from this handle; lent raw pointers stay usable
        // because this one derives through the cell, and their dereference is
        // gated by the shared `ValidityFlag`.
        unsafe { &mut *self.data.get() }
    }

    #[inline(always)]
    pub(crate) fn as_ptr(&self) -> *const T {
        self.data.get().cast_const()
    }

    /// The cell's address with write permission, for a borrow that shares it.
    #[inline(always)]
    pub(crate) fn as_mut_ptr(&self) -> *mut T {
        self.data.get()
    }
}

impl<T: fmt::Debug> fmt::Debug for OwnedCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::OwnedCell;

    #[test]
    fn lent_pointer_writes_reach_the_owner() {
        let cell = OwnedCell::new(1_i32);
        let ptr = cell.as_mut_ptr();

        // SAFETY: the cell outlives the pointer and nothing else aliases it here.
        unsafe { *ptr = 42 };

        assert_eq!(*cell.get(), 42);
    }

    #[test]
    fn owner_and_lent_pointer_write_alternately() {
        let mut cell = OwnedCell::new(0_i32);
        let ptr = cell.as_mut_ptr();

        // SAFETY: as above; each access is finished before the next begins.
        unsafe { *ptr = 1 };
        assert_eq!(*cell.get(), 1);
        *cell.get_mut() = 2;
        // SAFETY: the owner's reference above is dead, so this pointer is the
        // only live handle; deriving both from the cell keeps it valid.
        unsafe { *ptr += 3 };
        assert_eq!(*cell.get(), 5);
    }

    #[test]
    fn read_only_pointer_sees_owner_writes() {
        let mut cell = OwnedCell::new(7_i32);
        let ptr = cell.as_ptr();

        *cell.get_mut() = 9;

        // SAFETY: the owner's reference is dead and the cell is still alive.
        assert_eq!(unsafe { *ptr }, 9);
    }

    #[test]
    fn distinct_cells_do_not_alias_each_other() {
        let first = OwnedCell::new(1_i32);
        let second = OwnedCell::new(2_i32);

        assert_ne!(first.as_ptr(), second.as_ptr());

        // SAFETY: separate allocations; nothing aliases the second cell here.
        unsafe { *second.as_mut_ptr() = 9 };

        assert_eq!(*first.get(), 1);
        assert_eq!(*second.get(), 9);
    }
}
