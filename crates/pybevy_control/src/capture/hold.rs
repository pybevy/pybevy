use bevy::prelude::{Component, Resource};

/// Marker component for internal overlay UI that should always be hidden in screenshots.
/// Applied to hot reload overlay entities so the screenshot handler can find and hide them.
#[derive(Component)]
pub struct InternalOverlayUi;

/// Refcount of in-flight captures (screenshots, timelines, turnarounds) that
/// want the internal overlay off-screen. Captures only increment/decrement;
/// the overlay's render system drives `Visibility` from this every frame, so
/// restoration is automatic once the count returns to zero (and survives the
/// overlay entities being respawned mid-capture by a Full hot reload).
#[derive(Resource, Default)]
pub struct OverlaySuppression(pub u32);
