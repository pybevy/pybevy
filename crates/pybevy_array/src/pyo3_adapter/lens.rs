//! Fused in-place expression lens over a writable bounded array.

use pybevy_bytecodevm::{
    buffer_lens::{
        BufferKey, alloc_buffer_key, execute_buffer_assignment, validate_buffer_program,
    },
    bytecode::FieldType,
    expr::RustExpr,
    view_engine::compile_assignment,
};
use pyo3::{
    PyTraverseError, PyVisit,
    exceptions::{PyIndexError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
};

use super::{array::PyArray, kernels::map_array_err};
use crate::ArrayDType;

fn field_type(dtype: ArrayDType) -> Option<(FieldType, &'static str)> {
    Some(match dtype {
        ArrayDType::Float32 => (FieldType::F32, "F32"),
        ArrayDType::Float64 => (FieldType::F64, "F64"),
        ArrayDType::Int64 => (FieldType::I64, "I64"),
        ArrayDType::Int32 => (FieldType::I32, "I32"),
        ArrayDType::Uint32 => (FieldType::U32, "U32"),
        ArrayDType::Uint8 => (FieldType::U8, "U8"),
        ArrayDType::Uint16 | ArrayDType::Bool => return None,
    })
}

/// Dynamic expression proxy returned by [`PyArray::lens`]. Integer subscripts
/// select lanes in the final axis. A scalar or one-dimensional array is treated
/// as one scalar lane over all elements.
#[pyclass(
    name = "ArrayLens",
    module = "pybevy.array",
    frozen,
    skip_from_py_object
)]
pub struct ArrayLens {
    owner: Py<PyArray>,
    key: BufferKey,
    field_type: FieldType,
    field_type_name: &'static str,
    itemsize: usize,
    width: usize,
    stride: usize,
    count: usize,
}

impl ArrayLens {
    pub fn new(py: Python<'_>, owner: &Bound<'_, PyArray>) -> PyResult<Py<Self>> {
        let array = owner.borrow();
        array.core.ensure_writable().map_err(map_array_err)?;
        if !array.core.is_c_contiguous() || array.core.layout().offset != 0 {
            return Err(PyValueError::new_err(
                "lens() requires a C-contiguous array with zero offset",
            ));
        }
        let dtype = array.core.dtype();
        let Some((field_type, field_type_name)) = field_type(dtype) else {
            return Err(PyTypeError::new_err(format!(
                "lens() does not support dtype {}",
                dtype.name()
            )));
        };
        let itemsize = dtype.itemsize();
        let (width, stride, count) = if array.core.ndim() <= 1 {
            (1, itemsize, array.core.size())
        } else {
            let width = *array.core.shape().last().expect("ndim is nonzero");
            let stride = width
                .checked_mul(itemsize)
                .ok_or_else(|| PyValueError::new_err("lens lane stride overflow"))?;
            let count = array.core.size().checked_div(width).unwrap_or(0);
            (width, stride, count)
        };
        drop(array);
        Py::new(
            py,
            ArrayLens {
                owner: owner.clone().unbind(),
                key: alloc_buffer_key(),
                field_type,
                field_type_name,
                itemsize,
                width,
                stride,
                count,
            },
        )
    }

    fn normalize_lane(&self, lane: isize) -> PyResult<usize> {
        let index = if lane < 0 {
            self.width.checked_sub(lane.unsigned_abs())
        } else {
            usize::try_from(lane)
                .ok()
                .filter(|index| *index < self.width)
        };
        index.ok_or_else(|| {
            PyIndexError::new_err(format!(
                "lane index {lane} is out of bounds for final-axis width {}",
                self.width
            ))
        })
    }

    fn field_expression(slf: &Bound<'_, Self>, lane: isize) -> PyResult<Py<PyAny>> {
        let this = slf.borrow();
        let index = this.normalize_lane(lane)?;
        this.owner
            .bind(slf.py())
            .borrow()
            .core
            .ensure_readable()
            .map_err(map_array_err)?;
        let offset = index * this.itemsize;
        let name = format!("lane[{index}]");
        let expression = slf
            .py()
            .import("pybevy.expr")?
            .getattr("FieldExpr")?
            .call1((this.key.index(), name, offset, this.field_type_name))?;
        Ok(expression.unbind())
    }

    fn assign(&self, py: Python<'_>, lane: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let index = self.normalize_lane(lane)?;
        let destination = index * self.itemsize;
        let expression = RustExpr::from_py_object(py, value)?;
        let bytecode = compile_assignment(self.key, destination, self.field_type, &expression);
        let validated =
            validate_buffer_program(&bytecode, self.key, self.stride, &[self.field_type])
                .map_err(|error| PyValueError::new_err(error.to_string()))?;

        let owner = self.owner.bind(py).borrow_mut();
        let mut storage = owner.core.write_storage().map_err(map_array_err)?;
        let base = storage.as_mut_contiguous_ptr().ok_or_else(|| {
            PyRuntimeError::new_err("array storage cannot provide a writable contiguous buffer")
        })?;
        // SAFETY: the backing write guard keeps exclusive storage access for
        // this synchronous call. The writable check gates borrowed storage,
        // and validation proves every lane is aligned and contained in `stride`.
        unsafe { execute_buffer_assignment(&validated, base, self.count) };
        Ok(())
    }
}

#[pymethods]
impl ArrayLens {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.owner)
    }

    fn __getitem__(slf: &Bound<'_, Self>, lane: isize) -> PyResult<Py<PyAny>> {
        Self::field_expression(slf, lane)
    }

    fn __setitem__(&self, py: Python<'_>, lane: isize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.assign(py, lane, value)
    }
}
