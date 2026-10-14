use pybevy_core::PyAssetPath;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(bevy::gltf::GltfAssetLabel, manual)]
#[pyclass(
    name = "GltfAssetLabel",
    module = "pybevy.gltf",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyGltfAssetLabel {
    Scene {
        value: usize,
    },
    Node {
        value: usize,
    },
    Mesh {
        value: usize,
    },
    #[pyo3(constructor = (*, mesh, primitive))]
    Primitive {
        mesh: usize,
        primitive: usize,
    },
    Texture {
        value: usize,
    },
    #[pyo3(constructor = (*, index, is_scale_inverted))]
    Material {
        index: usize,
        is_scale_inverted: bool,
    },
    DefaultMaterial(),
    Animation {
        value: usize,
    },
    Skin {
        value: usize,
    },
    InverseBindMatrices {
        value: usize,
    },
}

#[pymethods]
impl PyGltfAssetLabel {
    pub fn from_asset(&self, path: &str) -> PyAssetPath {
        PyAssetPath::new(path.to_string(), Some(self.__str__()))
    }

    pub fn __str__(&self) -> String {
        match self {
            PyGltfAssetLabel::Scene { value: index } => format!("Scene{index}"),
            PyGltfAssetLabel::Node { value: index } => format!("Node{index}"),
            PyGltfAssetLabel::Mesh { value: index } => format!("Mesh{index}"),
            PyGltfAssetLabel::Primitive { mesh, primitive } => {
                format!("Mesh{mesh}/Primitive{primitive}")
            }
            PyGltfAssetLabel::Texture { value: index } => format!("Texture{index}"),
            PyGltfAssetLabel::Material {
                index,
                is_scale_inverted,
            } => {
                if *is_scale_inverted {
                    format!("Material{index} (inverted)")
                } else {
                    format!("Material{index}")
                }
            }
            PyGltfAssetLabel::DefaultMaterial() => "DefaultMaterial".to_string(),
            PyGltfAssetLabel::Animation { value: index } => format!("Animation{index}"),
            PyGltfAssetLabel::Skin { value: index } => format!("Skin{index}"),
            PyGltfAssetLabel::InverseBindMatrices { value: index } => {
                format!("Skin{index}/InverseBindMatrices")
            }
        }
    }
}
