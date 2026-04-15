use pyo3::prelude::*;

pub mod batch_spawn;
pub mod commands;
pub(crate) mod component;
pub mod component_layout;
pub mod component_type;
pub mod component_wrapper;
pub mod conditional_system;
pub mod custom_batch;
pub mod custom_component;
pub mod disabled;
pub mod dynamic_condition;
pub mod dynamic_system;
pub mod entity_commands;
pub mod filter;
pub mod helpers;
pub mod lazy_wrapper_proxy;

// Re-exports from pybevy_core
#[allow(unused_imports)]
pub use pybevy_core::{PyChildOf, PyChildren, PyChildrenIterator, PyEntity};
#[allow(unused_imports)]
pub use pybevy_core::{PyF32List, field_storage::FieldStorage, value_storage::ValueStorage};
pub mod local;
pub mod message;
pub mod messages;
pub mod mutable;
// Name moved to pybevy_ecs crate
pub mod observer;
pub mod observer_registry;
pub mod query;
pub mod resource;
pub mod resource_type;
pub mod state;
pub mod system;
pub mod view;
pub mod world;

pub(crate) fn add_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register ChildOf bridge from pybevy_core
    pybevy_core::register_core_bridges();

    // Register global registry functions for cross-crate access
    message::register_message_write_fn();
    message::register_run_system_once_fn();

    // Register custom component batch bridge
    custom_batch::register_custom_batch_bridge();

    let ecs = PyModule::new(m.py(), "ecs")?;
    ecs.add_class::<commands::PyCommands>()?;
    // Re-export PyComponent so Python can import it from pybevy.ecs
    ecs.add_class::<component::PyComponent>()?;
    ecs.add_class::<component::PyComponentId>()?;
    ecs.add_class::<conditional_system::PyConditionalSystem>()?;
    ecs.add_class::<custom_component::PyCustomComponent>()?;
    ecs.add_class::<custom_batch::PyCustomComponentBatch>()?;
    ecs.add_class::<pybevy_core::PyRustComponentBatch>()?;
    ecs.add_function(wrap_pyfunction!(conditional_system::run_if, m)?)?;
    ecs.add_class::<lazy_wrapper_proxy::PyLazyWrapperProxy>()?;
    ecs.add_class::<PyF32List>()?;
    ecs.add_class::<PyEntity>()?;
    ecs.add_class::<entity_commands::PyEntityCommands>()?;
    ecs.add_class::<entity_commands::PyRelatedSpawnerCommands>()?;
    ecs.add_class::<message::PyMessage>()?;
    ecs.add_class::<message::PyMessageId>()?;
    ecs.add_class::<message::PyMessageReader>()?;
    ecs.add_class::<message::PyMessageWriter>()?;
    ecs.add_class::<messages::PyMessageType>()?;
    ecs.add_class::<messages::PyMessageTypeParam>()?;
    ecs.add_class::<messages::PyMessages>()?;
    ecs.add_class::<PyChildOf>()?;
    ecs.add_class::<PyChildren>()?;
    ecs.add_class::<PyChildrenIterator>()?;
    ecs.add_class::<local::PyLocal>()?;
    ecs.add_class::<mutable::PyMut>()?;
    // Name from pybevy_ecs crate
    pybevy_ecs::add_ecs_classes(&ecs)?;
    ecs.add_class::<observer::PyEvent>()?;
    ecs.add_class::<observer::PyOn>()?;
    ecs.add_class::<observer::PyOnTypeParam>()?;
    ecs.add_class::<observer::PyAdd>()?;
    ecs.add_class::<observer::PyInsert>()?;
    ecs.add_class::<observer::PyRemove>()?;
    ecs.add_class::<observer::PyReplace>()?;
    ecs.add_class::<observer::PyDespawn>()?;
    ecs.add_class::<query::query_param::PyQueryParam>()?;
    ecs.add_class::<query::query_runtime::PyQueryIter>()?;
    ecs.add_class::<view::view::PyView>()?;
    ecs.add_class::<view::view_param::PyViewParam>()?;
    ecs.add_class::<view::view::PyViewCol>()?;
    ecs.add_class::<view::view::PyViewColMut>()?;
    ecs.add_class::<view::view_column::PyViewColumn>()?;
    ecs.add_class::<view::view::PyBatch>()?;
    ecs.add_class::<view::view::PyBatchIterator>()?;
    ecs.add_class::<filter::filters::PyWith>()?;
    ecs.add_class::<filter::filters::PyWithout>()?;
    ecs.add_class::<filter::filters::PyChanged>()?;
    ecs.add_class::<filter::filters::PyAdded>()?;
    ecs.add_class::<filter::filters::PyHas>()?;
    ecs.add_class::<filter::filters::PyAnyOf>()?;
    ecs.add_class::<query::PyQuery>()?;
    ecs.add_class::<query::single::PySingle>()?;
    ecs.add_class::<query::single_runtime::PySingleQuery>()?;
    ecs.add_class::<resource::PyRes>()?;
    ecs.add_class::<resource::PyResMut>()?;
    ecs.add_class::<resource::PyResParam>()?;
    // Re-export PyResource so Python can import it from pybevy.ecs
    ecs.add_class::<resource::PyResource>()?;
    ecs.add_class::<state::PyState>()?;
    ecs.add_class::<state::PyNextState>()?;
    ecs.add_class::<state::PyOnEnterSchedule>()?;
    ecs.add_class::<state::PyOnExitSchedule>()?;
    ecs.add_class::<state::PyOnTransitionSchedule>()?;
    ecs.add_class::<state::PyDespawnOnExit>()?;
    ecs.add_class::<state::PyDespawnOnEnter>()?;
    ecs.add_function(wrap_pyfunction!(state::state, m)?)?;
    ecs.add_function(wrap_pyfunction!(state::in_state, m)?)?;
    ecs.add_function(wrap_pyfunction!(state::on_enter, m)?)?;
    ecs.add_function(wrap_pyfunction!(state::on_exit, m)?)?;
    ecs.add_function(wrap_pyfunction!(state::on_transition, m)?)?;
    ecs.add_class::<world::PyWorld>()?;
    ecs.add_class::<disabled::PyDisabled>()?;
    m.add_submodule(&ecs)
}
