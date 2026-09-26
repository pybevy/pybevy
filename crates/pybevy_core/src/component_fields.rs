//! PyO3 annotation collection for custom component metadata.

use pyo3::{
    prelude::*,
    types::{PyDict, PyType},
};

/// Every annotation the class declares or inherits, base classes first.
///
/// Order matches the constructor a dataclass generates: a base field keeps its
/// position even when a subclass re-annotates it.
pub fn declared_annotations<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyDict>> {
    let merged = PyDict::new(cls.py());
    let mro = cls.getattr("__mro__")?;
    let mut classes: Vec<Bound<'py, PyAny>> = mro.try_iter()?.collect::<PyResult<_>>()?;
    classes.reverse();

    for class in classes {
        let Some(annotations) = class.getattr_opt("__annotations__")? else {
            continue;
        };
        let annotations = annotations.cast_into::<PyDict>()?;
        for (name, hint) in annotations.iter() {
            merged.set_item(name, hint)?;
        }
    }

    let dataclasses = cls.py().import("dataclasses")?;
    if !dataclasses
        .call_method1("is_dataclass", (cls,))?
        .is_truthy()?
    {
        return Ok(merged);
    }

    let fields = PyDict::new(cls.py());
    for field in dataclasses.call_method1("fields", (cls,))?.try_iter()? {
        let name = field?.getattr("name")?;
        if let Some(annotation) = merged.get_item(&name)? {
            fields.set_item(name, annotation)?;
        }
    }
    let dataclass_fields = cls.getattr("__dataclass_fields__")?;
    for (name, annotation) in merged.iter() {
        if !dataclass_fields.contains(&name)? {
            fields.set_item(name, annotation)?;
        }
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use pyo3::{
        prelude::*,
        types::{PyDict, PyFloat, PyType},
    };

    use super::declared_annotations;

    fn class<'py>(py: Python<'py>, source: &str, name: &str) -> Bound<'py, PyType> {
        let namespace = PyDict::new(py);
        py.run(
            &CString::new(source).unwrap(),
            Some(&namespace),
            Some(&namespace),
        )
        .unwrap();
        namespace
            .get_item(name)
            .unwrap()
            .unwrap()
            .cast_into::<PyType>()
            .unwrap()
    }

    fn names(annotations: &Bound<'_, PyDict>) -> Vec<String> {
        annotations
            .keys()
            .iter()
            .map(|key| key.extract::<String>().unwrap())
            .collect()
    }

    #[test]
    fn a_subclass_reports_the_fields_it_inherits() {
        Python::attach(|py| {
            let derived = class(
                py,
                "class Base:\n    hp: float\n\nclass Derived(Base):\n    mp: float\n",
                "Derived",
            );
            let annotations = declared_annotations(&derived).unwrap();
            assert_eq!(names(&annotations), ["hp", "mp"]);
        });
    }

    #[test]
    fn a_re_annotated_field_keeps_its_inherited_position() {
        Python::attach(|py| {
            let derived = class(
                py,
                "class Base:\n    hp: float\n    armor: float\n\n\
                 class Derived(Base):\n    mp: float\n    hp: int\n",
                "Derived",
            );
            let annotations = declared_annotations(&derived).unwrap();
            assert_eq!(names(&annotations), ["hp", "armor", "mp"]);
            let hp = annotations.get_item("hp").unwrap().unwrap();
            assert_eq!(
                hp.getattr("__name__").unwrap().extract::<String>().unwrap(),
                "int"
            );
        });
    }

    #[test]
    fn a_class_declaring_nothing_reports_nothing() {
        Python::attach(|py| {
            let plain = class(py, "class Plain:\n    pass\n", "Plain");
            assert!(declared_annotations(&plain).unwrap().is_empty());
        });
    }

    #[test]
    fn dataclass_pseudo_fields_are_excluded_but_later_annotation_edits_are_seen() {
        Python::attach(|py| {
            let component = class(
                py,
                r#"from dataclasses import InitVar, dataclass
from typing import ClassVar

@dataclass
class Component:
    limit: ClassVar[int] = 10
    value: int = 1
    scale: InitVar[int] = 2
"#,
                "Component",
            );
            assert_eq!(names(&declared_annotations(&component).unwrap()), ["value"]);

            component
                .getattr("__annotations__")
                .unwrap()
                .set_item("extra", py.get_type::<PyFloat>())
                .unwrap();
            assert_eq!(
                names(&declared_annotations(&component).unwrap()),
                ["value", "extra"]
            );
        });
    }
}
