//! Neutral error variants. Adapters map these to interpreter exceptions;
//! this crate never constructs Python objects.

use std::fmt;

use crate::dtype::ArrayDType;

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayError {
    /// Storage length does not equal the product of the shape.
    StorageLengthMismatch {
        shape_elements: usize,
        storage_len: usize,
    },
    /// An integer index was out of range for its axis.
    IndexOutOfBounds {
        axis: usize,
        index: isize,
        size: usize,
    },
    /// More index operations than the array has dimensions.
    TooManyIndices { ndim: usize, indices: usize },
    /// Reshape target has a different element count than the source.
    ReshapeMismatch { from: Vec<usize>, to: Vec<usize> },
    /// A slice step of zero is not allowed.
    ZeroStep,
    /// Attempted to mutate a read-only array.
    NotWritable,
    /// Operation on an array whose backing authority has been closed.
    Closed,
    /// Two shapes could not be broadcast together.
    BroadcastMismatch { left: Vec<usize>, right: Vec<usize> },
    /// Checked arithmetic on shape/stride/allocation sizes overflowed.
    Overflow(&'static str),
    /// The requested owned backing allocation could not be reserved.
    AllocationFailed { dtype: ArrayDType, elements: usize },
    /// Arrays have a bounded dimensionality so recursive adapter conversion
    /// cannot exhaust the native stack.
    TooManyDimensions { ndim: usize, max: usize },
    /// An element-wise operation was applied to an unsupported dtype.
    UnsupportedDType { op: &'static str, dtype: ArrayDType },
    /// A reduction axis is outside the array's dimensions.
    AxisOutOfBounds { axis: usize, ndim: usize },
    /// A boolean mask's length does not equal the array's element count.
    MaskLengthMismatch { mask_len: usize, size: usize },
    /// A borrowed array's backing data is no longer valid (the owning system
    /// finished, or access crossed threads).
    BorrowExpired(String),
    /// A min/max reduction over a zero-size axis (no identity).
    ZeroSizeReduction,
}

impl fmt::Display for ArrayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArrayError::StorageLengthMismatch {
                shape_elements,
                storage_len,
            } => write!(
                f,
                "storage length {storage_len} does not match shape element count {shape_elements}"
            ),
            ArrayError::IndexOutOfBounds { axis, index, size } => write!(
                f,
                "index {index} is out of bounds for axis {axis} with size {size}"
            ),
            ArrayError::TooManyIndices { ndim, indices } => write!(
                f,
                "too many indices for array: array is {ndim}-dimensional, but {indices} were indexed"
            ),
            ArrayError::ReshapeMismatch { from, to } => {
                write!(f, "cannot reshape array of shape {from:?} into {to:?}")
            }
            ArrayError::ZeroStep => write!(f, "slice step cannot be zero"),
            ArrayError::NotWritable => {
                write!(f, "assignment destination is read-only")
            }
            ArrayError::Closed => write!(f, "operation on a closed array"),
            ArrayError::BroadcastMismatch { left, right } => write!(
                f,
                "operands could not be broadcast together with shapes {left:?} {right:?}"
            ),
            ArrayError::Overflow(what) => write!(f, "integer overflow computing {what}"),
            ArrayError::AllocationFailed { dtype, elements } => write!(
                f,
                "could not allocate {elements} elements of dtype {}",
                dtype.name()
            ),
            ArrayError::TooManyDimensions { ndim, max } => {
                write!(
                    f,
                    "maximum supported array dimensionality is {max}, got {ndim}"
                )
            }
            ArrayError::UnsupportedDType { op, dtype } => write!(
                f,
                "{op} is not supported for dtype {}; use astype to a float dtype",
                dtype.name()
            ),
            ArrayError::AxisOutOfBounds { axis, ndim } => write!(
                f,
                "axis {axis} is out of bounds for array of dimension {ndim}"
            ),
            ArrayError::MaskLengthMismatch { mask_len, size } => write!(
                f,
                "boolean mask length {mask_len} does not match array size {size}"
            ),
            ArrayError::BorrowExpired(reason) => {
                write!(f, "borrowed array data is no longer valid: {reason}")
            }
            ArrayError::ZeroSizeReduction => write!(
                f,
                "zero-size array to reduction operation with no identity (min/max)"
            ),
        }
    }
}

impl std::error::Error for ArrayError {}

pub type ArrayResult<T> = Result<T, ArrayError>;
