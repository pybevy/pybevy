use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};

/// Entity reference by packed `Entity::to_bits()` ID or Name string.
/// A Python `Entity.index()` and the index in its repr are not numeric IDs.
#[derive(Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub enum EntityRef {
    Id(u64),
    Name(String),
}

impl EntityRef {
    /// A packed ID when `text` is all digits, else a Name, as in a URL path.
    pub fn parse(text: &str) -> Self {
        match text.parse::<u64>() {
            Ok(id) => Self::Id(id),
            Err(_) => Self::Name(text.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for EntityRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged, expecting = "an entity name or packed numeric ID")]
        enum Raw {
            Id(u64),
            Name(String),
        }
        Ok(match Raw::deserialize(deserializer)? {
            Raw::Id(id) => Self::Id(id),
            Raw::Name(name) => Self::parse(&name),
        })
    }
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
    fn entity_ref_numeric_string_is_an_id() {
        let entity_ref: EntityRef = serde_json::from_str(r#""7""#).unwrap();
        assert!(matches!(entity_ref, EntityRef::Id(7)));
    }

    #[test]
    fn entity_ref_rejects_other_json_kinds() {
        let error = serde_json::from_str::<EntityRef>("true").unwrap_err();
        assert_eq!(error.to_string(), "an entity name or packed numeric ID");
    }

    #[test]
    fn entity_ref_debug_format() {
        let id_ref = EntityRef::Id(42);
        let name_ref = EntityRef::Name("Test".into());
        assert!(format!("{:?}", id_ref).contains("42"));
        assert!(format!("{:?}", name_ref).contains("Test"));
    }
}
