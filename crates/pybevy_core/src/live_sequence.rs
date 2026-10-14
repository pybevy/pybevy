use pybevy_storage::{FieldStorage, StorageError};
use pyo3::{
    IntoPyObjectExt,
    exceptions::PyValueError,
    prelude::*,
    pyclass::CompareOp,
    types::{PyByteArray, PyBytes, PyList, PyString, PyTuple, PyType},
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

/// A generated live collection class, registered with `collections.abc` at import.
pub struct LiveSequenceType {
    pub type_object: for<'py> fn(Python<'py>) -> Bound<'py, PyType>,
    pub mutable: bool,
}

inventory::collect!(LiveSequenceType);

/// Register every live collection as a `Sequence`, or a `MutableSequence` when it resizes.
pub fn register_sequence_abcs(py: Python<'_>) -> PyResult<()> {
    let abc = py.import("collections.abc")?;
    for entry in inventory::iter::<LiveSequenceType> {
        let name = if entry.mutable {
            "MutableSequence"
        } else {
            "Sequence"
        };
        abc.getattr(name)?
            .call_method1("register", ((entry.type_object)(py),))?;
    }
    Ok(())
}

fn concatenable(other: &Bound<'_, PyAny>) -> PyResult<bool> {
    if other.is_instance_of::<PyList>() {
        return Ok(true);
    }
    if other.is_instance_of::<PyTuple>()
        || other.is_instance_of::<PyString>()
        || other.is_instance_of::<PyBytes>()
        || other.is_instance_of::<PyByteArray>()
    {
        return Ok(false);
    }
    let sequence = other.py().import("collections.abc")?.getattr("Sequence")?;
    other.is_instance(&sequence)
}

