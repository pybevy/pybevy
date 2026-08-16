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
}
