//! Interpreter-neutral wording for stable public Python errors.
//!
//! Backend adapters still choose the native exception class. Put messages here
//! when both PyO3 and RustPython expose the same invalid operation so wording
//! cannot drift independently.

use std::fmt::{Debug, Display};

pub const ADD_SYSTEMS_SCHEDULE_TYPE: &str =
    "add_systems() schedule parameter must be Stage, OnEnter(), OnExit(), or OnTransition()";

pub fn invalid_asset_type(actual: impl Display) -> String {
    format!("Invalid asset type. Expected a subclass of `Asset`, but got `{actual}`")
}

pub fn entity_does_not_exist(entity: impl Debug) -> String {
    format!("Entity {entity:?} does not exist")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_error_wording() {
        assert_eq!(
            invalid_asset_type("<class 'bool'>"),
            "Invalid asset type. Expected a subclass of `Asset`, but got `<class 'bool'>`"
        );
        assert_eq!(entity_does_not_exist(7), "Entity 7 does not exist");
    }
}
