use bevy::{camera::CameraMainTextureUsages, render::render_resource::TextureUsages};
use pybevy_core::PyComponent;
use pybevy_macros::newtype_storage;
use pyo3::prelude::*;

const DEFAULT_USAGES: u32 = TextureUsages::RENDER_ATTACHMENT.bits()
    | TextureUsages::TEXTURE_BINDING.bits()
    | TextureUsages::COPY_SRC.bits();

#[newtype_storage(CameraMainTextureUsages)]
#[pyclass(name = "CameraMainTextureUsages", extends = PyComponent, frozen)]
#[derive(Clone)]
pub struct PyCameraMainTextureUsages(pub(crate) CameraMainTextureUsages);

#[pymethods]
impl PyCameraMainTextureUsages {
    #[new]
    #[pyo3(signature = (flags = DEFAULT_USAGES))]
    pub fn new(flags: u32) -> (Self, PyComponent) {
        let usages = TextureUsages::from_bits_truncate(flags);
        (
            PyCameraMainTextureUsages(CameraMainTextureUsages(usages)),
            PyComponent,
        )
    }

    #[getter]
    pub fn flags(&self) -> u32 {
        self.0.0.bits()
    }

    #[pyo3(name = "with_")]
    pub fn with_(&self, py: Python<'_>, usages: u32) -> PyResult<Py<Self>> {
        let current = self.0.0;
        let additional = TextureUsages::from_bits_truncate(usages);
        Py::new(
            py,
            (
                PyCameraMainTextureUsages(CameraMainTextureUsages(current | additional)),
                PyComponent,
            ),
        )
    }

    #[classattr]
    const COPY_SRC: u32 = TextureUsages::COPY_SRC.bits();

    #[classattr]
    const COPY_DST: u32 = TextureUsages::COPY_DST.bits();

    #[classattr]
    const TEXTURE_BINDING: u32 = TextureUsages::TEXTURE_BINDING.bits();

    #[classattr]
    const STORAGE_BINDING: u32 = TextureUsages::STORAGE_BINDING.bits();

    #[classattr]
    const RENDER_ATTACHMENT: u32 = TextureUsages::RENDER_ATTACHMENT.bits();

    pub fn __repr__(&self) -> String {
        format!("CameraMainTextureUsages(flags=0x{:x})", self.0.0.bits())
    }

    pub fn __eq__(&self, other: &Self) -> bool {
        self.0.0.bits() == other.0.0.bits()
    }
}
