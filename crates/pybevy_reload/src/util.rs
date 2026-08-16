use std::{
    env,
    sync::{Mutex, MutexGuard},
};

use bevy::ecs::{schedule::Schedules, world::World};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};

use crate::profiling::SystemMonitor;

/// Helper to lock a mutex, recovering from poison if a thread panicked while holding it.
pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        bevy::log::warn!(
            "Recovered from poisoned mutex - a thread may have panicked while holding the lock"
        );
        poisoned.into_inner()
    })
}

/// Check if verbose debug output is enabled via environment variable
pub fn is_verbose() -> bool {
    env::var("PYBEVY_VERBOSE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Parse a "WIDTHxHEIGHT" resolution string into (width, height) as f32.
pub fn parse_resolution(s: &str) -> Option<(f32, f32)> {
    let (w, h) = s.split_once('x').or_else(|| s.split_once('X'))?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

/// Refresh and return the current RSS of this process in MB.
pub fn get_current_rss_mb(world: &mut World) -> f64 {
    let Some(mut monitor) = world.get_resource_mut::<SystemMonitor>() else {
        return 0.0;
    };
    let Some(pid) = monitor.process_pid else {
        return 0.0;
    };
    monitor.system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing().with_memory(),
    );
    monitor
        .system
        .process(pid)
        .map(|p| p.memory() as f64 / 1_048_576.0)
        .unwrap_or(0.0)
}

/// Count total systems across all schedules in the world.
pub fn count_schedule_systems(world: &World) -> usize {
    let Some(schedules) = world.get_resource::<Schedules>() else {
        return 0;
    };
    schedules.iter().map(|(_, s)| s.systems_len()).sum()
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Arc};

    use sysinfo::{System, get_current_pid};

    use super::*;

    #[test]
    fn test_lock_or_recover_normal() {
        let mutex = Mutex::new(42);
        let guard = lock_or_recover(&mutex);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_lock_or_recover_poisoned() {
        let mutex = Arc::new(Mutex::new(42));
        let mutex2 = mutex.clone();
        let _ = std::thread::spawn(move || {
            let _guard = mutex2.lock().unwrap();
            panic!("intentional panic to poison mutex");
        })
        .join();
        let guard = lock_or_recover(&mutex);
        assert_eq!(*guard, 42);
    }

    #[test]
    fn current_rss_refreshes_an_uninitialized_process_table() {
        let mut world = World::new();
        world.insert_resource(SystemMonitor {
            system: System::new(),
            process_pid: Some(get_current_pid().expect("current PID should be available")),
            last_update: 0.0,
            fps_history: VecDeque::new(),
            last_render_update: 0.0,
        });

        assert!(get_current_rss_mb(&mut world) > 0.0);
    }
}