/// `items + other` (or `other + items` when `reflected`) as a new list; a tuple or
/// string operand is not concatenated, as with `list`.
pub fn concat_items<'py>(
    items: &Bound<'py, PyList>,
    other: &Bound<'py, PyAny>,
    reflected: bool,
) -> PyResult<Py<PyAny>> {
    let py = other.py();
    if !concatenable(other)? {
        return Ok(py.NotImplemented());
    }
    let other = PyList::new(py, other.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
    let (left, right) = if reflected {
        (other, items.clone())
    } else {
        (items.clone(), other)
    };
    Ok(left.as_any().add(right)?.unbind())
}

/// The index of the first item equal to `value`, raising `ValueError` as `list.remove` does.
pub fn remove_index(items: &Bound<'_, PyList>, value: &Bound<'_, PyAny>) -> PyResult<usize> {
    for (index, item) in items.iter().enumerate() {
        if item.eq(value)? {
            return Ok(index);
        }
    }
    Err(PyValueError::new_err(
        public_error::LIVE_LIST_REMOVE_MISSING,
    ))
}

/// The stable `list.sort` order of `items`, as source indices.
pub fn sort_order(
    items: &Bound<'_, PyList>,
    key: Option<&Bound<'_, PyAny>>,
    reverse: bool,
) -> PyResult<Vec<usize>> {
    let py = items.py();
    let keys = match key {
        Some(key) => PyList::new(
            py,
            items
                .iter()
                .map(|item| key.call1((item,)))
                .collect::<PyResult<Vec<_>>>()?,
        )?,
        None => items.clone(),
    };
    let kwargs = pyo3::types::PyDict::new(py);
    kwargs.set_item("key", keys.getattr("__getitem__")?)?;
    kwargs.set_item("reverse", reverse)?;
    py.import("builtins")?
        .getattr("sorted")?
        .call(
            (pyo3::types::PyRange::new(py, 0, items.len() as isize)?,),
            Some(&kwargs),
        )?
        .extract()
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
        $crate::__live_sequence!(@register $py_name, true);
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
        $crate::__live_sequence!(@register $py_name, true);
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
        $crate::__live_sequence!(@register $py_name, true);
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
        $crate::__live_sequence!(@register $py_name, false);
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
        $crate::__live_sequence!(@register $py_name, true);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __live_sequence {
    (@register $py_name:ident, $mutable:literal) => {
        const _: () = {
            fn live_type(py: pyo3::Python<'_>) -> pyo3::Bound<'_, pyo3::types::PyType> {
                py.get_type::<$py_name>()
            }
            $crate::inventory::submit!($crate::live_sequence::LiveSequenceType {
                type_object: live_type,
                mutable: $mutable,
            });
        };
    };

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
        impl $py_name {
            fn live_items<'py>(
                &self,
                py: Python<'py>,
            ) -> PyResult<pyo3::Bound<'py, pyo3::types::PyList>> {
                let items = (0..self.storage.as_ref()?.len())
                    .map(|index| {
                        pyo3::IntoPyObjectExt::into_py_any(self.item(index as isize)?, py)
                    })
                    .collect::<PyResult<Vec<_>>>()?;
                pyo3::types::PyList::new(py, items)
            }
        }

        #[pyo3::pymethods]
        impl $py_name {
            fn __len__(&self) -> PyResult<usize> {
                Ok(self.storage.as_ref()?.len())
            }

            fn __getitem__(&self, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                let py = index.py();
                if index.is_instance_of::<pyo3::types::PySlice>() {
                    return Ok(self.live_items(py)?.as_any().get_item(index)?.unbind());
                }
                pyo3::IntoPyObjectExt::into_py_any(self.item(index.extract()?)?, py)
            }

            fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                Ok(self.live_items(py)?.call_method0("__iter__")?.unbind())
            }

            fn __reversed__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                Ok(self.live_items(py)?.call_method0("__reversed__")?.unbind())
            }

            #[pyo3(signature = (value, start = 0, stop = isize::MAX))]
            fn index(&self, value: &Bound<'_, PyAny>, start: isize, stop: isize) -> PyResult<usize> {
                self.live_items(value.py())?
                    .call_method1("index", (value, start, stop))?
                    .extract()
            }

            fn count(&self, value: &Bound<'_, PyAny>) -> PyResult<usize> {
                self.live_items(value.py())?
                    .call_method1("count", (value,))?
                    .extract()
            }

            fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                $crate::live_sequence::concat_items(&self.live_items(other.py())?, other, false)
            }

            fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                $crate::live_sequence::concat_items(&self.live_items(other.py())?, other, true)
            }

            /// Detached owned copies, as `copy.copy` of a list gives.
            fn __copy__(&self) -> PyResult<Vec<$py_elem>> {
                self.to_list()
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

            fn __iadd__(
                mut slf: pyo3::PyRefMut<'_, Self>,
                values: &Bound<'_, PyAny>,
            ) -> PyResult<()> {
                let values = if values.as_ptr() == slf.as_ptr() {
                    slf.storage.as_ref()?.to_vec()
                } else {
                    values
                        .try_iter()?
                        .map(|item| Self::live_element(item?.extract::<$arg>()?))
                        .collect::<PyResult<Vec<_>>>()?
                };
                let len = slf.storage.as_ref()?.len();
                slf.live_resize(len + values.len())?;
                slf.storage.as_mut()?.extend(values);
                Ok(())
            }

            fn remove(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
                let index =
                    $crate::live_sequence::remove_index(&self.live_items(value.py())?, value)?;
                self.__delitem__(index as isize)
            }

            fn reverse(&mut self) -> PyResult<()> {
                self.storage.as_ref()?;
                self.storage.as_mut()?.reverse();
                Ok(())
            }

            #[pyo3(signature = (*, key = None, reverse = false))]
            fn sort(
                &mut self,
                py: Python<'_>,
                key: Option<Bound<'_, PyAny>>,
                reverse: bool,
            ) -> PyResult<()> {
                let snapshot = pyo3::types::PyList::new(py, self.to_list()?)?;
                let order = $crate::live_sequence::sort_order(&snapshot, key.as_ref(), reverse)?;
                let values = self.storage.as_ref()?.to_vec();
                if values.len() != order.len() {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        $crate::public_error::LIVE_LIST_MODIFIED_DURING_SORT,
                    ));
                }
                let mut target = self.storage.as_mut()?;
                for (slot, index) in target.iter_mut().zip(order) {
                    *slot = values[index].clone();
                }
                Ok(())
            }

            /// Detached owned copies of the current elements, as `list.copy()` gives.
            fn copy(&self) -> PyResult<Vec<$py_elem>> {
                self.to_list()
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

        impl $py_name {
            fn item(&self, index: isize, py: Python<'_>) -> PyResult<Py<$py_elem>> {
                Py::new(py, <$py_elem>::from_borrowed(self.item_storage(index)?))
            }

            fn live_items<'py>(
                &self,
                py: Python<'py>,
            ) -> PyResult<pyo3::Bound<'py, pyo3::types::PyList>> {
                let items = (0..self.storage.as_ref()?.len())
                    .map(|index| self.item(index as isize, py))
                    .collect::<PyResult<Vec<_>>>()?;
                pyo3::types::PyList::new(py, items)
            }
        }

        #[pyo3::pymethods]
        impl $py_name {
            fn __len__(&self) -> PyResult<usize> {
                Ok(self.storage.as_ref()?.len())
            }

            fn __getitem__(&self, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                let py = index.py();
                if index.is_instance_of::<pyo3::types::PySlice>() {
                    return Ok(self.live_items(py)?.as_any().get_item(index)?.unbind());
                }
                Ok(self.item(index.extract()?, py)?.into_any())
            }

            fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                Ok(self.live_items(py)?.call_method0("__iter__")?.unbind())
            }
            fn __reversed__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
                Ok(self.live_items(py)?.call_method0("__reversed__")?.unbind())
            }

            #[pyo3(signature = (value, start = 0, stop = isize::MAX))]
            fn index(&self, value: &Bound<'_, PyAny>, start: isize, stop: isize) -> PyResult<usize> {
                self.live_items(value.py())?
                    .call_method1("index", (value, start, stop))?
                    .extract()
            }

            fn count(&self, value: &Bound<'_, PyAny>) -> PyResult<usize> {
                self.live_items(value.py())?
                    .call_method1("count", (value,))?
                    .extract()
            }

            fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                $crate::live_sequence::concat_items(&self.live_items(other.py())?, other, false)
            }

            fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                $crate::live_sequence::concat_items(&self.live_items(other.py())?, other, true)
            }

            /// Detached owned copies, as `copy.copy` of a list gives.
            fn __copy__(&self, py: Python<'_>) -> PyResult<Vec<Py<$py_elem>>> {
                self.to_list(py)
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

        $crate::__live_sequence!(@register $py_name, false);
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
