use pybevy_reload::SystemGenerationRegistry;

use crate::ecs::dynamic_system::DynamicSystemHandle;

/// Main/PyO3 specialization of the neutral reload-generation registry.
pub(crate) type DynamicSystemRegistry = SystemGenerationRegistry<DynamicSystemHandle>;
