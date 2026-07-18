//! Shared, backend-neutral messages for operations that fall outside the bounded
//! `pybevy.array` surface. Kept identical across the PyO3 and
//! RustPython adapters so users see the same guidance on both backends.

/// Accessing an attribute/method the bounded array does not implement.
pub(crate) fn unsupported_attr(name: &str) -> String {
    format!(
        "'{name}' is not part of pybevy.array's bounded surface. \
         Call .to_numpy() (CPython) or .copy() to obtain a full array, then use \
         NumPy directly. See docs/array.md."
    )
}

/// An operator the bounded array does not support (e.g. `@` matrix multiply).
pub(crate) fn unsupported_op(op: &str) -> String {
    format!(
        "{op} is not supported on pybevy.array arrays. \
         Call .to_numpy() (CPython) or .copy() first, then use NumPy directly. \
         See docs/array.md."
    )
}

/// Assigning a Python list/tuple to an index (a numpy-only idiom).
pub(crate) fn unsupported_list_assign() -> String {
    "Assigning a Python list to a pybevy.array index is not supported. Assign \
     elementwise (arr[0, 0] = 1.0) or with a matching-shape pybevy.array Array."
        .to_string()
}
