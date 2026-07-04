use pybevy_core::PyAssetPath;
use pyo3::prelude::*;

#[pyclass(name = "GltfAssetLabel", frozen, eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyGltfAssetLabel {
    Scene(usize),
    Node(usize),
    Mesh(usize),
    Primitive {
        mesh: usize,
        primitive: usize,
    },
    Texture(usize),
    Material {
        index: usize,
        is_scale_inverted: bool,
    },
    DefaultMaterial(),
    Animation(usize),
    Skin(usize),
    InverseBindMatrices(usize),
}

#[pymethods]
impl PyGltfAssetLabel {
    pub fn from_asset(&self, path: &str) -> PyAssetPath {
        PyAssetPath::new(path.to_string(), Some(self.__str__()))
    }

    pub fn __str__(&self) -> String {
        match self {
            PyGltfAssetLabel::Scene(index) => format!("Scene{index}"),
            PyGltfAssetLabel::Node(index) => format!("Node{index}"),
            PyGltfAssetLabel::Mesh(index) => format!("Mesh{index}"),
            PyGltfAssetLabel::Primitive { mesh, primitive } => {
                format!("Mesh{mesh}/Primitive{primitive}")
            }
            PyGltfAssetLabel::Texture(index) => format!("Texture{index}"),
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
            PyGltfAssetLabel::Animation(index) => format!("Animation{index}"),
            PyGltfAssetLabel::Skin(index) => format!("Skin{index}"),
            PyGltfAssetLabel::InverseBindMatrices(index) => {
                format!("Skin{index}/InverseBindMatrices")
            }
        }
    }
}
