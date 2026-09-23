use std::{
    env,
    sync::{Mutex, MutexGuard},
};

use bevy::ecs::{
    schedule::{IntoSystemSet, Schedules, SystemSet},
    world::World,
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};

use crate::{profiling::SystemMonitor, state::ReloadGenerationSet};

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

/// Count systems assigned to one hot-reload generation across all schedules.
pub fn count_generation_schedule_systems(world: &World, generation: u32) -> usize {
    let Some(schedules) = world.get_resource::<Schedules>() else {
        return 0;
    };
    let generation_set = ReloadGenerationSet(generation).into_system_set().intern();
    schedules
        .iter()
        .filter_map(|(_, schedule)| schedule.graph().systems_in_set(generation_set).ok())
        .map(|systems| systems.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Arc};

    use bevy::ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel};
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

    #[test]
    fn parse_resolution_accepts_both_cases_and_rejects_garbage() {
        assert_eq!(parse_resolution("1024x768"), Some((1024.0, 768.0)));
        assert_eq!(parse_resolution("640X480"), Some((640.0, 480.0)));
        assert_eq!(parse_resolution("1024-768"), None);
        assert_eq!(parse_resolution("1024x"), None);
        assert_eq!(parse_resolution("x768"), None);
        assert_eq!(parse_resolution("1024x768x"), None);
        assert_eq!(parse_resolution("1024xabc"), None);
        assert_eq!(parse_resolution(""), None);
    }

    #[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
    struct TestSchedule;

    fn generation_zero_system() {}

    fn generation_one_system() {}

    fn infrastructure_system() {}

    #[test]
    fn schedule_counts_distinguish_total_and_generation_membership() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestSchedule);
        schedule.add_systems((
            generation_zero_system.in_set(ReloadGenerationSet(0)),
            generation_one_system.in_set(ReloadGenerationSet(1)),
            infrastructure_system,
        ));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        assert_eq!(count_schedule_systems(&world), 3);
        assert_eq!(count_generation_schedule_systems(&world, 0), 1);
        assert_eq!(count_generation_schedule_systems(&world, 1), 1);
        assert_eq!(count_generation_schedule_systems(&world, 2), 0);
    }
}
