use bevy::{ecs::world::World, prelude::Resource};

/// Number of captures in progress. Captures start one at a time so each
/// snapshots authored state, not another capture's; a `CaptureHold` takes the
/// lease when its capture begins and returns it when it releases.
#[derive(Resource, Default)]
pub struct CaptureLease {
    holders: u32,
}

pub(crate) fn capture_in_progress(world: &World) -> bool {
    world
        .get_resource::<CaptureLease>()
        .is_some_and(|lease| lease.holders > 0)
}

pub(super) fn take_capture_lease(world: &mut World) {
    world
        .get_resource_or_insert_with(CaptureLease::default)
        .holders += 1;
}

pub(super) fn return_capture_lease(world: &mut World) {
    if let Some(mut lease) = world.get_resource_mut::<CaptureLease>() {
        lease.holders = lease.holders.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_is_held_until_every_holder_returns_it() {
        let mut world = World::new();
        assert!(!capture_in_progress(&world));
        take_capture_lease(&mut world);
        take_capture_lease(&mut world);
        return_capture_lease(&mut world);
        assert!(capture_in_progress(&world));
        return_capture_lease(&mut world);
        assert!(!capture_in_progress(&world));
        return_capture_lease(&mut world);
        assert!(!capture_in_progress(&world));
    }
}
