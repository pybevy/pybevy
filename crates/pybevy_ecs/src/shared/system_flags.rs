use bevy::ecs::system::SystemStateFlags;

/// Compute the system state flags based on parameter requirements.
///
/// - `needs_exclusive`: system has a World parameter (requires exclusive access)
/// - `needs_commands`: system has a Commands parameter (requires deferred sync points)
/// - `needs_main_thread`: system has a `NonSendMarker` parameter
///
/// Exclusivity itself is declared by `initialize` returning
/// `SystemAccess::Exclusive`. An exclusive system is NON_SEND without DEFERRED,
/// exactly as Bevy's `&mut World` parameter sets it.
///
/// A main-thread system is NON_SEND, as Bevy's `NonSendMarker::init_access`
/// sets it; it may also be DEFERRED.
pub fn compute_system_flags(
    needs_exclusive: bool,
    needs_commands: bool,
    needs_main_thread: bool,
) -> SystemStateFlags {
    if needs_exclusive {
        return SystemStateFlags::NON_SEND;
    }
    let mut flags = SystemStateFlags::empty();
    if needs_main_thread {
        flags |= SystemStateFlags::NON_SEND;
    }
    if needs_commands {
        flags |= SystemStateFlags::DEFERRED;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_takes_priority_over_commands() {
        let flags = compute_system_flags(true, true, false);
        assert!(flags == SystemStateFlags::NON_SEND);
    }

    #[test]
    fn exclusive_without_commands() {
        let flags = compute_system_flags(true, false, false);
        assert!(flags == SystemStateFlags::NON_SEND);
    }

    #[test]
    fn commands_only() {
        let flags = compute_system_flags(false, true, false);
        assert!(flags == SystemStateFlags::DEFERRED);
    }

    #[test]
    fn neither_exclusive_nor_commands() {
        let flags = compute_system_flags(false, false, false);
        assert!(flags.is_empty());
    }

    #[test]
    fn main_thread_is_non_send_without_being_exclusive() {
        let flags = compute_system_flags(false, false, true);
        assert!(flags == SystemStateFlags::NON_SEND);
    }

    #[test]
    fn main_thread_keeps_deferred_commands() {
        let flags = compute_system_flags(false, true, true);
        assert!(flags == SystemStateFlags::NON_SEND | SystemStateFlags::DEFERRED);
    }

    #[test]
    fn exclusive_is_already_main_thread() {
        assert!(
            compute_system_flags(true, false, true) == compute_system_flags(true, false, false)
        );
    }
}
