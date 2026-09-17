//! Interpreter-neutral component-bundle validation.

use std::{collections::HashMap, hash::Hash};

use smallvec::SmallVec;

/// Bundles this size or smaller are checked by comparison rather than hashing.
const LINEAR_SCAN_LIMIT: usize = 16;

/// Return the first component identity that appears twice.
///
/// Backend adapters resolve their native or custom component types into an
/// interpreter-free key before calling this function.
pub fn first_duplicate_indices<K>(keys: impl IntoIterator<Item = K>) -> Option<(usize, usize)>
where
    K: Eq + Hash,
{
    let mut scanned: SmallVec<[K; LINEAR_SCAN_LIMIT]> = SmallVec::new();
    let mut spilled: Option<HashMap<K, usize>> = None;
    for (index, key) in keys.into_iter().enumerate() {
        if let Some(seen) = spilled.as_mut() {
            if let Some(first) = seen.insert(key, index) {
                return Some((first, index));
            }
            continue;
        }
        if let Some(first) = scanned.iter().position(|seen| *seen == key) {
            return Some((first, index));
        }
        if scanned.len() < LINEAR_SCAN_LIMIT {
            scanned.push(key);
            continue;
        }
        // This key is the one past the window, so the map earns its allocation.
        let mut seen = HashMap::with_capacity(LINEAR_SCAN_LIMIT * 2);
        for (first, seen_key) in scanned.drain(..).enumerate() {
            seen.insert(seen_key, first);
        }
        seen.insert(key, index);
        spilled = Some(seen);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_first_duplicate_indices() {
        assert_eq!(first_duplicate_indices([4, 7, 9, 7, 4]), Some((1, 3)));
        assert_eq!(first_duplicate_indices([4, 7, 9]), None);
    }

    #[test]
    fn scans_past_the_linear_limit() {
        let unique: Vec<usize> = (0..LINEAR_SCAN_LIMIT * 3).collect();
        assert_eq!(first_duplicate_indices(unique), None);
    }

    #[test]
    fn reports_duplicate_found_after_spilling() {
        let mut keys: Vec<usize> = (0..LINEAR_SCAN_LIMIT * 2).collect();
        keys.push(LINEAR_SCAN_LIMIT + 3);
        let last = keys.len() - 1;
        assert_eq!(
            first_duplicate_indices(keys),
            Some((LINEAR_SCAN_LIMIT + 3, last))
        );
    }

    #[test]
    fn reports_duplicate_spanning_the_spill_boundary() {
        // First occurrence sits inside the linear window, the repeat lands after
        // the switch to hashing, so the indices must survive the handover.
        let mut keys: Vec<usize> = (0..LINEAR_SCAN_LIMIT * 2).collect();
        keys.push(3);
        let last = keys.len() - 1;
        assert_eq!(first_duplicate_indices(keys), Some((3, last)));
    }

    #[test]
    fn a_full_window_of_unique_keys_stays_on_the_stack() {
        // Exactly LINEAR_SCAN_LIMIT unique keys must finish without spilling; the
        // boundary case that used to allocate a map it never read.
        let exact: Vec<usize> = (0..LINEAR_SCAN_LIMIT).collect();
        assert_eq!(first_duplicate_indices(exact), None);
    }

    #[test]
    fn reports_duplicate_of_the_last_windowed_key() {
        let mut keys: Vec<usize> = (0..LINEAR_SCAN_LIMIT).collect();
        keys.push(LINEAR_SCAN_LIMIT - 1);
        assert_eq!(
            first_duplicate_indices(keys),
            Some((LINEAR_SCAN_LIMIT - 1, LINEAR_SCAN_LIMIT))
        );
    }
}
