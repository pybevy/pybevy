//! Host-shareable storage element validation using the renderer's WGSL compiler.

use bevy::{pbr::MATERIAL_BIND_GROUP_INDEX, reflect::Reflect};
use naga::{
    AddressSpace, Handle, Module, Type, TypeInner,
    front::wgsl,
    proc::Layouter,
    valid::{Capabilities, ValidationFlags, Validator},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct StorageElement {
    pub wgsl: String,
    pub declarations: String,
    pub stride: u32,
    pub alignment: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageStructField {
    pub name: String,
    pub offset: u32,
    pub size: u32,
    pub alignment: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageStructLayout {
    pub size: u32,
    pub stride: u32,
    pub alignment: u32,
    pub fields: Vec<StorageStructField>,
}

/// Naga-readable bindings: the generated module's only WESL construct is its bind group constant.
pub fn generated_bindings_wgsl(source: &str) -> String {
    source.replace(
        "constants::MATERIAL_BIND_GROUP",
        &MATERIAL_BIND_GROUP_INDEX.to_string(),
    )
}

pub fn validate_bindings(source: &str) -> Result<(), String> {
    let source = generated_bindings_wgsl(source);
    let module = wgsl::parse_str(&source).map_err(|error| error.emit_to_string(&source))?;
    validate_member_names(&module)
}

fn validate_member_names(module: &Module) -> Result<(), String> {
    for (_, ty) in module.types.iter() {
        if let TypeInner::Struct { members, .. } = &ty.inner {
            for member in members {
                if let Some(name) = &member.name
                    && name.chars().last().is_some_and(|c| c.is_ascii_digit())
                {
                    return Err(format!(
                        "Storage struct {} member '{name}' ends in a digit; WGSL shader composition cannot round-trip it. Rename it to end in a letter or underscore.",
                        ty.name.as_deref().unwrap_or("<unnamed>")
                    ));
                }
            }
        }
    }
    Ok(())
}

impl Default for StorageElement {
    fn default() -> Self {
        Self {
            wgsl: "vec4<f32>".into(),
            declarations: String::new(),
            stride: 16,
            alignment: 16,
        }
    }
}

impl StorageElement {
    pub fn parse_layouts(values: &[(String, String)]) -> Result<Vec<Self>, String> {
        let mut declarations = Vec::new();
        for (_, source) in values {
            if !declarations.contains(&source.as_str()) {
                declarations.push(source.as_str());
            }
        }
        let shared = declarations.join("\n");
        values
            .iter()
            .map(|(element, source)| {
                let mut layout = Self::parse(element, &shared)?;
                layout.declarations = source.clone();
                Ok(layout)
            })
            .collect()
    }

    fn parse_module(
        element: &str,
        declarations: &str,
    ) -> Result<(Module, Handle<Type>, u32), String> {
        let source = format!(
            "{declarations}\n@group(0) @binding(0) var<storage, read> pybevy_storage_element: array<{element}>;"
        );
        let module = wgsl::parse_str(&source).map_err(|error| error.emit_to_string(&source))?;
        validate_member_names(&module)?;
        if module.global_variables.len() != 1
            || !module.functions.is_empty()
            || !module.entry_points.is_empty()
            || !module.overrides.is_empty()
        {
            return Err("Storage declarations may contain only types and constants".into());
        }
        if module
            .types
            .iter()
            .any(|(_, ty)| matches!(ty.inner, TypeInner::Atomic(_)))
        {
            return Err("Read-only material storage does not support atomic elements".into());
        }
        Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .map_err(|error| error.emit_to_string(&source))?;
        let (_, global) = module.global_variables.iter().next().unwrap();
        if !matches!(global.space, AddressSpace::Storage { .. }) {
            return Err("Storage elements must be host-shareable WGSL types".into());
        }
        let TypeInner::Array { base, stride, .. } = module.types[global.ty].inner else {
            return Err("Storage elements must form a runtime-sized array".into());
        };
        Ok((module, base, stride))
    }

    pub fn parse(element: &str, declarations: &str) -> Result<Self, String> {
        let (module, base, stride) = Self::parse_module(element, declarations)?;
        let mut layouter = Layouter::default();
        layouter
            .update(module.to_ctx())
            .map_err(|error| error.to_string())?;
        let alignment = layouter[base].alignment * 1;
        Ok(Self {
            wgsl: element.into(),
            declarations: declarations.into(),
            stride,
            alignment,
        })
    }

    pub fn struct_layout(element: &str, declarations: &str) -> Result<StorageStructLayout, String> {
        let (module, base, stride) = Self::parse_module(element, declarations)?;
        let TypeInner::Struct { members, span } = &module.types[base].inner else {
            return Err("Shader struct layout requires a WGSL struct".into());
        };
        let mut layouter = Layouter::default();
        layouter
            .update(module.to_ctx())
            .map_err(|error| error.to_string())?;
        let fields = members
            .iter()
            .map(|member| {
                Ok(StorageStructField {
                    name: member
                        .name
                        .clone()
                        .ok_or("Shader struct fields must be named")?,
                    offset: member.offset,
                    size: layouter[member.ty].size,
                    alignment: layouter[member.ty].alignment * 1,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(StorageStructLayout {
            size: *span,
            stride,
            alignment: layouter[base].alignment * 1,
            fields,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::StorageElement;

    #[test]
    fn derives_scalar_vector_matrix_and_nested_struct_layouts() {
        for (element, stride) in [
            ("f32", 4),
            ("u32", 4),
            ("vec2<f32>", 8),
            ("vec3<f32>", 16),
            ("mat3x3<f32>", 48),
            ("array<vec2<u32>, 3>", 24),
        ] {
            assert_eq!(StorageElement::parse(element, "").unwrap().stride, stride);
        }
        let value = StorageElement::parse(
            "Particle",
            "struct Particle { position: vec3<f32>, age: f32, weights: array<vec2<f32>, 3>, }",
        )
        .unwrap();
        assert_eq!((value.alignment, value.stride), (16, 48));
    }

    #[test]
    fn reports_struct_member_offsets_and_sizes() {
        let layout = StorageElement::struct_layout(
            "Tile",
            "struct Tile { weight: f32, position: vec3<f32>, count: i32, uv: vec2<f32>, }",
        )
        .unwrap();
        assert_eq!((layout.size, layout.stride, layout.alignment), (48, 48, 16));
        assert_eq!(
            layout
                .fields
                .iter()
                .map(|f| (f.offset, f.size, f.alignment))
                .collect::<Vec<_>>(),
            vec![(0, 4, 4), (16, 12, 16), (28, 4, 4), (32, 8, 8)]
        );
        assert_eq!(layout.fields[1].name, "position");
        assert!(StorageElement::struct_layout("vec4<f32>", "").is_err());
    }

    #[test]
    fn validates_shared_declarations_and_rejects_conflicting_names() {
        let source = "struct Paint { color: vec4<f32>, }".to_owned();
        let values = vec![("Paint".into(), source.clone()), ("Paint".into(), source)];
        assert_eq!(StorageElement::parse_layouts(&values).unwrap().len(), 2);
        let conflict = vec![
            ("Paint".into(), "struct Paint { a: f32, }".into()),
            ("Paint".into(), "struct Paint { b: u32, }".into()),
        ];
        assert!(StorageElement::parse_layouts(&conflict).is_err());
    }

    #[test]
    fn rejects_non_host_shareable_or_unsized_elements_and_code() {
        for value in [
            "bool",
            "array<f32>",
            "atomic<u32>",
            "ptr<storage, f32>",
            "unknown",
            "vec4<f16>",
        ] {
            assert!(StorageElement::parse(value, "").is_err(), "{value}");
        }
        assert!(StorageElement::parse("f32", "fn helper() {}").is_err());
    }
}
