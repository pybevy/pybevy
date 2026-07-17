use bevy::camera::RenderTarget;
use pybevy_core::{ComponentStorage, PyComponent, PyEntity, PyHandle, registry::global_registry};
use pybevy_macros::{pycomponent, pyenum};
use pybevy_math::uvec2::PyUVec2;
use pybevy_window::window_ref::PyWindowRef;
use pyo3::{PyTypeInfo, prelude::*};

use crate::{
    image_render_target::PyImageRenderTarget,
    manual_texture_view_handle::PyManualTextureViewHandle,
    normalized_render_target::PyNormalizedRenderTarget,
};

#[pyenum(RenderTarget, manual)]
#[pycomponent(
    RenderTarget,
    bridge,
    materialize = materialize_render_target
)]
#[pyclass(
    name = "RenderTarget",
    module = "pybevy.camera",
    extends = PyComponent,
    subclass
)]
pub struct PyRenderTarget {
    pub(crate) storage: ComponentStorage<RenderTarget>,
}

#[pymethods]
impl PyRenderTarget {
    pub fn as_image(&self) -> PyResult<Option<PyHandle>> {
        Ok(self.as_ref()?.as_image().map(PyHandle::from))
    }

    #[pyo3(signature = (primary_window = None))]
    pub fn normalize(
        &self,
        primary_window: Option<&PyEntity>,
    ) -> PyResult<Option<PyNormalizedRenderTarget>> {
        Ok(self
            .as_ref()?
            .normalize(primary_window.map(|entity| entity.0))
            .map(Into::into))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()? {
            RenderTarget::Window(value) => Ok(format!("RenderTarget.Window({value:?})")),
            RenderTarget::Image(value) => Ok(format!("RenderTarget.Image({value:?})")),
            RenderTarget::TextureView(value) => Ok(format!("RenderTarget.TextureView({value:?})")),
            RenderTarget::None { size } => Ok(format!(
                "RenderTarget.None_(size=UVec2({}, {}))",
                size.x, size.y
            )),
        }
    }
}

#[pyclass(
    name = "Window",
    module = "pybevy.camera",
    extends = PyRenderTarget
)]
pub struct PyRenderTargetWindow;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRenderTargetWindow {
    #[classattr]
    const __qualname__: &'static str = "RenderTarget.Window";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyWindowRef) -> PyClassInitializer<Self> {
        render_target_initializer(RenderTarget::Window(value.into())).add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyResult<PyWindowRef> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            RenderTarget::Window(value) => Ok((*value).into()),
            _ => unreachable!("RenderTarget.Window instance changed discriminant"),
        }
    }
}

#[pyclass(
    name = "Image",
    module = "pybevy.camera",
    extends = PyRenderTarget
)]
pub struct PyRenderTargetImage;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRenderTargetImage {
    #[classattr]
    const __qualname__: &'static str = "RenderTarget.Image";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyImageRenderTarget) -> PyClassInitializer<Self> {
        render_target_initializer(RenderTarget::Image(value.into())).add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyResult<PyImageRenderTarget> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            RenderTarget::Image(value) => Ok(value.clone().into()),
            _ => unreachable!("RenderTarget.Image instance changed discriminant"),
        }
    }
}

#[pyclass(
    name = "TextureView",
    module = "pybevy.camera",
    extends = PyRenderTarget
)]
pub struct PyRenderTargetTextureView;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRenderTargetTextureView {
    #[classattr]
    const __qualname__: &'static str = "RenderTarget.TextureView";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyManualTextureViewHandle) -> PyClassInitializer<Self> {
        render_target_initializer(RenderTarget::TextureView(value.into())).add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyResult<PyManualTextureViewHandle> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            RenderTarget::TextureView(value) => Ok((*value).into()),
            _ => unreachable!("RenderTarget.TextureView instance changed discriminant"),
        }
    }
}

#[pyclass(name = "None_", module = "pybevy.camera", extends = PyRenderTarget)]
pub struct PyRenderTargetNone;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyRenderTargetNone {
    #[classattr]
    const __qualname__: &'static str = "RenderTarget.None_";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("size",)
    }

    #[new]
    pub fn new(size: PyUVec2) -> PyClassInitializer<Self> {
        render_target_initializer(RenderTarget::None { size: size.into() }).add_subclass(Self)
    }

    #[getter]
    pub fn size(slf: PyRef<'_, Self>) -> PyResult<PyUVec2> {
        let base = slf.into_super();
        match base.storage.as_ref()? {
            RenderTarget::None { size } => Ok((*size).into()),
            _ => unreachable!("RenderTarget.None_ instance changed discriminant"),
        }
    }
}

pub fn materialize_render_target(
    py: Python<'_>,
    storage: ComponentStorage<RenderTarget>,
) -> PyResult<Py<PyAny>> {
    enum Variant {
        Window,
        Image,
        TextureView,
        None,
    }

    let variant = match storage.as_ref()? {
        RenderTarget::Window(_) => Variant::Window,
        RenderTarget::Image(_) => Variant::Image,
        RenderTarget::TextureView(_) => Variant::TextureView,
        RenderTarget::None { .. } => Variant::None,
    };
    let base = PyClassInitializer::from(PyComponent).add_subclass(PyRenderTarget { storage });

    match variant {
        Variant::Window => Ok(Py::new(py, base.add_subclass(PyRenderTargetWindow))?.into_any()),
        Variant::Image => Ok(Py::new(py, base.add_subclass(PyRenderTargetImage))?.into_any()),
        Variant::TextureView => {
            Ok(Py::new(py, base.add_subclass(PyRenderTargetTextureView))?.into_any())
        }
        Variant::None => Ok(Py::new(py, base.add_subclass(PyRenderTargetNone))?.into_any()),
    }
}

fn render_target_initializer(value: RenderTarget) -> PyClassInitializer<PyRenderTarget> {
    PyClassInitializer::from(PyComponent).add_subclass(PyRenderTarget {
        storage: ComponentStorage::owned(value),
    })
}

pub fn register_render_target_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("RenderTarget")?;
    base.setattr("Window", py.get_type::<PyRenderTargetWindow>())?;
    base.setattr("Image", py.get_type::<PyRenderTargetImage>())?;
    base.setattr("TextureView", py.get_type::<PyRenderTargetTextureView>())?;
    base.setattr("None_", py.get_type::<PyRenderTargetNone>())?;

    let canonical = PyRenderTarget::type_object_raw(py);
    for alias in [
        PyRenderTargetWindow::type_object_raw(py),
        PyRenderTargetImage::type_object_raw(py),
        PyRenderTargetTextureView::type_object_raw(py),
        PyRenderTargetNone::type_object_raw(py),
    ] {
        if !global_registry::register_component_bridge_alias(alias, canonical) {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "RenderTarget bridge was not registered before its variants",
            ));
        }
    }
    Ok(())
}
