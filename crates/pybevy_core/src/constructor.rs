//! Presence-only checks for competing constructor representations.

pub mod pyo3;

use crate::public_error;

pub fn check_conflict(
    ty: &str,
    native: &[(&str, bool)],
    alternate: &[(&str, bool)],
) -> Result<(), String> {
    if native.iter().any(|(_, supplied)| *supplied)
        && alternate.iter().any(|(_, supplied)| *supplied)
    {
        let native: Vec<_> = native.iter().map(|(name, _)| *name).collect();
        let alternate: Vec<_> = alternate.iter().map(|(name, _)| *name).collect();
        return Err(public_error::constructor_form_conflict(
            ty, &native, &alternate,
        ));
    }
    Ok(())
}

pub fn check_complete_form(ty: &str, members: &[(&str, bool)]) -> Result<(), String> {
    if members.iter().any(|(_, supplied)| *supplied) {
        let missing: Vec<_> = members
            .iter()
            .filter_map(|(name, supplied)| (!supplied).then_some(*name))
            .collect();
        if !missing.is_empty() {
            let names: Vec<_> = members.iter().map(|(name, _)| *name).collect();
            return Err(public_error::constructor_partial_group(
                ty, &names, &missing,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn conflict_depends_on_presence_in_both_forms() {
        for native in [false, true] {
            for alternate in [false, true] {
                assert_eq!(
                    check_conflict(
                        "Rectangle",
                        &[("width", native)],
                        &[("half_size", alternate)]
                    )
                    .is_err(),
                    native && alternate,
                );
            }
        }
    }

    #[test]
    fn complete_form_requires_every_member_once_any_is_supplied() {
        assert!(check_complete_form("Rot2", &[("cos", true), ("sin", true)]).is_ok());
        assert!(check_complete_form("Rot2", &[("cos", false), ("sin", false)]).is_ok());
        // The exact shared message; the Rot2 text is cross-checked by the
        // accepted Python pin in tests/math/test_callable_contract.py.
        assert_eq!(
            check_complete_form("Rot2", &[("cos", true), ("sin", false)]).unwrap_err(),
            "Rot2() requires (cos, sin) together; missing: sin"
        );
        // A multi-member group names every missing member, in group order.
        assert_eq!(
            check_complete_form("Probe", &[("a", false), ("b", true), ("c", false)]).unwrap_err(),
            "Probe() requires (a, b, c) together; missing: a, c"
        );
    }
}
