use std::{collections::HashMap, fmt};

use bevy::ecs::{
    schedule::{
        InternedScheduleLabel, InternedSystemSet, IntoScheduleConfigs, LogLevel, NodeId, Schedule,
        ScheduleBuildSettings, ScheduleLabel, Schedules, SystemKey, SystemSet,
    },
    system::IntoSystem,
    world::World,
};

/// A system or set a schedule extension adds, reduced to its scheduling edges.
#[derive(Debug, Clone, Default)]
pub struct PlannedNode {
    pub name: String,
    /// `Some` for a set configuration, `None` for a system.
    pub set: Option<InternedSystemSet>,
    pub in_sets: Vec<InternedSystemSet>,
    pub before: Vec<InternedSystemSet>,
    pub after: Vec<InternedSystemSet>,
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct ScheduleExtensionCheck;

#[derive(SystemSet, Clone, PartialEq, Eq, Hash)]
struct MirrorNode {
    id: usize,
    name: String,
}

impl fmt::Debug for MirrorNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.name)
    }
}

fn placeholder() {}

struct Live<'a> {
    schedule: &'a Schedule,
    names: HashMap<SystemKey, String>,
}

impl<'a> Live<'a> {
    fn new(schedule: &'a Schedule) -> Self {
        let names = match schedule.systems() {
            Ok(systems) => systems
                .map(|(key, system)| (key, system.name().to_string()))
                .collect(),
            Err(_) => schedule
                .graph()
                .systems
                .iter()
                .map(|(key, system, _)| (key, system.name().to_string()))
                .collect(),
        };
        Self { schedule, names }
    }
}

#[derive(Default)]
struct Mirror {
    names: Vec<String>,
    systems: Vec<usize>,
    live: HashMap<NodeId, usize>,
    added_sets: HashMap<InternedSystemSet, usize>,
    hierarchy: Vec<(usize, usize)>,
    dependency: Vec<(usize, usize)>,
}

impl Mirror {
    fn add(&mut self, name: String, system: bool) -> usize {
        let id = self.names.len();
        self.names.push(name);
        if system {
            self.systems.push(id);
        }
        id
    }

    fn live_node(&mut self, live: &Live<'_>, node: NodeId) -> usize {
        if let Some(&id) = self.live.get(&node) {
            return id;
        }
        let graph = live.schedule.graph();
        let (name, system) = match node {
            NodeId::System(key) => (live.names.get(&key).cloned().unwrap_or_default(), true),
            NodeId::Set(key) => (
                graph
                    .system_sets
                    .get(key)
                    .map(|set| format!("{set:?}"))
                    .unwrap_or_default(),
                false,
            ),
        };
        let id = self.add(name, system);
        self.live.insert(node, id);
        id
    }

    fn set(&mut self, live: Option<&Live<'_>>, set: InternedSystemSet) -> usize {
        if let Some(live) = live
            && let Some(key) = live.schedule.graph().system_sets.get_key(set)
        {
            return self.live_node(live, NodeId::Set(key));
        }
        if let Some(&id) = self.added_sets.get(&set) {
            return id;
        }
        let id = self.add(format!("{set:?}"), false);
        self.added_sets.insert(set, id);
        id
    }

    fn node(&self, id: usize) -> MirrorNode {
        MirrorNode {
            id,
            name: self.names[id].clone(),
        }
    }
}

