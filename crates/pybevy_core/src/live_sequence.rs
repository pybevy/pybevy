use pybevy_storage::StorageError;

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
                Self { storage }
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
                let index =
                    $crate::live_sequence::normalize_index(index, self.storage.as_ref()?.len())?;
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

            fn extend(&mut self, values: Vec<$elem>) -> PyResult<()> {
                self.storage.as_mut()?.extend(values);
                Ok(())
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
                Ok(self.storage.as_mut()?.remove(index))
            }

            fn clear(&mut self) -> PyResult<()> {
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

            fn extend(&mut self, values: Vec<$py_elem>) -> PyResult<()> {
                let values = values
                    .into_iter()
                    .map(<$native_elem>::try_from)
                    .collect::<PyResult<Vec<_>>>()?;
                self.storage.as_mut()?.extend(values);
                Ok(())
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
