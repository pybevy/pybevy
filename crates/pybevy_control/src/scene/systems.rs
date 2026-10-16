use std::collections::HashMap;

use bevy::ecs::{
    schedule::{IntoSystemSet, Schedules, SystemSet},
    world::World,
};
use pybevy_ecs::shared::{
    schedule::ScheduleDisplayNameRegistry,
    system_runtime::{HotReloadGeneration, ReloadGenerationSet},
};

use crate::protocol::ControlError;

/// List scene-owned systems by stage, with optional engine internals.
pub fn list_systems(
    world: &mut World,
    include_internal: bool,
) -> Result<serde_json::Value, ControlError> {
    let mut stages = serde_json::Map::new();
    let mut visible_system_count = 0;
    let mut total_system_count = 0;
    let mut scene_system_count = 0;
    let mut internal_system_count = 0;
    let current_generation = world
        .get_resource::<HotReloadGeneration>()
        .map(|generation| generation.current);
    let effective_current_generation = current_generation.unwrap_or(0);
    let display_names = world.get_resource::<ScheduleDisplayNameRegistry>();

    if let Some(schedules) = world.get_resource::<Schedules>() {
        for (label, schedule) in schedules.iter() {
            let stage_total_system_count = schedule.systems_len();
            total_system_count += stage_total_system_count;
            let current_generation_systems = schedule
                .graph()
                .systems_in_set(
                    ReloadGenerationSet(effective_current_generation)
                        .into_system_set()
                        .intern(),
                )
                .ok();
            let retained_generation = current_generation.and_then(|generation| {
                generation
                    .checked_sub(1)
                    .map(|previous| (generation, previous))
            });
            let retained_generation_systems = retained_generation.and_then(|(_, previous)| {
                schedule
                    .graph()
                    .systems_in_set(ReloadGenerationSet(previous).into_system_set().intern())
                    .ok()
            });
            let retired_generation_systems = current_generation
                .map(|generation| {
                    (0..generation.saturating_sub(1))
                        .flat_map(|retired_generation| {
                            schedule
                                .graph()
                                .systems_in_set(
                                    ReloadGenerationSet(retired_generation)
                                        .into_system_set()
                                        .intern(),
                                )
                                .ok()
                                .into_iter()
                                .flat_map(move |systems| {
                                    systems.iter().map(move |key| (*key, retired_generation))
                                })
                        })
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();
            let mut active_system_count = 0;
            let mut retained_system_count = 0;
            let mut retired_system_count = 0;
            let mut stage_scene_system_count = 0;
            let mut stage_internal_system_count = 0;
            // Graph may be uninitialized; degrade to empty list rather than failing.
            let schedule_systems = schedule.systems().map(|iter| {
                iter.map(|(key, system)| (key, system.name().to_string()))
                    .collect::<Vec<_>>()
            });
            let systems: Vec<serde_json::Value> = match schedule_systems {
                Ok(schedule_systems) => {
                    let mut current_name_counts = HashMap::<String, usize>::new();
                    for (key, name) in &schedule_systems {
                        if current_generation_systems.is_some_and(|systems| systems.contains(key)) {
                            *current_name_counts.entry(name.clone()).or_default() += 1;
                        }
                    }

                    schedule_systems
                        .into_iter()
                        .filter_map(|(key, system_name)| {
                            let in_current_generation = current_generation_systems
                                .is_some_and(|systems| systems.contains(&key));
                            let in_retained_generation = retained_generation_systems
                                .is_some_and(|systems| systems.contains(&key));
                            let retired_generation = retired_generation_systems.get(&key).copied();
                            let scene_owned = in_current_generation
                                || in_retained_generation
                                || retired_generation.is_some();
                            if scene_owned {
                                stage_scene_system_count += 1;
                            } else {
                                stage_internal_system_count += 1;
                            }

                            if !include_internal && !scene_owned {
                                return None;
                            }

                            let (active, reload_generation, reload_state) = if in_current_generation
                            {
                                active_system_count += 1;
                                (true, Some(effective_current_generation), "active")
                            } else if in_retained_generation {
                                let has_current_successor = current_name_counts
                                    .get_mut(&system_name)
                                    .is_some_and(|remaining| {
                                        if *remaining == 0 {
                                            false
                                        } else {
                                            *remaining -= 1;
                                            true
                                        }
                                    });
                                if has_current_successor {
                                    retained_system_count += 1;
                                    (
                                        false,
                                        retained_generation.map(|(_, previous)| previous),
                                        "rollback_retained",
                                    )
                                } else {
                                    retired_system_count += 1;
                                    (
                                        false,
                                        retained_generation.map(|(_, previous)| previous),
                                        "retired",
                                    )
                                }
                            } else if let Some(retired_generation) = retired_generation {
                                retired_system_count += 1;
                                (false, Some(retired_generation), "retired")
                            } else {
                                active_system_count += 1;
                                (true, None, "active")
                            };
                            Some(serde_json::json!({
                                "name": system_name,
                                "origin": if scene_owned { "scene" } else { "internal" },
                                "active": active,
                                "reload_generation": reload_generation,
                                "reload_state": reload_state,
                            }))
                        })
                        .collect()
                }
                Err(_) => Vec::new(),
            };

            let stage_visible_system_count = systems.len();
            visible_system_count += stage_visible_system_count;
            scene_system_count += stage_scene_system_count;
            internal_system_count += stage_internal_system_count;

            if !include_internal && systems.is_empty() {
                continue;
            }

            stages.insert(
                display_names
                    .and_then(|registry| registry.get_by_label(label))
                    .map_or_else(|| format!("{label:?}"), str::to_owned),
                serde_json::json!({
                    "system_count": stage_visible_system_count,
                    "total_system_count": stage_total_system_count,
                    "scene_system_count": stage_scene_system_count,
                    "internal_system_count": stage_internal_system_count,
                    "active_system_count": active_system_count,
                    "rollback_retained_system_count": retained_system_count,
                    "retired_system_count": retired_system_count,
                    "systems": systems,
                }),
            );
        }
    }

    Ok(serde_json::json!({
        "include_internal": include_internal,
        "system_count": visible_system_count,
        "total_system_count": total_system_count,
        "scene_system_count": scene_system_count,
        "internal_system_count": internal_system_count,
        "stages": stages,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicU32};

    use bevy::ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel};
    use pybevy_ecs::shared::schedule::{
        StateMachineId, StateScheduleLabel, TransitionScheduleLabel,
    };

    use super::*;

    #[test]
    fn list_systems_empty_world() {
        let mut world = World::new();
        let result = list_systems(&mut world, false).unwrap();
        assert!(result["stages"].as_object().unwrap().is_empty());
        assert_eq!(result["system_count"], 0);
        assert_eq!(result["internal_system_count"], 0);
    }

    #[test]
    fn list_systems_returns_names_per_stage() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems((sys_alpha, sys_beta, sys_gamma));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, true).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["system_count"], 3);
        assert_eq!(stage["scene_system_count"], 0);
        assert_eq!(stage["internal_system_count"], 3);
        let systems = stage["systems"].as_array().expect("systems array");
        assert_eq!(systems.len(), 3);
        let names: Vec<String> = systems
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_string())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("sys_alpha")),
            "expected sys_alpha in {names:?}"
        );
        assert!(
            names.iter().any(|n| n.contains("sys_beta")),
            "expected sys_beta in {names:?}"
        );
        assert!(
            names.iter().any(|n| n.contains("sys_gamma")),
            "expected sys_gamma in {names:?}"
        );

        let filtered = list_systems(&mut world, false).unwrap();
        assert!(filtered["stages"]["TestStage"].is_null());
        assert_eq!(filtered["system_count"], 0);
        assert_eq!(filtered["total_system_count"], 3);
        assert_eq!(filtered["internal_system_count"], 3);
    }

    #[test]
    fn list_systems_empty_schedule() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(EmptyStage);
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, true).unwrap();
        let stage = &val["stages"]["EmptyStage"];
        assert_eq!(stage["system_count"], 0);
        let systems = stage["systems"].as_array().expect("systems array");
        assert!(
            systems.is_empty(),
            "expected empty systems, got {systems:?}"
        );
    }

    #[test]
    fn list_systems_count_matches_names_len() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems((sys_alpha, sys_beta));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, true).unwrap();
        let stage = &val["stages"]["TestStage"];
        let count = stage["system_count"].as_u64().unwrap() as usize;
        let systems_len = stage["systems"].as_array().unwrap().len();
        assert_eq!(count, systems_len);
    }

    #[test]
    fn list_systems_uses_registered_state_schedule_names() {
        let mut world = World::new();
        let first_enter = StateScheduleLabel::on_enter(StateMachineId::new(1), 7);
        let second_exit = StateScheduleLabel::on_exit(StateMachineId::new(2), 7);
        let transition = TransitionScheduleLabel::new(StateMachineId::new(1), 7, 8);
        let mut schedules = Schedules::default();
        for label in [
            first_enter.intern(),
            second_exit.intern(),
            transition.intern(),
        ] {
            let mut schedule = Schedule::new(label);
            schedule.add_systems(sys_alpha);
            schedule.initialize(&mut world).unwrap();
            schedules.insert(schedule);
        }
        let mut ordinary = Schedule::new(TestStage);
        ordinary.add_systems(sys_beta);
        ordinary.initialize(&mut world).unwrap();
        schedules.insert(ordinary);
        world.insert_resource(schedules);

        let mut names = ScheduleDisplayNameRegistry::default();
        names.insert(first_enter.intern(), "OnEnter(game.First.READY)");
        names.insert(second_exit.intern(), "OnExit(game.Second.READY)");
        names.insert(
            transition.intern(),
            "OnTransition(game.First.READY -> game.First.RUNNING)",
        );
        world.insert_resource(names);

        let value = list_systems(&mut world, true).unwrap();
        let stages = value["stages"].as_object().unwrap();
        assert!(stages.contains_key("OnEnter(game.First.READY)"));
        assert!(stages.contains_key("OnExit(game.Second.READY)"));
        assert!(stages.contains_key("OnTransition(game.First.READY -> game.First.RUNNING)"));
        assert!(stages.contains_key("TestStage"));
        assert!(
            stages
                .keys()
                .all(|name| !name.contains("StateMachineId") && !name.contains("state_hash"))
        );
    }

    #[test]
    fn list_systems_recognizes_generation_zero_without_reload_resource() {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_alpha.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta);
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["system_count"], 1);
        assert_eq!(stage["scene_system_count"], 1);
        assert_eq!(stage["internal_system_count"], 1);
        let system = &stage["systems"][0];
        assert!(system["name"].as_str().unwrap().contains("sys_alpha"));
        assert_eq!(system["origin"], "scene");
        assert_eq!(system["reload_generation"], 0);
    }

    #[test]
    fn list_systems_distinguishes_active_retained_and_retired_systems() {
        let mut world = World::new();
        world.insert_resource(HotReloadGeneration::new(Arc::new(AtomicU32::new(2))));

        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_alpha);
        schedule.add_systems(sys_delta.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(1)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(2)));
        schedule.add_systems(sys_gamma.in_set(ReloadGenerationSet(2)));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["system_count"], 4);
        assert_eq!(stage["total_system_count"], 5);
        assert_eq!(stage["scene_system_count"], 4);
        assert_eq!(stage["internal_system_count"], 1);
        assert_eq!(stage["active_system_count"], 2);
        assert_eq!(stage["rollback_retained_system_count"], 1);
        assert_eq!(stage["retired_system_count"], 1);
        assert_eq!(val["system_count"], 4);
        assert_eq!(val["total_system_count"], 5);
        assert_eq!(val["scene_system_count"], 4);
        assert_eq!(val["internal_system_count"], 1);

        let systems = stage["systems"].as_array().unwrap();
        assert!(systems.iter().all(|system| system["origin"] == "scene"));
        assert!(
            systems
                .iter()
                .all(|system| !system["name"].as_str().unwrap().contains("sys_alpha"))
        );
        let retained = systems
            .iter()
            .find(|system| {
                system["name"].as_str().unwrap().contains("sys_beta")
                    && system["reload_state"] == "rollback_retained"
            })
            .unwrap();
        assert_eq!(retained["active"], false);
        assert_eq!(retained["reload_generation"], 1);
        assert_eq!(retained["reload_state"], "rollback_retained");

        let current = systems
            .iter()
            .find(|system| system["name"].as_str().unwrap().contains("sys_gamma"))
            .unwrap();
        assert_eq!(current["active"], true);
        assert_eq!(current["reload_generation"], 2);
        assert_eq!(current["reload_state"], "active");

        let retired = systems
            .iter()
            .find(|system| system["name"].as_str().unwrap().contains("sys_delta"))
            .unwrap();
        assert_eq!(retired["active"], false);
        assert_eq!(retired["reload_generation"], 0);
        assert_eq!(retired["reload_state"], "retired");

        let with_internal = list_systems(&mut world, true).unwrap();
        let all_systems = with_internal["stages"]["TestStage"]["systems"]
            .as_array()
            .unwrap();
        let internal = all_systems
            .iter()
            .find(|system| system["name"].as_str().unwrap().contains("sys_alpha"))
            .unwrap();
        assert_eq!(internal["origin"], "internal");
        assert_eq!(with_internal["system_count"], 5);
    }

    #[test]
    fn list_systems_matches_retained_occurrences_one_for_one() {
        let mut world = World::new();
        world.insert_resource(HotReloadGeneration::new(Arc::new(AtomicU32::new(1))));

        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(0)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(1)));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["active_system_count"], 1);
        assert_eq!(stage["rollback_retained_system_count"], 1);
        assert_eq!(stage["retired_system_count"], 1);
        assert_eq!(stage["scene_system_count"], 3);
    }

    #[test]
    fn list_systems_classifies_nonzero_older_generation_as_retired() {
        let mut world = World::new();
        world.insert_resource(HotReloadGeneration::new(Arc::new(AtomicU32::new(3))));

        let mut schedules = Schedules::default();
        let mut schedule = Schedule::new(TestStage);
        schedule.add_systems(sys_delta.in_set(ReloadGenerationSet(1)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(2)));
        schedule.add_systems(sys_beta.in_set(ReloadGenerationSet(3)));
        schedule.initialize(&mut world).unwrap();
        schedules.insert(schedule);
        world.insert_resource(schedules);

        let val = list_systems(&mut world, false).unwrap();
        let stage = &val["stages"]["TestStage"];
        assert_eq!(stage["active_system_count"], 1);
        assert_eq!(stage["rollback_retained_system_count"], 1);
        assert_eq!(stage["retired_system_count"], 1);
        let retired = stage["systems"]
            .as_array()
            .unwrap()
            .iter()
            .find(|system| system["reload_state"] == "retired")
            .unwrap();
        assert!(retired["name"].as_str().unwrap().contains("sys_delta"));
        assert_eq!(retired["reload_generation"], 1);
    }

    // Test-only labels for list_systems coverage.
    #[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct TestStage;

    #[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct EmptyStage;

    fn sys_alpha() {}

    fn sys_beta() {}

    fn sys_gamma() {}

    fn sys_delta() {}
}
