//! The bounded `Array` exposed through the PyO3 backend.

use pybevy_bytecodevm::bytecode::Op;
use pyo3::{
    basic::CompareOp,
    exceptions::{PyAttributeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyList, PySlice, PyTuple},
};

use super::{
    convert,
    dtype::{PyDType, parse_dtype},
    hint,
    kernels::{self, CmpKind, OperandRef, Reduce, extract_operand, gather_scalars, map_array_err},
    lens::ArrayLens,
};
use crate::{ArrayDType, DenseArrayCore, IndexOp, Scalar};

/// A bounded, C-contiguous array. Float element-wise math runs on the
/// dense VM; comparisons and reductions are exact per dtype.
#[pyclass(name = "Array", module = "pybevy.array", skip_from_py_object)]
#[derive(Clone)]
pub struct PyArray {
    pub core: DenseArrayCore,
}

impl PyArray {
    fn wrap(core: DenseArrayCore) -> Self {
        PyArray { core }
    }
}

fn binary_op(
    op: Op,
    name: &'static str,
    lhs: OperandRef<'_>,
    rhs: OperandRef<'_>,
) -> PyResult<PyArray> {
    kernels::float_elementwise(
        name,
        &[Op::PushInput(0), Op::PushInput(1), op],
        &[],
        &[lhs, rhs],
    )
}

fn unary(op: Op, name: &'static str, a: &PyArray) -> PyResult<PyArray> {
    kernels::float_elementwise(
        name,
        &[Op::PushInput(0), op],
        &[],
        &[OperandRef::Array(&a.core)],
    )
}

fn binary_other(
    array: &PyArray,
    other: &Bound<'_, PyAny>,
    op: Op,
    name: &'static str,
    reflected: bool,
) -> PyResult<PyArray> {
    let other = extract_operand(other)?;
    let this = array.as_operand();
    let other = other.as_kernel();
    if reflected {
        binary_op(op, name, other, this)
    } else {
        binary_op(op, name, this, other)
    }
}

fn inplace_other(
    slf: &Bound<'_, PyArray>,
    other: &Bound<'_, PyAny>,
    op: Op,
    name: &'static str,
) -> PyResult<()> {
    // Reject read-only/expired borrows before doing potentially expensive
    // arithmetic. The result itself is owned, then copied back only after all
    // reads finish, so `array += array` cannot observe partial writes.
    slf.borrow().core.ensure_writable().map_err(map_array_err)?;
    let result = {
        let this = slf.borrow();
        let other = extract_operand(other)?;
        binary_op(op, name, this.as_operand(), other.as_kernel())?
    };
    slf.borrow_mut()
        .core
        .assign_from(&result.core)
        .map_err(map_array_err)
}

/// Parse one axis index (int or slice) into an `IndexOp`.
fn parse_axis(item: &Bound<'_, PyAny>) -> PyResult<IndexOp> {
    if let Ok(slice) = item.cast::<PySlice>() {
        let start: Option<isize> = slice.getattr("start")?.extract()?;
        let stop: Option<isize> = slice.getattr("stop")?.extract()?;
        let step: Option<isize> = slice.getattr("step")?.extract()?;
        return Ok(IndexOp::Slice {
            start,
            stop,
            step: step.unwrap_or(1),
        });
    }
    let index: isize = item.extract().map_err(|_| {
        PyTypeError::new_err("only integers, slices, and tuples of them are valid indices")
    })?;
    Ok(IndexOp::Index(index))
}

fn parse_index(index: &Bound<'_, PyAny>) -> PyResult<Vec<IndexOp>> {
    if let Ok(tuple) = index.cast::<PyTuple>() {
        return tuple.iter().map(|item| parse_axis(&item)).collect();
    }
    Ok(vec![parse_axis(index)?])
}

/// If `index` is a bounded boolean array, return its flattened mask. A non-bool
/// bounded array (integer fancy indexing) is rejected; anything else is `None`
/// so basic indexing handles it.
fn as_bool_mask(index: &Bound<'_, PyAny>) -> PyResult<Option<Vec<bool>>> {
    let Ok(nd) = index.extract::<PyRef<'_, PyArray>>() else {
        return Ok(None);
    };
    if nd.core.dtype() != ArrayDType::Bool {
        return Err(PyTypeError::new_err(
            "only boolean-array advanced indexing is supported",
        ));
    }
    Ok(Some(
        nd.core
            .to_scalars()
            .map_err(map_array_err)?
            .iter()
            .map(|s| s.to_bool())
            .collect(),
    ))
}

