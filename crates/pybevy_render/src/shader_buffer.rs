use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    platform::collections::AlignedVec,
    render::storage::{ShaderBuffer, ShaderBufferData},
};
use pybevy_array::{ArrayDType, ArrayError, BorrowProbe, PyArray, borrowed_read_only_u8};
use pybevy_core::{
    AssetStorage, PyAsset,
    borrowed_array_anchor::{AssetBorrowAnchor, AssetBorrowAnchorMut},
    numpy_view_guard::{PendingNumpyViewGuard, PyNumpyViewGuard},
    public_error::{
        SHADER_BUFFER_DATA_CHANGED, SHADER_BUFFER_DATA_TYPE, SHADER_BUFFER_NO_DATA,
        shader_buffer_size_unaligned, shader_buffer_too_large,
    },
};
use pybevy_macros::pyasset;
use pyo3::{
    PyTraverseError, PyVisit,
    buffer::PyBuffer,
    exceptions::{PyMemoryError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyDict, PyMemoryView},
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
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.array)
    }

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
    #[pyo3(signature = (data = None, *, element = None, copy_on_resize = false))]
    pub fn new(
        data: Option<&Bound<'_, PyAny>>,
        element: Option<&Bound<'_, PyAny>>,
        copy_on_resize: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut buffer = match data {
            Some(value) => ShaderBuffer::new(
                checked_data(extract_shader_data(value, element)?)?,
                RenderAssetUsages::default(),
            ),
            None => {
                reject_empty_element(element)?;
                ShaderBuffer::default()
            }
        };
        buffer.copy_on_resize = copy_on_resize;
        Ok(Self::from_owned(buffer).into())
    }

    #[staticmethod]
    pub fn with_size(py: Python<'_>, size: u64) -> PyResult<Py<Self>> {
        check_size(size)?;
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
        let data =
            cpu_data(&buffer).ok_or_else(|| PyRuntimeError::new_err(SHADER_BUFFER_NO_DATA))?;
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
        let len = cpu_data(&*this.storage.as_ref()?)
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
        let current_len = cpu_data(transaction.preflight())
            .ok_or_else(|| PyRuntimeError::new_err(SHADER_BUFFER_NO_DATA))?
            .len();
        if current_len != len {
            return Err(PyRuntimeError::new_err(SHADER_BUFFER_DATA_CHANGED));
        }
        let data = cpu_data_mut(transaction.commit()).expect("preflight confirmed CPU data exists");
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
        Ok(cpu_data(&*self.as_ref()?).map(|data| PyBytes::new(py, data).unbind()))
    }

    #[pyo3(signature = (value, *, element = None))]
    pub fn set_data(
        slf: &Bound<'_, Self>,
        value: Option<&Bound<'_, PyAny>>,
        element: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let data = match value {
            Some(value) => Some(checked_data(extract_shader_data(value, element)?)?),
            None => {
                reject_empty_element(element)?;
                None
            }
        };
        // Packing may execute Python; acquire mutable storage only after it finishes.
        let mut this = slf.try_borrow_mut()?;
        let mut buffer = PyShaderBuffer::as_mut(&mut this)?;
        buffer.data = match data {
            Some(data) => {
                let data = AlignedVec::from(data);
                let buffer_size = data.len() as u64;
                ShaderBufferData::Initialized { data, buffer_size }
            }
            None => ShaderBufferData::Uninitialized(buffer.buffer_size()),
        };
        Ok(())
    }

    pub fn resize(&mut self, size: u64) -> PyResult<()> {
        check_size(size)?;
        let mut buffer = self.as_mut()?;
        reserve_cpu_data(&mut buffer, size)?;
        buffer.resize(size);
        Ok(())
    }

    pub fn resize_buffer(&mut self, size: u64) -> PyResult<()> {
        check_size(size)?;
        self.as_mut()?.resize_buffer(size);
        Ok(())
    }

    pub fn buffer_size(&self) -> PyResult<u64> {
        Ok(self.as_ref()?.buffer_size())
    }

    pub fn data_len(&self) -> PyResult<Option<usize>> {
        Ok(cpu_data(&*self.as_ref()?).map(AlignedVec::len))
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

fn reject_empty_element(element: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    if element.is_some() {
        return Err(PyTypeError::new_err(
            "element= requires typed values, not None or raw bytes",
        ));
    }
    Ok(())
}

fn extract_shader_data(
    value: &Bound<'_, PyAny>,
    element: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<u8>> {
    if value.is_instance_of::<PyArray>() {
        reject_empty_element(element)?;
        return extract_shader_buffer_data_from_any(value);
    }
    if let Ok(view) = PyMemoryView::from(value) {
        reject_empty_element(element)?;
        if !view.getattr("c_contiguous")?.extract::<bool>()? {
            return Err(PyValueError::new_err(
                "ShaderBuffer requires a contiguous byte buffer",
            ));
        }
        return Ok(view
            .call_method0("tobytes")?
            .cast::<PyBytes>()?
            .as_bytes()
            .to_vec());
    }
    let kwargs = PyDict::new(value.py());
    if let Some(element) = element {
        kwargs.set_item("element", element)?;
    }
    let packed = value
        .py()
        .import("pybevy.material")?
        .getattr("pack_shader_data")?
        .call((value,), Some(&kwargs))?;
    Ok(packed.cast::<PyBytes>()?.as_bytes().to_vec())
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

/// wgpu requires buffer sizes in multiples of 4; Bevy 0.20 panics on upload otherwise.
fn check_size(size: u64) -> PyResult<()> {
    if size % 4 != 0 {
        return Err(PyValueError::new_err(shader_buffer_size_unaligned(size)));
    }
    Ok(())
}

fn checked_data(data: Vec<u8>) -> PyResult<Vec<u8>> {
    check_size(data.len() as u64)?;
    Ok(data)
}

fn cpu_data(buffer: &ShaderBuffer) -> Option<&AlignedVec> {
    match &buffer.data {
        ShaderBufferData::Initialized { data, .. } => Some(data),
        ShaderBufferData::Uninitialized(_) => None,
    }
}

fn cpu_data_mut(buffer: &mut ShaderBuffer) -> Option<&mut AlignedVec> {
    match &mut buffer.data {
        ShaderBufferData::Initialized { data, .. } => Some(data),
        ShaderBufferData::Uninitialized(_) => None,
    }
}

/// Bevy zero-extends CPU data with an infallible `AlignedVec::resize`, which aborts
/// on allocation failure; probe a fallible allocation first so that becomes a
/// `MemoryError`.
fn reserve_cpu_data(buffer: &mut ShaderBuffer, size: u64) -> PyResult<()> {
    let Some(data) = cpu_data(buffer) else {
        return Ok(());
    };
    let size =
        usize::try_from(size).map_err(|_| PyMemoryError::new_err(shader_buffer_too_large(size)))?;
    if size <= data.capacity() {
        return Ok(());
    }
    Vec::<u8>::new()
        .try_reserve_exact(size)
        .map_err(|_| PyMemoryError::new_err(shader_buffer_too_large(size as u64)))
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
    if let Ok(buffer) = PyBuffer::<u8>::get(value) {
        if !buffer.is_c_contiguous() {
            return Err(PyValueError::new_err(
                "ShaderBuffer requires a contiguous byte buffer",
            ));
        }
        return buffer.to_vec(value.py());
    }

    Err(PyTypeError::new_err(SHADER_BUFFER_DATA_TYPE))
}
