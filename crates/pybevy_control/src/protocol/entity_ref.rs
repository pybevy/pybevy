use schemars::JsonSchema;
use serde::Deserialize;

/// Entity reference by packed `Entity::to_bits()` ID or Name string.
/// A Python `Entity.index()` and the index in its repr are not numeric IDs.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EntityRef {
    Id(u64),
    Name(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ref_deserialize_id() {
        let json = "42";
        let entity_ref: EntityRef = serde_json::from_str(json).unwrap();
        assert!(matches!(entity_ref, EntityRef::Id(42)));
    }

    #[test]
    fn entity_ref_deserialize_name() {
        let json = r#""MyEntity""#;
        let entity_ref: EntityRef = serde_json::from_str(json).unwrap();
        assert!(matches!(entity_ref, EntityRef::Name(ref s) if s == "MyEntity"));
    }
    #[test]
    fn entity_ref_debug_format() {
        let id_ref = EntityRef::Id(42);
        let name_ref = EntityRef::Name("Test".into());
        assert!(format!("{:?}", id_ref).contains("42"));
        assert!(format!("{:?}", name_ref).contains("Test"));
    }
}
