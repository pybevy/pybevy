use std::sync::Arc;

use bevy::{asset::RenderAssetUsages, render::storage::ShaderBuffer};
use pybevy_array::{ArrayDType, ArrayError, BorrowProbe, PyArray, borrowed_read_only_u8};
use pybevy_core::{
    AssetStorage, PyAsset,
    borrowed_array_anchor::{AssetBorrowAnchor, AssetBorrowAnchorMut},
    numpy_view_guard::{PendingNumpyViewGuard, PyNumpyViewGuard},
    public_error::{SHADER_BUFFER_DATA_CHANGED, SHADER_BUFFER_DATA_TYPE, SHADER_BUFFER_NO_DATA},
};
use pybevy_macros::pyasset;
use pyo3::{
    exceptions::{PyMemoryError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::PyBytes,
};

enum DataAnchor {
    Read(Arc<AssetBorrowAnchor>),
    Write(Arc<AssetBorrowAnchorMut>),
}

#[pyclass(name = "_ShaderBufferDataContext", module = "pybevy.render")]
pub struct ShaderBufferDataContext {
    array: Py<PyArray>,
    anchor: DataAnchor,
}

#[pymethods]
impl ShaderBufferDataContext {
    fn __enter__(&self, py: Python<'_>) -> PyResult<Py<PyArray>> {
        match &self.anchor {
            DataAnchor::Read(anchor) => anchor.check_read(),
            DataAnchor::Write(anchor) => anchor.check_read(),
        }
        .map_err(PyRuntimeError::new_err)?;
        Ok(self.array.clone_ref(py))
    }

    #[pyo3(signature = (_exc_type=None, _exc_value=None, _traceback=None))]
    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        match &self.anchor {
            DataAnchor::Read(anchor) => anchor.close(),
            DataAnchor::Write(anchor) => anchor.close(),
        }
        false
    }
}

#[pyasset(ShaderBuffer, bridge, not_loadable)]
#[pyclass(name = "ShaderBuffer", module = "pybevy.render", extends = PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyShaderBuffer {
    storage: AssetStorage<ShaderBuffer>,
}

#[pymethods]
impl PyShaderBuffer {
    #[new]
    #[pyo3(signature = (data = None, *, copy_on_resize = false))]
    pub fn new(
        data: Option<&Bound<'_, PyAny>>,
        copy_on_resize: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut buffer = match data {
            Some(value) => ShaderBuffer::new(
                &extract_shader_buffer_data_from_any(value)?,
                RenderAssetUsages::default(),
            ),
            None => ShaderBuffer::default(),
        };
        buffer.copy_on_resize = copy_on_resize;
        Ok(Self::from_owned(buffer).into())
    }

