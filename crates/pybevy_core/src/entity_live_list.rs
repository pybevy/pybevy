//! Live Python list protocol for a `Vec<Entity>` component field.
//!
//! Backs `SkinnedMesh.joints` and `VisibleMeshEntities.entities`. Entities are
//! immutable values, so items are returned by value and assigned by index.

use bevy::ecs::entity::Entity;
use pybevy_storage::FieldStorage;
use pyo3::{prelude::*, types::PyList};

use crate::PyEntity;

#[pyclass(name = "_EntityLiveList", module = "pybevy.ecs", skip_from_py_object)]
#[derive(Clone)]
pub struct PyEntityLiveList {
    storage: FieldStorage<Vec<Entity>>,
}

crate::impl_live_value_list!(
    PyEntityLiveList,
    "_EntityLiveList",
    Vec<Entity>,
    Entity,
    PyEntity
);

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use pyo3::exceptions::PyRuntimeError;

    use super::*;
    use crate::StorageError;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).unwrap()
    }

    fn list(values: Vec<Entity>) -> PyEntityLiveList {
        PyEntityLiveList {
            storage: FieldStorage::owned(values),
        }
    }

    fn entities(list: &PyEntityLiveList) -> Vec<Entity> {
        list.to_list().unwrap().into_iter().map(|e| e.0).collect()
    }

    #[test]
    fn value_list_protocol_mutates_and_reads_back() {
        let mut joints = list(vec![entity(1), entity(2)]);
        assert_eq!(joints.__len__().unwrap(), 2);
        assert_eq!(joints.__getitem__(-1).unwrap().0, entity(2));
        assert!(joints.__getitem__(2).is_err());

        joints.__setitem__(0, PyEntity(entity(7))).unwrap();
        joints.append(PyEntity(entity(3))).unwrap();
        joints.insert(0, PyEntity(entity(9))).unwrap();
        assert_eq!(
            entities(&joints),
            vec![entity(9), entity(7), entity(2), entity(3)]
        );
        assert_eq!(joints.__repr__().unwrap(), "_EntityLiveList(len=4)");

        assert_eq!(joints.pop(-1).unwrap().0, entity(3));
        joints.__delitem__(0).unwrap();
        assert_eq!(entities(&joints), vec![entity(7), entity(2)]);
        joints.clear().unwrap();
        assert_eq!(joints.__len__().unwrap(), 0);
        assert!(joints.pop(-1).is_err());
    }

    #[test]
    fn snapshot_storage_rejects_every_write_without_mutating() {
        let mut joints = PyEntityLiveList {
            storage: <FieldStorage<Vec<Entity>> as crate::BorrowableStorage<_>>::snapshot(&vec![
                entity(1),
            ]),
        };
        Python::attach(|py| {
            let errors = [
                joints.__setitem__(0, PyEntity(entity(2))).unwrap_err(),
                joints.append(PyEntity(entity(2))).unwrap_err(),
                joints.clear().unwrap_err(),
            ];
            let expected = PyErr::from(StorageError::OwnedFieldReadOnly);
            for error in errors {
                assert!(error.is_instance_of::<PyRuntimeError>(py));
                assert_eq!(error.value(py).to_string(), expected.value(py).to_string());
            }
        });
        assert_eq!(entities(&joints), vec![entity(1)]);
    }
}
