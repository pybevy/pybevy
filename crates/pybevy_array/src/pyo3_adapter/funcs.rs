//! Module-level constructors, element-wise functions, reductions, and
//! comparison helpers.

use pybevy_bytecodevm::bytecode::Op;
use pyo3::{prelude::*, types::PyModule};

use super::{
    array::PyArray,
    convert::{array_from_object, parse_shape},
    dtype::parse_dtype,
    kernels::{self, Reduce, extract_operand, extract_scalar, map_array_err},
};
use crate::{ArrayDType, ArrayStorage, DenseArrayCore, Scalar};

fn wrap(core: DenseArrayCore) -> PyArray {
    PyArray::wrap(core)
}

enum ExtractedCore<'py> {
    Array(PyRef<'py, PyArray>),
    Owned(DenseArrayCore),
}

impl ExtractedCore<'_> {
    fn as_ref(&self) -> &DenseArrayCore {
        match self {
            ExtractedCore::Array(array) => &array.core,
            ExtractedCore::Owned(core) => core,
        }
    }
}

fn as_core<'py>(obj: &Bound<'py, PyAny>) -> PyResult<ExtractedCore<'py>> {
    if let Ok(nd) = obj.extract::<PyRef<'_, PyArray>>() {
        Ok(ExtractedCore::Array(nd))
    } else {
        Ok(ExtractedCore::Owned(array_from_object(obj, None)?))
    }
}

fn float_unary(name: &'static str, op: Op, a: &Bound<'_, PyAny>) -> PyResult<PyArray> {
    let a = extract_operand(a)?;
    kernels::float_elementwise(name, &[Op::PushInput(0), op], &[], &[a.as_kernel()])
}

fn float_binary(
    name: &'static str,
    op: Op,
    a: &Bound<'_, PyAny>,
    b: &Bound<'_, PyAny>,
) -> PyResult<PyArray> {
    let a = extract_operand(a)?;
    let b = extract_operand(b)?;
    kernels::float_elementwise(
        name,
        &[Op::PushInput(0), Op::PushInput(1), op],
        &[],
        &[a.as_kernel(), b.as_kernel()],
    )
}

#[pyfunction]
#[pyo3(signature = (obj, dtype=None))]
fn array(obj: &Bound<'_, PyAny>, dtype: Option<Bound<'_, PyAny>>) -> PyResult<PyArray> {
    Ok(wrap(array_from_object(obj, parse_dtype(dtype.as_ref())?)?))
}

#[pyfunction]
#[pyo3(signature = (obj, dtype=None))]
fn asarray(obj: &Bound<'_, PyAny>, dtype: Option<Bound<'_, PyAny>>) -> PyResult<PyArray> {
    Ok(wrap(array_from_object(obj, parse_dtype(dtype.as_ref())?)?))
}

#[pyfunction]
#[pyo3(signature = (shape, dtype=None))]
fn zeros(shape: &Bound<'_, PyAny>, dtype: Option<Bound<'_, PyAny>>) -> PyResult<PyArray> {
    let dt = parse_dtype(dtype.as_ref())?.unwrap_or(ArrayDType::Float64);
    Ok(wrap(
        DenseArrayCore::zeros(dt, &parse_shape(shape)?).map_err(map_array_err)?,
    ))
}

#[pyfunction]
#[pyo3(signature = (shape, dtype=None))]
fn ones(shape: &Bound<'_, PyAny>, dtype: Option<Bound<'_, PyAny>>) -> PyResult<PyArray> {
    let dt = parse_dtype(dtype.as_ref())?.unwrap_or(ArrayDType::Float64);
    Ok(wrap(
        DenseArrayCore::ones(dt, &parse_shape(shape)?).map_err(map_array_err)?,
    ))
}

#[pyfunction]
#[pyo3(signature = (shape, dtype=None))]
fn empty(shape: &Bound<'_, PyAny>, dtype: Option<Bound<'_, PyAny>>) -> PyResult<PyArray> {
    let dt = parse_dtype(dtype.as_ref())?.unwrap_or(ArrayDType::Float64);
    Ok(wrap(
        DenseArrayCore::empty(dt, &parse_shape(shape)?).map_err(map_array_err)?,
    ))
}

