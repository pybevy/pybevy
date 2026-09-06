use bevy::prelude::Resource;

/// Importable identity of the scene module currently driving an app.
///
/// This interpreter-neutral resource stores only a name. Interpreter adapters
/// resolve that name on demand, so hot reload can replace the live module
/// without retaining a stale interpreter object in the Bevy World.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct ActiveSceneModule {
    name: String,
}

impl ActiveSceneModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
