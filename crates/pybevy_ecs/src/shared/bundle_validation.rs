//! Interpreter-neutral component-bundle validation.

use std::{collections::HashMap, hash::Hash};

/// Return the first component identity that appears twice.
///
/// Backend adapters resolve their native or custom component types into an
/// interpreter-free key before calling this function.
pub fn first_duplicate_indices<K>(keys: impl IntoIterator<Item = K>) -> Option<(usize, usize)>
where
    K: Eq + Hash,
{
    let mut seen = HashMap::new();
    for (index, key) in keys.into_iter().enumerate() {
        if let Some(first) = seen.insert(key, index) {
            return Some((first, index));
        }
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
}