#[pyfunction]
#[pyo3(signature = (shape, fill_value, dtype=None))]
fn full(
    shape: &Bound<'_, PyAny>,
    fill_value: &Bound<'_, PyAny>,
    dtype: Option<Bound<'_, PyAny>>,
) -> PyResult<PyArray> {
    let fill = extract_scalar(fill_value)?;
    let dt = match parse_dtype(dtype.as_ref())? {
        Some(dt) => dt,
        None => match fill {
            Scalar::F64(_) => ArrayDType::Float64,
            Scalar::I64(_) => ArrayDType::Int64,
            Scalar::Bool(_) => ArrayDType::Bool,
        },
    };
    Ok(wrap(
        DenseArrayCore::full(dt, &parse_shape(shape)?, fill).map_err(map_array_err)?,
    ))
}

#[pyfunction]
#[pyo3(signature = (start, stop=None, step=None, dtype=None))]
fn arange(
    start: &Bound<'_, PyAny>,
    stop: Option<Bound<'_, PyAny>>,
    step: Option<Bound<'_, PyAny>>,
    dtype: Option<Bound<'_, PyAny>>,
) -> PyResult<PyArray> {
    let first = extract_scalar(start)?;
    let (start_v, stop_v, all_int_bounds) = match &stop {
        None => (
            Scalar::I64(0),
            first,
            matches!(first, Scalar::I64(_) | Scalar::Bool(_)),
        ),
        Some(s) => {
            let last = extract_scalar(s)?;
            (
                first,
                last,
                matches!(first, Scalar::I64(_) | Scalar::Bool(_))
                    && matches!(last, Scalar::I64(_) | Scalar::Bool(_)),
            )
        }
    };
    let step_v = match &step {
        None => Scalar::I64(1),
        Some(s) => extract_scalar(s)?,
    };
    let step_int = matches!(step_v, Scalar::I64(_) | Scalar::Bool(_));
    let dt = match parse_dtype(dtype.as_ref())? {
        Some(dt) => dt,
        None if all_int_bounds && step_int => ArrayDType::Int64,
        None => ArrayDType::Float64,
    };
    Ok(wrap(
        DenseArrayCore::arange(start_v, stop_v, step_v, dt).map_err(map_array_err)?,
    ))
}

#[pyfunction]
#[pyo3(signature = (start, stop, num=50, dtype=None))]
fn linspace(
    start: f64,
    stop: f64,
    num: usize,
    dtype: Option<Bound<'_, PyAny>>,
) -> PyResult<PyArray> {
    let dt = parse_dtype(dtype.as_ref())?.unwrap_or(ArrayDType::Float64);
    // The storage layer reserves fallibly, so an impossible `num` raises.
    let mut storage = ArrayStorage::zeros(dt, num).map_err(map_array_err)?;
    if num == 1 {
        storage.set(0, Scalar::F64(start));
    } else if num > 1 {
        let step = (stop - start) / (num - 1) as f64;
        for i in 0..num - 1 {
            storage.set(i, Scalar::F64(start + i as f64 * step));
        }
        storage.set(num - 1, Scalar::F64(stop)); // NumPy pins the endpoint exactly.
    }
    Ok(wrap(
        DenseArrayCore::from_storage(storage, &[num]).map_err(map_array_err)?,
    ))
}

macro_rules! unary_ufunc {
    ($fn_name:ident, $label:literal, $op:expr) => {
        #[pyfunction]
        fn $fn_name(a: &Bound<'_, PyAny>) -> PyResult<PyArray> {
            float_unary($label, $op, a)
        }
    };
}

unary_ufunc!(sin, "sin", Op::Sin);
unary_ufunc!(cos, "cos", Op::Cos);
unary_ufunc!(tan, "tan", Op::Tan);
unary_ufunc!(sqrt, "sqrt", Op::Sqrt);
unary_ufunc!(exp, "exp", Op::Exp);
unary_ufunc!(log, "log", Op::Ln);
unary_ufunc!(log10, "log10", Op::Log10);
unary_ufunc!(log2, "log2", Op::Log2);
unary_ufunc!(floor, "floor", Op::Floor);
unary_ufunc!(ceil, "ceil", Op::Ceil);
unary_ufunc!(round, "round", Op::Round);
unary_ufunc!(sign, "sign", Op::Sign);
unary_ufunc!(abs, "abs", Op::Abs);