    #[staticmethod]
    pub fn with_size(py: Python<'_>, size: usize) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self::from_owned(ShaderBuffer::with_size(size, RenderAssetUsages::default())),
        )
    }

    pub fn data(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<ShaderBufferDataContext>> {
        let this = slf.borrow();
        let claim = this.storage.prepare_read_view()?;
        let guard = PyNumpyViewGuard::from_acquired(claim, slf.clone().unbind().into_any());
        let validity = this.storage.validity_flag();
        let buffer = this.storage.as_ref()?;
        let data = buffer
            .data
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err(SHADER_BUFFER_NO_DATA))?;
        let len = data.len();
        let anchor = Arc::new(AssetBorrowAnchor::new(validity, guard));
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        // SAFETY: the anchor retains the owner, read lease, and validity for this byte slice.
        let array = unsafe { borrowed_read_only_u8(data.as_ptr(), len, &[len], probe)? };
        Py::new(
            py,
            ShaderBufferDataContext {
                array: Py::new(py, array)?,
                anchor: DataAnchor::Read(anchor),
            },
        )
    }

    pub fn data_mut(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
    ) -> PyResult<Py<ShaderBufferDataContext>> {
        let mut this = slf.borrow_mut();
        let len = this
            .storage
            .as_ref()?
            .data
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err(SHADER_BUFFER_NO_DATA))?
            .len();
        let validity = this.storage.validity_flag();
        let claim = this.storage.prepare_write_view()?;
        let guard = PendingNumpyViewGuard::from_acquired(claim, slf.clone().unbind().into_any());
        let anchor = Arc::new(AssetBorrowAnchorMut::new(validity, guard));
        let probe: Arc<dyn BorrowProbe> = anchor.clone();
        let array = Py::new(py, PyArray::pending_borrowed_mut_u8(len, &[len], probe)?)?;
        let context = Py::new(
            py,
            ShaderBufferDataContext {
                array: array.clone_ref(py),
                anchor: DataAnchor::Write(anchor.clone()),
            },
        )?;
        let mut transaction = this.storage.begin_write_view(anchor.pending_claim())?;
        let current_len = transaction
            .preflight()
            .data
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err(SHADER_BUFFER_NO_DATA))?
            .len();
        if current_len != len {
            return Err(PyRuntimeError::new_err(SHADER_BUFFER_DATA_CHANGED));
        }
        let data = transaction
            .commit()
            .data
            .as_mut()
            .expect("preflight confirmed CPU data exists");
        {
            let mut pending = array.borrow_mut(py);
            // SAFETY: preflight validated this buffer under the retained exclusive asset claim.
            unsafe { pending.bind_borrowed_mut_u8(data.as_mut_ptr()) };
        }
        anchor.commit();
        drop(transaction);
        Ok(context)
    }

    pub fn data_copy<'py>(&self, py: Python<'py>) -> PyResult<Option<Py<PyBytes>>> {
        Ok(self
            .as_ref()?
            .data
            .as_ref()
            .map(|data| PyBytes::new(py, data).unbind()))
    }

    #[pyo3(signature = (value))]
    pub fn set_data(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let data = value.map(extract_shader_buffer_data_from_any).transpose()?;
        self.as_mut()?.data = data;
        Ok(())
    }

    pub fn resize(&mut self, size: u64) -> PyResult<()> {
        self.as_mut()?.resize(size);
        Ok(())
    }

    pub fn resize_in_place(&mut self, size: u64) -> PyResult<()> {
        self.as_mut()?.resize_in_place(size);
        Ok(())
    }

    pub fn data_len(&self) -> PyResult<Option<usize>> {
        Ok(self.as_ref()?.data.as_ref().map(Vec::len))
    }

    #[getter]
    pub fn copy_on_resize(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.copy_on_resize)
    }

    #[setter]
    pub fn set_copy_on_resize(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.copy_on_resize = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let length = self
            .data_len()?
            .map_or_else(|| "None".to_string(), |len| len.to_string());
        Ok(format!("ShaderBuffer(data_len={length})"))
    }
}

fn map_array_read_error(error: ArrayError) -> PyErr {
    match error {
        ArrayError::BorrowExpired(_) | ArrayError::AccessConflict => {
            PyRuntimeError::new_err(error.to_string())
        }
        ArrayError::AllocationFailed { .. } => PyMemoryError::new_err(error.to_string()),
        _ => PyValueError::new_err(error.to_string()),
    }
}

pub fn extract_shader_buffer_data_from_any(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(bytes.as_bytes().to_vec());
    }
    if value.is_instance_of::<PyArray>() {
        let array = value.extract::<PyRef<'_, PyArray>>()?;
        if array.core.dtype() != ArrayDType::Uint8 {
            return Err(PyTypeError::new_err(SHADER_BUFFER_DATA_TYPE));
        }
        return array
            .core
            .to_scalars()
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value.to_i64_trunc() as u8)
                    .collect()
            })
            .map_err(map_array_read_error);
    }
    Err(PyTypeError::new_err(SHADER_BUFFER_DATA_TYPE))
}
