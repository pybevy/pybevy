use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const ASSET_REFERENCE_FIELD: &str = "handle";
pub const ASSET_REFERENCE_KEY: &str = "asset_ref";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetReference {
    pub session: String,
    pub token: String,
    pub entity: u64,
    pub component: String,
    pub asset_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetReferenceValue {
    pub asset_ref: AssetReference,
}

#[derive(Resource)]
pub struct AssetReferenceSession {
    id: Uuid,
    secret: [u8; 16],
}

impl Default for AssetReferenceSession {
    fn default() -> Self {
        let secret = Uuid::new_v4();
        Self {
            id: Uuid::new_v4(),
            secret: *secret.as_bytes(),
        }
    }
}

impl AssetReferenceSession {
    pub fn contains(&self, reference: &AssetReference) -> bool {
        reference.session == self.id.to_string()
    }

    pub fn issue(
        &self,
        entity: u64,
        component: &str,
        asset_type: &str,
        handle_identity: &[u8],
    ) -> AssetReference {
        AssetReference {
            session: self.id.to_string(),
            token: self.token(entity, component, asset_type, handle_identity),
            entity,
            component: component.to_owned(),
            asset_type: asset_type.to_owned(),
        }
    }

    pub fn validate(
        &self,
        reference: &AssetReference,
        handle_identity: &[u8],
    ) -> Result<(), AssetReferenceValidationError> {
        if !self.contains(reference) {
            return Err(AssetReferenceValidationError::WrongSession);
        }
        let expected = self.token(
            reference.entity,
            &reference.component,
            &reference.asset_type,
            handle_identity,
        );
        if !tokens_equal(reference.token.as_bytes(), expected.as_bytes()) {
            return Err(AssetReferenceValidationError::StaleOrForged);
        }
        Ok(())
    }

    fn token(
        &self,
        entity: u64,
        component: &str,
        asset_type: &str,
        handle_identity: &[u8],
    ) -> String {
        let mut digest = Sha256::new();
        digest.update(b"pybevy.asset-reference.v1\0");
        digest.update(entity.to_le_bytes());
        update_part(&mut digest, component.as_bytes());
        update_part(&mut digest, asset_type.as_bytes());
        update_part(&mut digest, handle_identity);
        digest.update(self.secret);
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

fn tokens_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn update_part(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetReferenceValidationError {
    WrongSession,
    StaleOrForged,
}

pub fn rotate_asset_reference_session(world: &mut bevy::prelude::World) {
    if world.contains_resource::<AssetReferenceSession>() {
        world.insert_resource(AssetReferenceSession::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_bound_to_every_reference_identity_field() {
        let session = AssetReferenceSession::default();
        let reference = session.issue(7, "Mesh3d", "Mesh", b"index:4");
        assert_eq!(session.validate(&reference, b"index:4"), Ok(()));

        let mut changed = reference.clone();
        changed.entity = 8;
        assert_eq!(
            session.validate(&changed, b"index:4"),
            Err(AssetReferenceValidationError::StaleOrForged)
        );
        let mut changed = reference.clone();
        changed.component = "Mesh2d".to_owned();
        assert_eq!(
            session.validate(&changed, b"index:4"),
            Err(AssetReferenceValidationError::StaleOrForged)
        );
        let mut changed = reference.clone();
        changed.asset_type = "StandardMaterial".to_owned();
        assert_eq!(
            session.validate(&changed, b"index:4"),
            Err(AssetReferenceValidationError::StaleOrForged)
        );
        assert_eq!(
            session.validate(&reference, b"index:5"),
            Err(AssetReferenceValidationError::StaleOrForged)
        );
    }

    #[test]
    fn another_session_rejects_the_reference() {
        let first = AssetReferenceSession::default();
        let second = AssetReferenceSession::default();
        let reference = first.issue(7, "Mesh3d", "Mesh", b"index:4");
        assert_eq!(
            second.validate(&reference, b"index:4"),
            Err(AssetReferenceValidationError::WrongSession)
        );
    }

    #[test]
    fn rotation_invalidates_existing_references_without_creating_a_session() {
        let mut world = bevy::prelude::World::new();
        rotate_asset_reference_session(&mut world);
        assert!(!world.contains_resource::<AssetReferenceSession>());

        world.init_resource::<AssetReferenceSession>();
        let reference = world
            .resource::<AssetReferenceSession>()
            .issue(7, "Mesh3d", "Mesh", b"index:4");
        rotate_asset_reference_session(&mut world);
        assert_eq!(
            world
                .resource::<AssetReferenceSession>()
                .validate(&reference, b"index:4"),
            Err(AssetReferenceValidationError::WrongSession)
        );
    }
}
