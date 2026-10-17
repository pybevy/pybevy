use pybevy_storage::{FieldStorage, StorageError};
use pyo3::{
    IntoPyObjectExt, exceptions::PyValueError, prelude::*, pyclass::CompareOp, types::PyList,
};

use crate::public_error;

pub fn compare_live_sequence(
    py: Python<'_>,
    items: &Bound<'_, PyList>,
    other: &Bound<'_, PyAny>,
    op: CompareOp,
    same_type: bool,
) -> PyResult<Py<PyAny>> {
    if !matches!(op, CompareOp::Eq | CompareOp::Ne) {
        return Ok(py.NotImplemented());
    }
    let sequence = py.import("collections.abc")?.getattr("Sequence")?;
    if !same_type && !other.is_instance(&sequence)? {
        return Ok(py.NotImplemented());
    }
    let other_items = PyList::new(py, other.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
    let equal = items.eq(&other_items)?;
    (if matches!(op, CompareOp::Eq) {
        equal
    } else {
        !equal
    })
    .into_py_any(py)
}

/// Normalize a Python sequence index, including negative indexing.
pub fn normalize_index(index: isize, len: usize) -> Result<usize, StorageError> {
    let index = if index < 0 {
        let index = len as isize + index;
        if index < 0 {
            return Err(StorageError::IndexOutOfRange);
        }
        index as usize
    } else {
        index as usize
    };

    if index >= len {
        return Err(StorageError::IndexOutOfRange);
    }
    Ok(index)
}

/// Normalize an insertion index with Python list semantics.
pub fn normalize_insert_index(index: isize, len: usize) -> usize {
    if index < 0 {
        (len as isize + index).max(0) as usize
    } else {
        (index as usize).min(len)
    }
}

/// The error for a length change on a fixed-length live collection.
///
/// Expired and read-only storage report their access error first, checked
/// without resolving mutable storage, so a rejected edit never marks the
/// owner changed.
pub fn resize_error<C: Clone>(storage: &FieldStorage<C>, name: &str) -> PyErr {
    match storage
        .as_ref()
        .map(drop)
        .and_then(|()| storage.check_write_access())
    {
        Err(error) => error.into(),
        Ok(()) => PyValueError::new_err(public_error::fixed_length_collection(name)),
    }
}

/// Live list over a `Vec` of copy scalars, with `from_storage` and
/// `validate_resize` hooks on the wrapper.
#[macro_export]
macro_rules! impl_live_scalar_list {
    ($py_name:ident, $py_class_name:literal, $collection:ty, $elem:ty) => {
        impl $crate::FromBorrowedStorage<$crate::FieldStorage<$collection>> for $py_name {
            fn from_borrowed(storage: $crate::FieldStorage<$collection>) -> Self {
                Self::from_storage(storage)
            }
        }

        $crate::__live_sequence!(@convert $py_name, $elem, $elem);
        $crate::__live_sequence!(@value_item $py_name, $elem);
        $crate::__live_sequence!(@read $py_name, $py_class_name, $elem);
        $crate::__live_sequence!(@resizable $py_name, $elem, $elem, validate_resize);
    };
}

/// Live list over a `Vec` of immutable values, returned by value on every read.
///
/// `extract = path` converts an incoming element with
/// `fn(&Bound<'_, PyAny>) -> PyResult<Native>` for unions such as `impl Into<T>`.
#[macro_export]
macro_rules! impl_live_value_list {
    (
        $py_name:ident,
        $py_class_name:literal,
        $collection:ty,
        $native_elem:ty,
        $py_elem:ty
    ) => {
        $crate::__live_sequence!(@from_borrowed $py_name, $collection);
        $crate::__live_sequence!(@convert $py_name, $native_elem, $py_elem);
        $crate::__live_sequence!(@value_item $py_name, $py_elem);
        $crate::__live_sequence!(@read $py_name, $py_class_name, $py_elem);
        $crate::__live_sequence!(@resizable $py_name, $py_elem, $py_elem);
    };
    (
        $py_name:ident,
        $py_class_name:literal,
        $collection:ty,
        $native_elem:ty,
        $py_elem:ty,
        extract = $extract:path
    ) => {
        $crate::__live_sequence!(@from_borrowed $py_name, $collection);
        $crate::__live_sequence!(@extract $py_name, $native_elem, $extract);
        $crate::__live_sequence!(@value_item $py_name, $py_elem);
        $crate::__live_sequence!(@read $py_name, $py_class_name, $py_elem);
        $crate::__live_sequence!(
            @resizable $py_name, $py_elem, pyo3::Bound<'_, pyo3::PyAny>
        );
    };
}

/// Live fixed-length array of immutable values, such as `[HalfSpace; 6]`.
///
/// Items are assignable by index; every resize raises `ValueError` naming
/// `$field`.
#[macro_export]
macro_rules! impl_live_value_array {
    (
        $py_name:ident,
        $py_class_name:literal,
        $collection:ty,
        $native_elem:ty,
        $py_elem:ty,
        fixed = $field:literal
    ) => {
        $crate::__live_sequence!(@from_borrowed $py_name, $collection);
        $crate::__live_sequence!(@convert $py_name, $native_elem, $py_elem);
        $crate::__live_sequence!(@value_item $py_name, $py_elem);
        $crate::__live_sequence!(@read $py_name, $py_class_name, $py_elem);
        $crate::__live_sequence!(@fixed $py_name, $py_elem, $field);
    };
}

/// Live list over a `Vec` of nested value or field wrappers; each item borrows
/// its element through a checked index path.
#[macro_export]
macro_rules! impl_live_field_list {
    (
        $py_name:ident,
        $py_class_name:literal,
        $collection:ty,
        $native_elem:ty,
        $py_elem:ty,
        $elem_storage:ty
    ) => {
        $crate::__live_sequence!(@from_borrowed $py_name, $collection);
        $crate::__live_sequence!(@convert $py_name, $native_elem, $py_elem);
        $crate::__live_sequence!(@field_item $py_name, $native_elem, $py_elem, $elem_storage);
        $crate::__live_sequence!(@read $py_name, $py_class_name, $py_elem);
        $crate::__live_sequence!(@resizable $py_name, $py_elem, $py_elem);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __live_sequence {
    (@from_borrowed $py_name:ident, $collection:ty) => {
        impl $crate::FromBorrowedStorage<$crate::FieldStorage<$collection>> for $py_name {
            fn from_borrowed(storage: $crate::FieldStorage<$collection>) -> Self {
                Self { storage }
            }
        }
    };

    (@convert $py_name:ident, $native_elem:ty, $py_elem:ty) => {
        impl $py_name {
            // The `?` converts an infallible `From` as well as a fallible `TryFrom`.
            #[allow(clippy::needless_question_mark)]
            fn live_element(value: $py_elem) -> PyResult<$native_elem> {
                Ok(<$native_elem>::try_from(value)?)
            }
        }
    };

    (@extract $py_name:ident, $native_elem:ty, $extract:path) => {
        impl $py_name {
            fn live_element(value: pyo3::Bound<'_, pyo3::PyAny>) -> PyResult<$native_elem> {
                $extract(&value)
            }
        }
    };

    (@value_item $py_name:ident, $py_elem:ty) => {
        impl $py_name {
            #[allow(clippy::clone_on_copy)]
            fn item(&self, index: isize) -> PyResult<$py_elem> {
                let values = self.storage.as_ref()?;
                let index = $crate::live_sequence::normalize_index(index, values.len())?;
                Ok(<$py_elem>::from(values[index].clone()))
            }
        }
    };

    (@field_item $py_name:ident, $native_elem:ty, $py_elem:ty, $elem_storage:ty) => {
        impl $py_name {
            fn item(&self, index: isize) -> PyResult<$py_elem> {
                let index =
                    $crate::live_sequence::normalize_index(index, self.storage.as_ref()?.len())?;
                Ok(self
                    .storage
                    .borrow_resolved_index_as::<$native_elem, $elem_storage, $py_elem>(
                        index,
                        |values, index| values.get(index),
                        |values, index| values.get_mut(index),
                    )?)
            }
        }
    };

    (@read $py_name:ident, $py_class_name:literal, $py_elem:ty) => {
        #[pyo3::pymethods]
        impl $py_name {
            fn __len__(&self) -> PyResult<usize> {
                Ok(self.storage.as_ref()?.len())
            }

            fn __getitem__(&self, index: isize) -> PyResult<$py_elem> {
                self.item(index)
            }

            fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                let items = (0..self.__len__()?)
                    .map(|index| {
                        pyo3::IntoPyObjectExt::into_py_any(self.item(index as isize)?, py)
                    })
                    .collect::<PyResult<Vec<_>>>()?;
                Ok(PyList::new(py, items)?.call_method0("__iter__")?.unbind())
            }

            fn __richcmp__(
                &self,
                other: &Bound<'_, PyAny>,
                op: pyo3::pyclass::CompareOp,
            ) -> PyResult<Py<PyAny>> {
                let py = other.py();
                let items = PyList::new(py, self.to_list()?)?;
                $crate::live_sequence::compare_live_sequence(
                    py,
                    &items,
                    other,
                    op,
                    other.is_instance_of::<Self>(),
                )
            }

            /// Detached owned copies of the current elements.
            fn to_list(&self) -> PyResult<Vec<$py_elem>> {
                Ok(self
                    .storage
                    .as_ref()?
                    .iter()
                    .cloned()
                    .map(<$py_elem>::from)
                    .collect())
            }

            fn __repr__(&self) -> PyResult<String> {
                Ok(format!(
                    concat!($py_class_name, "(len={})"),
                    self.__len__()?
                ))
            }
        }
    };

    (@setitem $py_name:ident, $arg:ty) => {
        #[pyo3::pymethods]
        impl $py_name {
            fn __setitem__(&mut self, index: isize, value: $arg) -> PyResult<()> {
                let value = Self::live_element(value)?;
                let index =
                    $crate::live_sequence::normalize_index(index, self.storage.as_ref()?.len())?;
                self.storage.as_mut()?[index] = value;
                Ok(())
            }
        }
    };

    (@resizable $py_name:ident, $py_elem:ty, $arg:ty $(, $validate:ident)?) => {
        $crate::__live_sequence!(@setitem $py_name, $arg);

        impl $py_name {
            #[allow(unreachable_code)]
            fn live_resize(&self, _length: usize) -> PyResult<()> {
                $(return self.$validate(_length);)?
                Ok(())
            }
        }

        #[pyo3::pymethods]
        impl $py_name {
            fn __delitem__(&mut self, index: isize) -> PyResult<()> {
                let len = self.storage.as_ref()?.len();
                let index = $crate::live_sequence::normalize_index(index, len)?;
                self.live_resize(len - 1)?;
                self.storage.as_mut()?.remove(index);
                Ok(())
            }

            fn append(&mut self, value: $arg) -> PyResult<()> {
                let value = Self::live_element(value)?;
                self.live_resize(self.storage.as_ref()?.len() + 1)?;
                self.storage.as_mut()?.push(value);
                Ok(())
            }

            fn extend(slf: &Bound<'_, Self>, values: &Bound<'_, PyAny>) -> PyResult<()> {
                let values = values
                    .try_iter()?
                    .map(|item| Self::live_element(item?.extract::<$arg>()?))
                    .collect::<PyResult<Vec<_>>>()?;
                let mut list = slf.borrow_mut();
                list.live_resize(list.storage.as_ref()?.len() + values.len())?;
                list.storage.as_mut()?.extend(values);
                Ok(())
            }

            fn insert(&mut self, index: isize, value: $arg) -> PyResult<()> {
                let value = Self::live_element(value)?;
                let len = self.storage.as_ref()?.len();
                self.live_resize(len + 1)?;
                let index = $crate::live_sequence::normalize_insert_index(index, len);
                self.storage.as_mut()?.insert(index, value);
                Ok(())
            }

            #[pyo3(signature = (index = -1))]
            fn pop(&mut self, index: isize) -> PyResult<$py_elem> {
                let len = self.storage.as_ref()?.len();
                if len == 0 {
                    return Err($crate::StorageError::EmptyList.into());
                }
                let index = $crate::live_sequence::normalize_index(index, len)?;
                self.live_resize(len - 1)?;
                Ok(<$py_elem>::from(self.storage.as_mut()?.remove(index)))
            }

            fn clear(&mut self) -> PyResult<()> {
                self.storage.as_ref()?;
                self.live_resize(0)?;
                self.storage.as_mut()?.clear();
                Ok(())
            }
        }
    };

    (@fixed $py_name:ident, $py_elem:ty, $field:literal) => {
        $crate::__live_sequence!(@setitem $py_name, $py_elem);
        #[pyo3::pymethods]
        impl $py_name {
            fn __delitem__(&self, index: isize) -> PyResult<()> {
                let _ = index;
                Err($crate::live_sequence::resize_error(&self.storage, $field))
            }

            fn append(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
                let _ = value;
                Err($crate::live_sequence::resize_error(&self.storage, $field))
            }

            fn extend(&self, values: &Bound<'_, PyAny>) -> PyResult<()> {
                let _ = values;
                Err($crate::live_sequence::resize_error(&self.storage, $field))
            }

            fn insert(&self, index: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
                let _ = (index, value);
                Err($crate::live_sequence::resize_error(&self.storage, $field))
            }

            #[pyo3(signature = (index = -1))]
            fn pop(&self, index: isize) -> PyResult<$py_elem> {
                let _ = index;
                Err($crate::live_sequence::resize_error(&self.storage, $field))
            }

            fn clear(&self) -> PyResult<()> {
                Err($crate::live_sequence::resize_error(&self.storage, $field))
            }
        }
    };
}

#[macro_export]
macro_rules! impl_live_asset_sequence {
    ($py_name:ident, $py_class_name:literal, $collection:ty, $native_elem:ty, $py_elem:ty) => {
        impl $crate::FromBorrowedStorage<$crate::FieldStorage<$collection>> for $py_name {
            fn from_borrowed(storage: $crate::FieldStorage<$collection>) -> Self {
                Self { storage }
            }
        }

        impl $py_name {
            fn item_storage(&self, index: isize) -> PyResult<$crate::AssetStorage<$native_elem>> {
                let index =
                    $crate::live_sequence::normalize_index(index, self.storage.as_ref()?.len())?;
                Ok(self.storage.borrow_resolved_asset_index(
                    index,
                    |values, index| values.get(index),
                    |values, index| values.get_mut(index),
                )?)
            }
        }

        #[pyo3::pymethods]
        impl $py_name {
            fn __len__(&self) -> PyResult<usize> {
                Ok(self.storage.as_ref()?.len())
            }

            fn __getitem__(&self, index: isize, py: Python<'_>) -> PyResult<Py<$py_elem>> {
                Py::new(py, <$py_elem>::from_borrowed(self.item_storage(index)?))
            }

            fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                let items = (0..self.__len__()?)
                    .map(|index| self.__getitem__(index as isize, py))
                    .collect::<PyResult<Vec<_>>>()?;
                Ok(PyList::new(py, items)?.call_method0("__iter__")?.unbind())
            }

            fn to_list(&self, py: Python<'_>) -> PyResult<Vec<Py<$py_elem>>> {
                let values = self.storage.as_ref()?.iter().cloned().collect::<Vec<_>>();
                values
                    .into_iter()
                    .map(|value| Py::new(py, <$py_elem>::from_owned(value)))
                    .collect()
            }

            fn __repr__(&self) -> PyResult<String> {
                Ok(format!(
                    concat!($py_class_name, "(len={})"),
                    self.__len__()?
                ))
            }
        }
    };
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use pybevy_storage::StorageError;

    use super::*;

    #[test]
    fn normalize_index_supports_python_negative_indexing() {
        assert_eq!(normalize_index(0, 3).unwrap(), 0);
        assert_eq!(normalize_index(2, 3).unwrap(), 2);
        assert_eq!(normalize_index(-1, 3).unwrap(), 2);
        assert_eq!(normalize_index(-3, 3).unwrap(), 0);
    }

    #[test]
    fn normalize_index_rejects_out_of_range() {
        assert!(matches!(
            normalize_index(3, 3),
            Err(StorageError::IndexOutOfRange)
        ));
        assert!(matches!(
            normalize_index(-4, 3),
            Err(StorageError::IndexOutOfRange)
        ));
        assert!(matches!(
            normalize_index(0, 0),
            Err(StorageError::IndexOutOfRange)
        ));
    }

    #[test]
    fn normalize_insert_index_clamps_like_a_python_list() {
        assert_eq!(normalize_insert_index(-5, 3), 0);
        assert_eq!(normalize_insert_index(-1, 3), 2);
        assert_eq!(normalize_insert_index(0, 3), 0);
        assert_eq!(normalize_insert_index(3, 3), 3);
        assert_eq!(normalize_insert_index(99, 3), 3);
    }
}