#[pymethods]
impl PyArray {
    #[getter]
    fn shape<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::new(py, self.core.shape().iter().copied()).expect("usize tuple")
    }

    #[getter]
    fn dtype(&self) -> PyDType {
        PyDType::new(self.core.dtype())
    }

    #[getter]
    fn ndim(&self) -> usize {
        self.core.ndim()
    }

    #[getter]
    fn size(&self) -> usize {
        self.core.size()
    }

    #[getter]
    fn itemsize(&self) -> usize {
        self.core.itemsize()
    }

    #[getter]
    fn strides<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        let itemsize = self.core.itemsize() as isize;
        let bytes = self.core.strides().iter().map(|s| s * itemsize);
        PyTuple::new(py, bytes).expect("isize tuple")
    }

    fn __len__(&self) -> PyResult<usize> {
        self.core
            .shape()
            .first()
            .copied()
            .ok_or_else(|| PyTypeError::new_err("len() of unsized object"))
    }

    // Reached only for attributes not defined on the class: point users at the
    // to_numpy()/.copy() escape hatch. Dunder probes (numpy protocols, hasattr)
    // stay a plain AttributeError so those protocols keep working.
    fn __getattr__(&self, name: &str) -> PyResult<Py<PyAny>> {
        if name.starts_with("__") && name.ends_with("__") {
            Err(PyAttributeError::new_err(format!(
                "'pybevy.array.Array' object has no attribute '{name}'"
            )))
        } else {
            Err(PyAttributeError::new_err(hint::unsupported_attr(name)))
        }
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let data = convert::to_list(py, &self.core)?;
        Ok(format!(
            "Array({}, dtype={})",
            data.bind(py).repr()?,
            self.core.dtype().name()
        ))
    }

    fn tolist(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        convert::to_list(py, &self.core)
    }

    #[pyo3(signature = (*shape))]
    fn reshape(&self, shape: &Bound<'_, PyTuple>) -> PyResult<PyArray> {
        // Accept reshape(2, 3) and reshape((2, 3)).
        let new_shape = if shape.len() == 1 {
            convert::parse_shape(&shape.get_item(0)?)?
        } else {
            convert::parse_shape(shape.as_any())?
        };
        Ok(PyArray::wrap(
            self.core.reshape(&new_shape).map_err(map_array_err)?,
        ))
    }

    fn ravel(&self) -> PyResult<PyArray> {
        Ok(PyArray::wrap(self.core.ravel().map_err(map_array_err)?))
    }

    fn copy(&self) -> PyResult<PyArray> {
        Ok(PyArray::wrap(self.core.copy().map_err(map_array_err)?))
    }

    fn astype(&self, dtype: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        let dt = parse_dtype(Some(dtype))?
            .ok_or_else(|| PyTypeError::new_err("astype requires a dtype"))?;
        Ok(PyArray::wrap(self.core.astype(dt).map_err(map_array_err)?))
    }

    /// Build a fused expression proxy over this writable array.
    fn lens(slf: &Bound<'_, Self>) -> PyResult<Py<ArrayLens>> {
        ArrayLens::new(slf.py(), slf)
    }

    #[pyo3(signature = (axis=None))]
    fn sum(&self, py: Python<'_>, axis: Option<usize>) -> PyResult<Py<PyAny>> {
        kernels::reduce(py, &self.core, Reduce::Sum, axis)
    }
    #[pyo3(signature = (axis=None))]
    fn mean(&self, py: Python<'_>, axis: Option<usize>) -> PyResult<Py<PyAny>> {
        kernels::reduce(py, &self.core, Reduce::Mean, axis)
    }
    #[pyo3(signature = (axis=None))]
    fn min(&self, py: Python<'_>, axis: Option<usize>) -> PyResult<Py<PyAny>> {
        kernels::reduce(py, &self.core, Reduce::Min, axis)
    }
    #[pyo3(signature = (axis=None))]
    fn max(&self, py: Python<'_>, axis: Option<usize>) -> PyResult<Py<PyAny>> {
        kernels::reduce(py, &self.core, Reduce::Max, axis)
    }
    #[pyo3(signature = (axis=None))]
    fn all(&self, py: Python<'_>, axis: Option<usize>) -> PyResult<Py<PyAny>> {
        kernels::reduce(py, &self.core, Reduce::All, axis)
    }
    #[pyo3(signature = (axis=None))]
    fn any(&self, py: Python<'_>, axis: Option<usize>) -> PyResult<Py<PyAny>> {
        kernels::reduce(py, &self.core, Reduce::Any, axis)
    }

    fn to_numpy(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        convert::to_numpy(py, &self.core)
    }

    #[pyo3(signature = (dtype=None, copy=None))]
    fn __array__(
        &self,
        py: Python<'_>,
        dtype: Option<Bound<'_, PyAny>>,
        copy: Option<bool>,
    ) -> PyResult<Py<PyAny>> {
        let _ = (dtype, copy); // NumPy casts/copies after consuming the result.
        convert::to_numpy(py, &self.core)
    }

    fn __getitem__(&self, py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Some(mask) = as_bool_mask(index)? {
            let sub = self.core.mask_select(&mask).map_err(map_array_err)?;
            return Ok(Py::new(py, PyArray::wrap(sub))?.into_any());
        }
        let ops = parse_index(index)?;
        let sub = self.core.slice_copy(&ops).map_err(map_array_err)?;
        if sub.ndim() == 0 {
            let scalar = sub.get(&[]).map_err(map_array_err)?;
            Ok(kernels::scalar_to_py(py, scalar))
        } else {
            Ok(Py::new(py, PyArray::wrap(sub))?.into_any())
        }
    }

    fn __setitem__(
        slf: &Bound<'_, Self>,
        index: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            return Err(PyTypeError::new_err(hint::unsupported_list_assign()));
        }
        if let Some(mask) = as_bool_mask(index)? {
            let count = mask.iter().filter(|&&m| m).count();
            let values: Vec<Scalar> = {
                let operand = extract_operand(value)?;
                let operand = operand.as_kernel();
                match operand {
                    OperandRef::Scalar(v) => vec![v],
                    OperandRef::Array(_) => gather_scalars(operand, &[count])?,
                }
            };
            slf.borrow_mut()
                .core
                .mask_assign(&mask, &values)
                .map_err(map_array_err)?;
            return Ok(());
        }
        let ops = parse_index(index)?;
        let plan = slf.borrow().core.plan(&ops).map_err(map_array_err)?;
        let target_shape = plan.shape.clone();
        let values = {
            let operand = extract_operand(value)?;
            gather_scalars(operand.as_kernel(), &target_shape)?
        };
        let mut destination = slf.borrow_mut();
        let offsets: Vec<usize> = plan.iter_offsets().collect();
        let storage = destination.core.storage_mut().map_err(map_array_err)?;
        for (i, off) in offsets.into_iter().enumerate() {
            storage.set(off, values[i]);
        }
        Ok(())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<PyArray> {
        let kind = match op {
            CompareOp::Eq => CmpKind::Eq,
            CompareOp::Ne => CmpKind::Ne,
            CompareOp::Lt => CmpKind::Lt,
            CompareOp::Le => CmpKind::Le,
            CompareOp::Gt => CmpKind::Gt,
            CompareOp::Ge => CmpKind::Ge,
        };
        let other = extract_operand(other)?;
        kernels::compare(self.as_operand(), other.as_kernel(), kind)
    }

    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Add, "add", false)
    }
    fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Add, "add", true)
    }
    fn __iadd__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        inplace_other(slf, other, Op::Add, "add")
    }
    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Sub, "subtract", false)
    }
    fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Sub, "subtract", true)
    }
    fn __isub__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        inplace_other(slf, other, Op::Sub, "subtract")
    }
    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Mul, "multiply", false)
    }
    fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Mul, "multiply", true)
    }
    fn __imul__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        inplace_other(slf, other, Op::Mul, "multiply")
    }
    fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Div, "divide", false)
    }
    fn __rtruediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Div, "divide", true)
    }
    fn __itruediv__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        inplace_other(slf, other, Op::Div, "divide")
    }
    fn __pow__(&self, other: &Bound<'_, PyAny>, modulo: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        if !modulo.is_none() {
            return Err(PyValueError::new_err("3-argument pow is not supported"));
        }
        binary_other(self, other, Op::Pow, "power", false)
    }
    fn __mod__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Mod, "mod", false)
    }
    fn __rmod__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        binary_other(self, other, Op::Mod, "mod", true)
    }
    fn __neg__(&self) -> PyResult<PyArray> {
        unary(Op::Neg, "negative", self)
    }
    fn __abs__(&self) -> PyResult<PyArray> {
        unary(Op::Abs, "absolute", self)
    }
    fn __matmul__(&self, _other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        Err(PyTypeError::new_err(hint::unsupported_op(
            "matrix multiply (@)",
        )))
    }
    fn __rmatmul__(&self, _other: &Bound<'_, PyAny>) -> PyResult<PyArray> {
        Err(PyTypeError::new_err(hint::unsupported_op(
            "matrix multiply (@)",
        )))
    }
}

impl PyArray {
    fn as_operand(&self) -> OperandRef<'_> {
        OperandRef::Array(&self.core)
    }
}
