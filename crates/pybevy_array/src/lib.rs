//! Shared bounded array domain for PyBevy.
//!
//! The always-available core owns dtype metadata, shared typed storage,
//! shape/stride and basic-slice planning, broadcasting plans, casting, copy
//! materialization, and neutral error variants. The `numeric` feature adds
//! interpreter-neutral element-wise kernels over these layouts, and the `pyo3`
//! feature adds the CPython class/module adapter. Other backends consume the
//! same core and numeric modules through their own adapter leaves.
//!
//! Storage uses typed enum variants behind operation-scoped access guards.
//!
//! See the array compatibility and design documentation for the supported
//! surface and overall design.

mod backing;
mod broadcast;
mod codec;
mod core;
mod dtype;
mod error;
mod scalar;
mod shape;
mod storage;

#[cfg(feature = "numeric")]
pub mod kernels;

#[cfg(feature = "pyo3")]
mod pyo3_adapter;

pub use core::{AxisReduce, DenseArrayCore};

pub use broadcast::{broadcast_shapes, broadcast_strides};
pub use codec::{decode_read_only_le_bytes, encode_contiguous_le_bytes};
pub use dtype::{ArrayDType, DlpackDType};
pub use error::{ArrayError, ArrayResult};
#[cfg(feature = "pyo3")]
pub use pyo3_adapter::*;
pub use scalar::Scalar;
pub use shape::{
    IndexOp, Layout, MAX_NDIM, OffsetIter, c_contiguous_strides, checked_num_elements,
};
pub use storage::{ArrayStorage, BorrowProbe};
