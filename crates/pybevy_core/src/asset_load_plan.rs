//! Interpreter-neutral, reusable settings for Bevy asset loads.

use std::{any::TypeId, sync::Arc};

use bevy::asset::{Asset, AssetPath, AssetServer, LoadBuilder, UntypedHandle, meta::Settings};

type SettingsTransform = Arc<dyn for<'a> Fn(LoadBuilder<'a>) -> LoadBuilder<'a> + Send + Sync>;

#[derive(Clone, Default)]
pub struct AssetLoadPlan {
    asset_type: Option<(TypeId, &'static str)>,
    loader_extensions: Option<&'static [&'static str]>,
    transforms: Vec<SettingsTransform>,
}

impl AssetLoadPlan {
    pub fn asset_type(&self) -> Option<(TypeId, &'static str)> {
        self.asset_type
    }

    pub fn with_settings<A: Asset, S: Settings>(
        &self,
        name: &'static str,
        loader_extensions: &'static [&'static str],
        apply: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Result<Self, (&'static str, &'static str)> {
        self.validate_type(TypeId::of::<A>(), name)?;
        let apply = Arc::new(apply);
        let mut next = self.clone();
        next.asset_type = Some((TypeId::of::<A>(), name));
        next.loader_extensions = Some(loader_extensions);
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

    pub fn validate_load_type(
        &self,
        path: &AssetPath<'_>,
        asset_type: TypeId,
        name: &'static str,
    ) -> Result<(), (&'static str, &'static str)> {
        if path.label().is_some()
            && path.get_extension().is_some_and(|extension| {
                self.loader_extensions
                    .is_some_and(|extensions| extensions.contains(&extension))
            })
        {
            return Ok(());
        }
        self.validate_type(asset_type, name)
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

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use bevy::asset::AssetPath;

    use super::AssetLoadPlan;

    #[test]
    fn labelled_subassets_use_loader_extensions_not_output_type() {
        let plan = AssetLoadPlan {
            asset_type: Some((TypeId::of::<u8>(), "Root")),
            loader_extensions: Some(&["gltf", "glb"]),
            transforms: Vec::new(),
        };
        let subasset_type = TypeId::of::<u16>();

        for path in ["model.gltf#Scene0", "model.glb#Mesh0/Primitive0"] {
            assert!(
                plan.validate_load_type(&AssetPath::from(path), subasset_type, "Subasset")
                    .is_ok()
            );
        }
        for path in ["model.gltf", "image.png#Scene0"] {
            assert_eq!(
                plan.validate_load_type(&AssetPath::from(path), subasset_type, "Subasset"),
                Err(("Root", "Subasset"))
            );
        }
        assert!(
            plan.validate_load_type(&AssetPath::from("model.gltf"), TypeId::of::<u8>(), "Root")
                .is_ok()
        );
    }
}
