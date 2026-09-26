use pybevy_storage::StorageError;
use pyo3::{IntoPyObjectExt, prelude::*, pyclass::CompareOp, types::PyList};

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

#[macro_export]
macro_rules! impl_live_scalar_list {
    ($py_name:ident, $py_class_name:literal, $collection:ty, $elem:ty) => {
        impl $crate::FromBorrowedStorage<$crate::FieldStorage<$collection>> for $py_name {
            fn from_borrowed(storage: $crate::FieldStorage<$collection>) -> Self {
                Self::from_storage(storage)
            }
        }

        #[pyo3::pymethods]
        impl $py_name {
            fn __len__(&self) -> PyResult<usize> {
                Ok(self.storage.as_ref()?.len())
            }

            fn __getitem__(&self, index: isize) -> PyResult<$elem> {
                let values = self.storage.as_ref()?;
                let index = $crate::live_sequence::normalize_index(index, values.len())?;
                Ok(values[index])
            }

            fn __setitem__(&mut self, index: isize, value: $elem) -> PyResult<()> {
                let index =
                    $crate::live_sequence::normalize_index(index, self.storage.as_ref()?.len())?;
                self.storage.as_mut()?[index] = value;
                Ok(())
            }

            fn __delitem__(&mut self, index: isize) -> PyResult<()> {
                let len = self.storage.as_ref()?.len();
                let index = $crate::live_sequence::normalize_index(index, len)?;
                self.validate_length_after_removal(len - 1)?;
                self.storage.as_mut()?.remove(index);
                Ok(())
            }

            fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                let items = self.to_list()?;
                Ok(PyList::new(py, items)?.call_method0("__iter__")?.unbind())
            }

            fn append(&mut self, value: $elem) -> PyResult<()> {
                self.storage.as_mut()?.push(value);
                Ok(())
            }

            fn extend(slf: &Bound<'_, Self>, values: &Bound<'_, PyAny>) -> PyResult<()> {
                let values = values
                    .try_iter()?
                    .map(|item| item?.extract::<$elem>())
                    .collect::<PyResult<Vec<_>>>()?;
                slf.borrow_mut().storage.as_mut()?.extend(values);
                Ok(())
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

            fn insert(&mut self, index: isize, value: $elem) -> PyResult<()> {
                let index = $crate::live_sequence::normalize_insert_index(
                    index,
                    self.storage.as_ref()?.len(),
                );
                self.storage.as_mut()?.insert(index, value);
                Ok(())
            }

            #[pyo3(signature = (index = -1))]
            fn pop(&mut self, index: isize) -> PyResult<$elem> {
                let len = self.storage.as_ref()?.len();
                if len == 0 {
                    return Err($crate::StorageError::EmptyList.into());
                }
                let index = $crate::live_sequence::normalize_index(index, len)?;
                self.validate_length_after_removal(len - 1)?;
                Ok(self.storage.as_mut()?.remove(index))
            }

            fn clear(&mut self) -> PyResult<()> {
                self.storage.as_ref()?;
                self.validate_length_after_removal(0)?;
                self.storage.as_mut()?.clear();
                Ok(())
            }

            fn to_list(&self) -> PyResult<Vec<$elem>> {
                Ok(self.storage.as_ref()?.iter().copied().collect())
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
        impl $crate::FromBorrowedStorage<$crate::FieldStorage<$collection>> for $py_name {
            fn from_borrowed(storage: $crate::FieldStorage<$collection>) -> Self {
                Self { storage }
            }
        }

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

        #[pyo3::pymethods]
        impl $py_name {
            fn __len__(&self) -> PyResult<usize> {
                Ok(self.storage.as_ref()?.len())
            }

            fn __getitem__(&self, index: isize) -> PyResult<$py_elem> {
                self.item(index)
            }

            fn __setitem__(&mut self, index: isize, value: $py_elem) -> PyResult<()> {
                let value = <$native_elem>::try_from(value)?;
                let index =
                    $crate::live_sequence::normalize_index(index, self.storage.as_ref()?.len())?;
                self.storage.as_mut()?[index] = value;
                Ok(())
            }

            fn __delitem__(&mut self, index: isize) -> PyResult<()> {
                let index =
                    $crate::live_sequence::normalize_index(index, self.storage.as_ref()?.len())?;
                self.storage.as_mut()?.remove(index);
                Ok(())
            }

            fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                let items = (0..self.__len__()?)
                    .map(|index| Py::new(py, self.item(index as isize)?))
                    .collect::<PyResult<Vec<_>>>()?;
                Ok(PyList::new(py, items)?.call_method0("__iter__")?.unbind())
            }

            fn append(&mut self, value: $py_elem) -> PyResult<()> {
                let value = <$native_elem>::try_from(value)?;
                self.storage.as_mut()?.push(value);
                Ok(())
            }

            fn extend(slf: &Bound<'_, Self>, values: &Bound<'_, PyAny>) -> PyResult<()> {
                let values = values
                    .try_iter()?
                    .map(|item| <$native_elem>::try_from(item?.extract::<$py_elem>()?))
                    .collect::<PyResult<Vec<_>>>()?;
                slf.borrow_mut().storage.as_mut()?.extend(values);
                Ok(())
            }

            fn __richcmp__(
                &self,
                other: &Bound<'_, PyAny>,
                op: pyo3::pyclass::CompareOp,
            ) -> PyResult<Py<PyAny>> {
                let py = other.py();
                let items = self
                    .to_list()?
                    .into_iter()
                    .map(|item| Py::new(py, item))
                    .collect::<PyResult<Vec<_>>>()?;
                let items = PyList::new(py, items)?;
                $crate::live_sequence::compare_live_sequence(
                    py,
                    &items,
                    other,
                    op,
                    other.is_instance_of::<Self>(),
                )
            }

            fn insert(&mut self, index: isize, value: $py_elem) -> PyResult<()> {
                let value = <$native_elem>::try_from(value)?;
                let index = $crate::live_sequence::normalize_insert_index(
                    index,
                    self.storage.as_ref()?.len(),
                );
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
                Ok(self.storage.as_mut()?.remove(index).into())
            }

            fn clear(&mut self) -> PyResult<()> {
                self.storage.as_mut()?.clear();
                Ok(())
            }

            fn to_list(&self) -> PyResult<Vec<$py_elem>> {
                Ok(self
                    .storage
                    .as_ref()?
                    .iter()
                    .cloned()
                    .map(Into::into)
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
