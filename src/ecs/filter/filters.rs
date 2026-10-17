use std::sync::Arc;

use pybevy_core::public_error::{
    OR_FILTER_EMPTY, OR_FILTER_ITEM_REQUIRED, OR_FILTER_TUPLE_REQUIRED,
};
use pyo3::{IntoPyObjectExt, PyTraverseError, PyVisit, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::{
    component_type::{PyComponentType, clone_retained_classes, retain_custom_classes},
    filter::{QueryFilter, parse_multi_component_filter, parse_single_component_filter},
    query::{query_helpers::parse_anyof_items, query_param::AnyOfItem},
};

/// Strong class references an escaped filter descriptor owns.
///
/// Each holder's increfs are traversed by exactly that holder; clones get
/// their own increfs through `clone_retained_classes` (docs/safety.md
/// section 6).
type RetainedClasses = SmallVec<[Arc<Py<PyType>>; 4]>;

#[pyclass(name = "With", module = "pybevy.ecs", from_py_object)]
#[derive(Debug)]
pub struct PyWith {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
    /// Keeps every class in `values` alive while this descriptor is reachable.
    pub(crate) retained: RetainedClasses,
}

impl Clone for PyWith {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            values: self.values.clone(),
            retained: clone_retained_classes(py, &self.retained),
        })
    }
}

/// Retention is derived from `values`, so it carries no identity of its own.
impl PartialEq for PyWith {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl PyWith {
    pub(crate) fn new(py: Python<'_>, values: SmallVec<[PyComponentType; 4]>) -> Self {
        Self {
            retained: retain_custom_classes(py, values.iter().cloned()),
            values,
        }
    }

    /// Raw-only copy for holders whose owner retains the classes separately.
    pub(crate) fn detached(&self) -> Self {
        Self {
            values: self.values.clone(),
            retained: SmallVec::new(),
        }
    }
}

#[pymethods]
impl PyWith {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "With")?;
        PyWith::new(py, values).into_py_any(py)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }
}

#[pyclass(name = "Without", module = "pybevy.ecs", from_py_object)]
#[derive(Debug)]
pub struct PyWithout {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
    /// Keeps every class in `values` alive while this descriptor is reachable.
    pub(crate) retained: RetainedClasses,
}

impl Clone for PyWithout {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            values: self.values.clone(),
            retained: clone_retained_classes(py, &self.retained),
        })
    }
}

/// Retention is derived from `values`, so it carries no identity of its own.
impl PartialEq for PyWithout {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl PyWithout {
    pub(crate) fn new(py: Python<'_>, values: SmallVec<[PyComponentType; 4]>) -> Self {
        Self {
            retained: retain_custom_classes(py, values.iter().cloned()),
            values,
        }
    }

    /// Raw-only copy for holders whose owner retains the classes separately.
    pub(crate) fn detached(&self) -> Self {
        Self {
            values: self.values.clone(),
            retained: SmallVec::new(),
        }
    }
}

#[pymethods]
impl PyWithout {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "Without")?;
        PyWithout::new(py, values).into_py_any(py)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }
}

#[pyclass(name = "Changed", module = "pybevy.ecs", from_py_object)]
#[derive(Debug)]
pub struct PyChanged {
    pub(crate) component_type: PyComponentType,
    /// Keeps `component_type` alive while this descriptor is reachable.
    pub(crate) retained: RetainedClasses,
}

impl Clone for PyChanged {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            component_type: self.component_type,
            retained: clone_retained_classes(py, &self.retained),
        })
    }
}

/// Retention is derived from `component_type`, so it carries no identity of its own.
impl PartialEq for PyChanged {
    fn eq(&self, other: &Self) -> bool {
        self.component_type == other.component_type
    }
}

impl PyChanged {
    pub(crate) fn new(py: Python<'_>, component_type: PyComponentType) -> Self {
        Self {
            retained: retain_custom_classes(py, [component_type]),
            component_type,
        }
    }

    /// Raw-only copy for holders whose owner retains the classes separately.
    pub(crate) fn detached(&self) -> Self {
        Self {
            component_type: self.component_type,
            retained: SmallVec::new(),
        }
    }
}

#[pymethods]
impl PyChanged {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        PyChanged::new(py, component_type).into_py_any(py)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }
}

#[pyclass(name = "Added", module = "pybevy.ecs", from_py_object)]
#[derive(Debug)]
pub struct PyAdded {
    pub(crate) component_type: PyComponentType,
    /// Keeps `component_type` alive while this descriptor is reachable.
    pub(crate) retained: RetainedClasses,
}