#[pyfunction]
fn minimum(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>) -> PyResult<PyArray> {
    float_binary("minimum", Op::Min, a, b)
}

#[pyfunction]
fn maximum(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>) -> PyResult<PyArray> {
    float_binary("maximum", Op::Max, a, b)
}

#[pyfunction]
fn clip(a: &Bound<'_, PyAny>, min: &Bound<'_, PyAny>, max: &Bound<'_, PyAny>) -> PyResult<PyArray> {
    let a = extract_operand(a)?;
    let min = extract_operand(min)?;
    let max = extract_operand(max)?;
    kernels::float_elementwise(
        "clip",
        &[
            Op::PushInput(0),
            Op::PushInput(1),
            Op::PushInput(2),
            Op::Clamp,
        ],
        &[],
        &[a.as_kernel(), min.as_kernel(), max.as_kernel()],
    )
}

#[pyfunction]
#[pyo3(name = "where")]
fn where_(
    condition: &Bound<'_, PyAny>,
    x: &Bound<'_, PyAny>,
    y: &Bound<'_, PyAny>,
) -> PyResult<PyArray> {
    let condition = extract_operand(condition)?;
    let x = extract_operand(x)?;
    let y = extract_operand(y)?;
    kernels::where_select(condition.as_kernel(), x.as_kernel(), y.as_kernel())
}

#[pyfunction]
fn isfinite(a: &Bound<'_, PyAny>) -> PyResult<PyArray> {
    let a = as_core(a)?;
    kernels::isfinite(a.as_ref())
}

#[pyfunction]
fn isclose(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>) -> PyResult<PyArray> {
    let a = extract_operand(a)?;
    let b = extract_operand(b)?;
    kernels::isclose(a.as_kernel(), b.as_kernel())
}

#[pyfunction]
fn allclose(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>) -> PyResult<bool> {
    let a = extract_operand(a)?;
    let b = extract_operand(b)?;
    kernels::allclose(a.as_kernel(), b.as_kernel())
}

#[pyfunction]
fn array_equal(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>) -> PyResult<bool> {
    let a = as_core(a)?;
    let b = as_core(b)?;
    kernels::array_equal(a.as_ref(), b.as_ref())
}

fn reduce_fn(
    py: Python<'_>,
    a: &Bound<'_, PyAny>,
    kind: Reduce,
    axis: Option<isize>,
) -> PyResult<Py<PyAny>> {
    let a = as_core(a)?;
    kernels::reduce(py, a.as_ref(), kind, axis)
}

macro_rules! reduction {
    ($fn_name:ident, $kind:expr) => {
        #[pyfunction]
        #[pyo3(signature = (a, axis=None))]
        fn $fn_name(
            py: Python<'_>,
            a: &Bound<'_, PyAny>,
            axis: Option<isize>,
        ) -> PyResult<Py<PyAny>> {
            reduce_fn(py, a, $kind, axis)
        }
    };
}

reduction!(sum, Reduce::Sum);
reduction!(mean, Reduce::Mean);
reduction!(min, Reduce::Min);
reduction!(max, Reduce::Max);
reduction!(all, Reduce::All);
reduction!(any, Reduce::Any);

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    macro_rules! add {
        ($($f:ident),* $(,)?) => { $( m.add_function(wrap_pyfunction!($f, m)?)?; )* };
    }
    add!(
        array,
        asarray,
        zeros,
        ones,
        empty,
        full,
        arange,
        linspace,
        sin,
        cos,
        tan,
        sqrt,
        exp,
        log,
        log10,
        log2,
        floor,
        ceil,
        round,
        sign,
        abs,
        minimum,
        maximum,
        clip,
        where_,
        isfinite,
        isclose,
        allclose,
        array_equal,
        sum,
        mean,
        min,
        max,
        all,
        any,
    );
    Ok(())
}
