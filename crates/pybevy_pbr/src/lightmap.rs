use bevy::{math::Rect, pbr::Lightmap};
use pybevy_core::{ComponentStorage, HandleArg, PyComponent, PyHandle, extract_handle_from_any};
use pybevy_macros::pycomponent;
use pybevy_math::rect::PyRect;
use pyo3::prelude::*;

#[pycomponent(Lightmap, bridge, view_fields = [bicubic_sampling])]
#[pyclass(name = "Lightmap", module = "pybevy.pbr", extends = PyComponent)]
pub struct PyLightmap {
    pub(crate) storage: ComponentStorage<Lightmap>,
}

#[pymethods]
impl PyLightmap {
    #[new]
    #[pyo3(signature = (*, image = HandleArg::default(), uv_rect = None, bicubic_sampling = false))]
    pub fn new(
        image: HandleArg,
        uv_rect: Option<PyRect>,
        bicubic_sampling: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(Lightmap {
            image: image.into_handle()?,
            uv_rect: uv_rect
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0)),
            bicubic_sampling,
        })
        .into())
    }

    #[getter]
    pub fn image(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.image))
    }

    #[setter]
    pub fn set_image(&mut self, image: &Bound<'_, PyAny>) -> PyResult<()> {
        let handle = extract_handle_from_any(image)?;
        self.as_mut()?.image = handle.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn uv_rect(&self) -> PyResult<PyRect> {
        Ok(self.storage.borrow_field_as(|l| &l.uv_rect)?)
    }

    #[setter]
    pub fn set_uv_rect(&mut self, rect: PyRect) -> PyResult<()> {
        self.as_mut()?.uv_rect = rect.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn bicubic_sampling(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.bicubic_sampling)
    }

    #[setter]
    pub fn set_bicubic_sampling(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.bicubic_sampling = value;
        Ok(())
    }
}
