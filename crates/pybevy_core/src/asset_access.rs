use std::{error::Error, fmt};

use bevy::{asset::UntypedAssetId, ecs::world::World};

use crate::{ActiveAssetAccess, AssetAccessRegistry, public_error};

#[derive(Debug, Clone)]
pub struct ActiveAssetAccessError {
    operation: String,
    access: ActiveAssetAccess,
}

impl fmt::Display for ActiveAssetAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let asset_id = match self.access.asset_id {
            UntypedAssetId::Index { index, .. } => {
                format!("asset index bits {}", index.to_bits())
            }
            UntypedAssetId::Uuid { uuid, .. } => format!("asset UUID {uuid}"),
        };
        f.write_str(&public_error::active_asset_access(
            &self.operation,
            &self.access.asset_name,
            &self.access.origin,
            &asset_id,
        ))
    }
}

impl Error for ActiveAssetAccessError {}

/// Refuse a world operation that can synchronously execute native asset
/// mutation while a Python asset pointer or zero-copy view is still valid.
pub fn ensure_no_live_asset_access(
    world: &World,
    operation: impl Into<String>,
) -> Result<(), ActiveAssetAccessError> {
    let Some(registry) = world.get_resource::<AssetAccessRegistry>() else {
        return Ok(());
    };
    let Some(access) = registry.first_active() else {
        return Ok(());
    };
    Err(ActiveAssetAccessError {
        operation: operation.into(),
        access,
    })
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::any::TypeId;

    use bevy::{
        asset::{Asset, Handle},
        reflect::TypePath,
    };

    use super::*;
    use crate::{AssetBorrowCounter, ValidityFlag, ensure_asset_access_registry};

    #[derive(Asset, TypePath)]
    struct TestAsset;

    #[test]
    fn barrier_is_idle_until_a_per_asset_wrapper_is_borrowed() {
        let mut world = World::new();
        ensure_asset_access_registry(&mut world);
        let registry = world.resource::<AssetAccessRegistry>();
        let scope = registry.new_scope(
            TypeId::of::<TestAsset>(),
            "TestAsset",
            ValidityFlag::new_write(),
            "test",
        );
        assert!(ensure_no_live_asset_access(&world, "world.spawn()").is_ok());

        let counter = AssetBorrowCounter::from_scope(scope);
        let asset_id = Handle::<TestAsset>::default().id().untyped();
        let ptr = Box::into_raw(Box::new(TestAsset));
        let world_cell = world.as_unsafe_world_cell_readonly();
        let storage = unsafe {
            crate::AssetStorage::borrowed_readonly_tracked(
                ptr,
                world_cell,
                asset_id,
                ValidityFlag::new_read().with_access_mode(crate::AccessMode::Read),
                counter,
            )
        }
        .expect("live tracked asset scope");
        let error = ensure_no_live_asset_access(&world, "world.spawn()").unwrap_err();
        assert_eq!(
            error.to_string(),
            "Cannot call world.spawn() while a borrowed TestAsset asset from test is live (asset UUID 97128bb1-2588-480b-bdc6-87b4adbec477). Drop the asset wrapper or close its view first."
        );
        drop(storage);
        assert!(ensure_no_live_asset_access(&world, "world.spawn()").is_ok());

        unsafe { drop(Box::from_raw(ptr)) };
    }

    #[derive(Asset, TypePath)]
    struct AlphaAsset;

    #[derive(Asset, TypePath)]
    struct BetaAsset;

    /// Bevy's fixed, type-independent `AssetId::DEFAULT_UUID`, pinned for exact-message asserts.
    const DEFAULT_ID_DISPLAY: &str = "asset UUID 97128bb1-2588-480b-bdc6-87b4adbec477";

    #[test]
    fn barrier_reports_the_first_live_scope_by_name_and_falls_back_on_release() {
        let mut world = World::new();
        ensure_asset_access_registry(&mut world);
        let registry = world.resource::<AssetAccessRegistry>();
        let alpha_scope = registry.new_scope(
            TypeId::of::<AlphaAsset>(),
            "Alpha",
            ValidityFlag::new_write(),
            "origin-alpha",
        );
        let beta_scope = registry.new_scope(
            TypeId::of::<BetaAsset>(),
            "Beta",
            ValidityFlag::new_write(),
            "origin-beta",
        );

        assert!(ensure_no_live_asset_access(&world, "app.update()").is_ok());

        let world_cell = world.as_unsafe_world_cell_readonly();
        let alpha_ptr = Box::into_raw(Box::new(AlphaAsset));
        let alpha_id = Handle::<AlphaAsset>::default().id().untyped();
        let alpha_storage = unsafe {
            // SAFETY: alpha_ptr owns a live Box allocation valid until the final drop below.
            crate::AssetStorage::borrowed_readonly_tracked(
                alpha_ptr,
                world_cell,
                alpha_id,
                ValidityFlag::new_read().with_access_mode(crate::AccessMode::Read),
                AssetBorrowCounter::from_scope(alpha_scope),
            )
        }
        .expect("live tracked asset scope");
        assert_eq!(
            ensure_no_live_asset_access(&world, "app.update()")
                .unwrap_err()
                .to_string(),
            "Cannot call app.update() while a borrowed Alpha asset from origin-alpha is live ("
                .to_string()
                + DEFAULT_ID_DISPLAY
                + "). Drop the asset wrapper or close its view first."
        );

        let beta_ptr = Box::into_raw(Box::new(BetaAsset));
        let beta_id = Handle::<BetaAsset>::default().id().untyped();
        let beta_storage = unsafe {
            // SAFETY: beta_ptr owns a live Box allocation valid until the final drop below.
            crate::AssetStorage::borrowed_readonly_tracked(
                beta_ptr,
                world_cell,
                beta_id,
                ValidityFlag::new_read().with_access_mode(crate::AccessMode::Read),
                AssetBorrowCounter::from_scope(beta_scope),
            )
        }
        .expect("live tracked asset scope");
        // Both live: the alphabetically first asset name is still reported.
        assert_eq!(
            ensure_no_live_asset_access(&world, "app.update()")
                .unwrap_err()
                .to_string(),
            "Cannot call app.update() while a borrowed Alpha asset from origin-alpha is live ("
                .to_string()
                + DEFAULT_ID_DISPLAY
                + "). Drop the asset wrapper or close its view first."
        );

        drop(alpha_storage);
        // Releasing the first live scope falls back to the remaining one.
        assert_eq!(
            ensure_no_live_asset_access(&world, "app.update()")
                .unwrap_err()
                .to_string(),
            "Cannot call app.update() while a borrowed Beta asset from origin-beta is live ("
                .to_string()
                + DEFAULT_ID_DISPLAY
                + "). Drop the asset wrapper or close its view first."
        );

        drop(beta_storage);
        assert!(ensure_no_live_asset_access(&world, "app.update()").is_ok());

        unsafe {
            // SAFETY: both pointers were created by Box::into_raw in this test and are un-freed.
            drop(Box::from_raw(alpha_ptr));
            drop(Box::from_raw(beta_ptr));
        }
    }
}
