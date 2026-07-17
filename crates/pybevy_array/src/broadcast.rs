//! Broadcasting plans. The core validates and computes broadcast shapes and
//! per-operand strides; adapters decide which plans their public API exposes.

use crate::{
    error::{ArrayError, ArrayResult},
    shape::Layout,
};

/// Compute the broadcast of two shapes using NumPy's right-aligned rule: axes
/// are compatible when equal or one of them is 1.
pub fn broadcast_shapes(a: &[usize], b: &[usize]) -> ArrayResult<Vec<usize>> {
    let rank = a.len().max(b.len());
    let mut out = vec![0usize; rank];
    for i in 0..rank {
        let da = axis_from_end(a, i);
        let db = axis_from_end(b, i);
        let dim = if da == db || db == 1 {
            da
        } else if da == 1 {
            db
        } else {
            return Err(ArrayError::BroadcastMismatch {
                left: a.to_vec(),
                right: b.to_vec(),
            });
        };
        out[rank - 1 - i] = dim;
    }
    Ok(out)
}

fn axis_from_end(shape: &[usize], from_end: usize) -> usize {
    if from_end < shape.len() {
        shape[shape.len() - 1 - from_end]
    } else {
        1
    }
}

/// Strides that view `layout` as if it had `target_shape`, using stride 0 for
/// broadcast (size-1 or missing leading) axes. Errors if `layout` cannot be
/// broadcast to `target_shape`.
pub fn broadcast_strides(layout: &Layout, target_shape: &[usize]) -> ArrayResult<Vec<isize>> {
    if layout.shape.len() > target_shape.len() {
        return Err(ArrayError::BroadcastMismatch {
            left: layout.shape.clone(),
            right: target_shape.to_vec(),
        });
    }
    let pad = target_shape.len() - layout.shape.len();
    let mut strides = vec![0isize; target_shape.len()];
    for axis in 0..target_shape.len() {
        if axis < pad {
            strides[axis] = 0; // new leading axis
            continue;
        }
        let src_axis = axis - pad;
        let size = layout.shape[src_axis];
        let target = target_shape[axis];
        if size == target {
            strides[axis] = layout.strides[src_axis];
        } else if size == 1 {
            strides[axis] = 0;
        } else {
            return Err(ArrayError::BroadcastMismatch {
                left: layout.shape.clone(),
                right: target_shape.to_vec(),
            });
        }
    }
    Ok(strides)
}