/// Build the live graph of `label` plus `chains` (each inner list is chained)
/// from placeholder systems, since a built schedule cannot drop systems.
pub fn check_schedule_extension(
    world: &mut World,
    label: InternedScheduleLabel,
    chains: &[Vec<PlannedNode>],
) -> Result<(), String> {
    let mut mirror = Mirror::default();
    {
        let live = world
            .get_resource::<Schedules>()
            .and_then(|schedules| schedules.get(label))
            .map(Live::new);
        let live = live.as_ref();
        if let Some(live) = live {
            for (parent, child) in live.schedule.graph().hierarchy().all_edges() {
                let edge = (
                    mirror.live_node(live, parent),
                    mirror.live_node(live, child),
                );
                mirror.hierarchy.push(edge);
            }
            for (first, then) in live.schedule.graph().dependency().all_edges() {
                let edge = (mirror.live_node(live, first), mirror.live_node(live, then));
                mirror.dependency.push(edge);
            }
        }
        for chain in chains {
            let mut previous = None;
            for planned in chain {
                let id = match planned.set {
                    Some(set) => mirror.set(live, set),
                    None => mirror.add(planned.name.clone(), true),
                };
                for &parent in &planned.in_sets {
                    let parent = mirror.set(live, parent);
                    mirror.hierarchy.push((parent, id));
                }
                for &target in &planned.before {
                    let target = mirror.set(live, target);
                    mirror.dependency.push((id, target));
                }
                for &target in &planned.after {
                    let target = mirror.set(live, target);
                    mirror.dependency.push((target, id));
                }
                if let Some(previous) = previous.replace(id) {
                    mirror.dependency.push((previous, id));
                }
            }
        }
    }

    let mut scratch = Schedule::new(ScheduleExtensionCheck);
    scratch.set_build_settings(ScheduleBuildSettings {
        ambiguity_detection: LogLevel::Ignore,
        hierarchy_detection: LogLevel::Ignore,
        report_sets: false,
        ..Default::default()
    });
    for &id in &mirror.systems {
        scratch.add_systems(
            IntoSystem::into_system(placeholder)
                .with_name(mirror.names[id].clone())
                .in_set(mirror.node(id)),
        );
    }
    for &(parent, child) in &mirror.hierarchy {
        scratch.configure_sets(mirror.node(child).in_set(mirror.node(parent)));
    }
    for &(first, then) in &mirror.dependency {
        scratch.configure_sets(mirror.node(first).before(mirror.node(then)));
    }
    match scratch.initialize(world) {
        Ok(_) => Ok(()),
        Err(error) => Err(error.to_string(scratch.graph(), world)),
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::Update,
        ecs::{
            schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel, Schedules, SystemSet},
            world::World,
        },
    };

    use super::{PlannedNode, check_schedule_extension};

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    enum Stage {
        Early,
        Late,
    }

    fn live() {}

    fn world_with_live_order() -> World {
        let mut world = World::new();
        let mut schedule = Schedule::new(Update);
        schedule.configure_sets(Stage::Early.before(Stage::Late));
        schedule.add_systems(live.in_set(Stage::Early));
        schedule.initialize(&mut world).unwrap();
        world.init_resource::<Schedules>();
        world.resource_mut::<Schedules>().insert(schedule);
        world
    }

    fn system(name: &str, before: &[Stage], after: &[Stage]) -> PlannedNode {
        PlannedNode {
            name: name.to_owned(),
            before: before.iter().map(|set| set.intern()).collect(),
            after: after.iter().map(|set| set.intern()).collect(),
            ..Default::default()
        }
    }

    fn live_system_count(world: &World) -> usize {
        world
            .resource::<Schedules>()
            .get(Update)
            .unwrap()
            .systems_len()
    }

    #[test]
    fn consistent_extension_builds_and_leaves_the_live_schedule_alone() {
        let mut world = world_with_live_order();
        let chains = vec![vec![system("between", &[Stage::Late], &[Stage::Early])]];
        assert_eq!(
            check_schedule_extension(&mut world, Update.intern(), &chains),
            Ok(())
        );
        assert_eq!(live_system_count(&world), 1);
    }

    #[test]
    fn cycle_with_the_live_graph_is_reported() {
        let mut world = world_with_live_order();
        let chains = vec![vec![system("inverted", &[Stage::Early], &[Stage::Late])]];
        let error = check_schedule_extension(&mut world, Update.intern(), &chains).unwrap_err();
        assert!(error.contains("before/after cycle"), "{error}");
        assert!(error.contains("inverted"), "{error}");
        assert_eq!(live_system_count(&world), 1);
    }

    #[test]
    fn chain_contradicting_a_set_order_is_reported_without_a_live_schedule() {
        let mut world = World::new();
        let chains = vec![vec![
            system("first", &[], &[Stage::Late]),
            PlannedNode {
                name: "second".to_owned(),
                in_sets: vec![Stage::Early.intern()],
                ..Default::default()
            },
        ]];
        let late = PlannedNode {
            name: "Late".to_owned(),
            set: Some(Stage::Late.intern()),
            after: vec![Stage::Early.intern()],
            ..Default::default()
        };
        let error = check_schedule_extension(
            &mut world,
            Update.intern(),
            &[chains[0].clone(), vec![late]],
        )
        .unwrap_err();
        assert!(error.contains("`second`"), "{error}");
    }
}
