//! Interpreter-neutral, reusable settings for Bevy asset loads.

use std::{any::TypeId, sync::Arc};

use bevy::asset::{Asset, AssetPath, AssetServer, LoadBuilder, UntypedHandle, meta::Settings};

type SettingsTransform = Arc<dyn for<'a> Fn(LoadBuilder<'a>) -> LoadBuilder<'a> + Send + Sync>;

#[derive(Clone, Default)]
pub struct AssetLoadPlan {
    asset_type: Option<(TypeId, &'static str)>,
    transforms: Vec<SettingsTransform>,
}

impl AssetLoadPlan {
    pub fn asset_type(&self) -> Option<(TypeId, &'static str)> {
        self.asset_type
    }

    pub fn with_settings<A: Asset, S: Settings>(
        &self,
        name: &'static str,
        apply: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Result<Self, (&'static str, &'static str)> {
        self.validate_type(TypeId::of::<A>(), name)?;
        let apply = Arc::new(apply);
        let mut next = self.clone();
        next.asset_type = Some((TypeId::of::<A>(), name));
        next.transforms.push(Arc::new(move |builder| {
            let apply = apply.clone();
            builder.with_settings(move |settings: &mut S| apply(settings))
        }));
        Ok(next)
    }

    pub fn validate_type(
        &self,
        asset_type: TypeId,
        name: &'static str,
    ) -> Result<(), (&'static str, &'static str)> {
        if let Some((expected, expected_name)) = self.asset_type
            && expected != asset_type
        {
            return Err((expected_name, name));
        }
        Ok(())
    }

    pub fn load(
        &self,
        server: &AssetServer,
        path: AssetPath<'static>,
        asset_type: TypeId,
    ) -> UntypedHandle {
        let mut builder = server.load_builder();
        for transform in &self.transforms {
            builder = transform(builder);
        }
        builder.load_erased(asset_type, path)
    }
}