impl Clone for PyAdded {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            component_type: self.component_type,
            retained: clone_retained_classes(py, &self.retained),
        })
    }
}

/// Retention is derived from `component_type`, so it carries no identity of its own.
impl PartialEq for PyAdded {
    fn eq(&self, other: &Self) -> bool {
        self.component_type == other.component_type
    }
}

impl PyAdded {
    pub(crate) fn new(py: Python<'_>, component_type: PyComponentType) -> Self {
        Self {
            retained: retain_custom_classes(py, [component_type]),
            component_type,
        }
    }

    /// Raw-only copy for holders whose owner retains the classes separately.
    pub(crate) fn detached(&self) -> Self {
        Self {
            component_type: self.component_type,
            retained: SmallVec::new(),
        }
    }
}

#[pymethods]
impl PyAdded {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        PyAdded::new(py, component_type).into_py_any(py)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }
}

#[pyclass(name = "Has", module = "pybevy.ecs", from_py_object)]
#[derive(Debug)]
pub struct PyHas {
    pub(crate) component_type: PyComponentType,
    /// Keeps `component_type` alive while this descriptor is reachable.
    pub(crate) retained: RetainedClasses,
}

impl Clone for PyHas {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            component_type: self.component_type,
            retained: clone_retained_classes(py, &self.retained),
        })
    }
}

/// Retention is derived from `component_type`, so it carries no identity of its own.
impl PartialEq for PyHas {
    fn eq(&self, other: &Self) -> bool {
        self.component_type == other.component_type
    }
}

impl PyHas {
    pub(crate) fn new(py: Python<'_>, component_type: PyComponentType) -> Self {
        Self {
            retained: retain_custom_classes(py, [component_type]),
            component_type,
        }
    }
}

#[pymethods]
impl PyHas {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        PyHas::new(py, component_type).into_py_any(py)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }
}

#[pyclass(name = "AnyOf", module = "pybevy.ecs", from_py_object)]
#[derive(Debug)]
pub struct PyAnyOf {
    pub(crate) items: SmallVec<[AnyOfItem; 4]>,
    /// Keeps every class in `items` alive while this descriptor is reachable.
    pub(crate) retained: RetainedClasses,
}

impl Clone for PyAnyOf {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            items: self.items.clone(),
            retained: clone_retained_classes(py, &self.retained),
        })
    }
}

/// Retention is derived from `items`, so it carries no identity of its own.
impl PartialEq for PyAnyOf {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl PyAnyOf {
    pub(crate) fn new(py: Python<'_>, items: SmallVec<[AnyOfItem; 4]>) -> Self {
        Self {
            retained: retain_custom_classes(py, items.iter().map(|item| item.ty)),
            items,
        }
    }
}

#[pymethods]
impl PyAnyOf {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let items = parse_anyof_items(py, key)?;
        PyAnyOf::new(py, items).into_py_any(py)
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }
}

#[pyclass(name = "Or", module = "pybevy.ecs", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyOr {
    pub(crate) values: Vec<QueryFilter>,
}

#[pymethods]
impl PyOr {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let origin = key
            .getattr("__origin__")
            .map_err(|_| pyo3::exceptions::PyTypeError::new_err(OR_FILTER_TUPLE_REQUIRED))?;
        if !origin.is(py.get_type::<pyo3::types::PyTuple>()) {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                OR_FILTER_TUPLE_REQUIRED,
            ));
        }
        let args = key.getattr("__args__")?;
        let mut values = Vec::new();
        for value in args.try_iter()? {
            let value = value?;
            let filter = if value.is_instance_of::<PyWith>() {
                QueryFilter::With(value.extract()?)
            } else if value.is_instance_of::<PyWithout>() {
                QueryFilter::Without(value.extract()?)
            } else if value.is_instance_of::<PyChanged>() {
                QueryFilter::Changed(value.extract()?)
            } else if value.is_instance_of::<PyAdded>() {
                QueryFilter::Added(value.extract()?)
            } else if value.is_instance_of::<PyOr>() {
                values.extend(value.extract::<PyOr>()?.values);
                continue;
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    OR_FILTER_ITEM_REQUIRED,
                ));
            };
            values.push(filter);
        }
        if values.is_empty() {
            return Err(pyo3::exceptions::PyTypeError::new_err(OR_FILTER_EMPTY));
        }
        PyOr { values }.into_py_any(py)
    }

    /// Visit the retained classes of the nested filters: each nested marker
    /// owns its increfs and this descriptor is their traversing owner.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for filter in &self.values {
            for class in filter.retained_classes() {
                visit.call(class.as_ref())?;
            }
        }
        Ok(())
    }
}
