use std::{collections::VecDeque, time::Duration};

use bevy::{
    asset::UntypedAssetLoadFailedEvent,
    ecs::{
        message::MessageReader,
        system::{Res, ResMut},
        world::World,
    },
    prelude::Resource,
    time::{Real, Time},
};

const KEPT_FAILURES: usize = 16;

struct AssetLoadFailure {
    path: String,
    error: String,
    at: Duration,
}

/// Recent asset load failures, newest last; Bevy reports them only to the log.
#[derive(Resource, Default)]
pub struct AssetLoadFailures(VecDeque<AssetLoadFailure>);

pub fn record_asset_load_failures(
    mut events: MessageReader<UntypedAssetLoadFailedEvent>,
    time: Option<Res<Time<Real>>>,
    mut failures: ResMut<AssetLoadFailures>,
) {
    let at = time.map_or(Duration::ZERO, |time| time.elapsed());
    for event in events.read() {
        let path = event.path.to_string();
        failures.0.retain(|failure| failure.path != path);
        failures.0.push_back(AssetLoadFailure {
            path,
            error: event.error.to_string(),
            at,
        });
        while failures.0.len() > KEPT_FAILURES {
            failures.0.pop_front();
        }
    }
}

/// `get_last_error.asset_load_failures`: path, Bevy's error, and its age.
pub fn asset_load_failures_json(world: &World) -> serde_json::Value {
    let Some(failures) = world
        .get_resource::<AssetLoadFailures>()
        .filter(|failures| !failures.0.is_empty())
    else {
        return serde_json::Value::Null;
    };
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(Duration::ZERO, Time::elapsed);
    serde_json::Value::Array(
        failures
            .0
            .iter()
            .map(|failure| {
                serde_json::json!({
                    "path": failure.path,
                    "error": failure.error,
                    "age_secs": now.saturating_sub(failure.at).as_secs_f64(),
                })
            })
            .collect(),
    )
}
