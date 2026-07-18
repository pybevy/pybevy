//! Shape, stride, and basic-slice planning. All layout math is pure and
//! interpreter-neutral; nothing here touches storage bytes.

use crate::error::{ArrayError, ArrayResult};

/// Maximum supported rank. Besides matching NumPy's practical bounded-rank
/// model, this bounds recursion in interpreter adapters and stack usage in
/// shape operations.
pub const MAX_NDIM: usize = 64;

/// Product of a shape's dimensions with overflow checking. The empty shape
/// (a 0-d array) has one element.
pub fn checked_num_elements(shape: &[usize]) -> ArrayResult<usize> {
    if shape.len() > MAX_NDIM {
        return Err(ArrayError::TooManyDimensions {
            ndim: shape.len(),
            max: MAX_NDIM,
        });
    }
    let mut n: usize = 1;
    for &d in shape {
        n = n
            .checked_mul(d)
            .ok_or(ArrayError::Overflow("shape element count"))?;
    }
    Ok(n)
}

/// Row-major (C-contiguous) strides in elements for `shape`.
pub fn c_contiguous_strides(shape: &[usize]) -> Vec<isize> {
    let mut strides = vec![0isize; shape.len()];
    let mut acc: usize = 1;
    for i in (0..shape.len()).rev() {
        strides[i] = acc as isize;
        acc = acc.saturating_mul(shape[i]);
    }
    strides
}

/// A basic index applied to one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexOp {
    /// Integer index (may be negative); removes the axis.
    Index(isize),
    /// Basic slice `start:stop:step`; keeps the axis with a new length.
    Slice {
        start: Option<isize>,
        stop: Option<isize>,
        step: isize,
    },
}

impl IndexOp {
    /// A full-axis slice (`:`).
    pub const fn full() -> Self {
        IndexOp::Slice {
            start: None,
            stop: None,
            step: 1,
        }
    }
}

/// Resolve a Python-style slice against an axis of length `size`, returning the
/// first element's index and the number of selected elements. Mirrors
/// CPython's `PySlice_GetIndicesEx`.
fn resolve_slice(
    start: Option<isize>,
    stop: Option<isize>,
    step: isize,
    size: usize,
) -> ArrayResult<(isize, usize)> {
    if step == 0 {
        return Err(ArrayError::ZeroStep);
    }
    let n = size as isize;
    let (lower, upper) = if step < 0 { (-1, n - 1) } else { (0, n) };

    let clamp = |value: isize| -> isize {
        let mut v = value;
        if v < 0 {
            v += n;
            if v < lower {
                v = lower;
            }
        } else if v > upper {
            v = upper;
        }
        v
    };

    let start_i = match start {
        None => {
            if step < 0 {
                upper
            } else {
                lower
            }
        }
        Some(s) => clamp(s),
    };
    let stop_i = match stop {
        None => {
            if step < 0 {
                lower
            } else {
                upper
            }
        }
        Some(s) => clamp(s),
    };

    let length = if step < 0 {
        if stop_i < start_i {
            (start_i - stop_i - 1) / (-step) + 1
        } else {
            0
        }
    } else if start_i < stop_i {
        (stop_i - start_i - 1) / step + 1
    } else {
        0
    };

    Ok((start_i, length as usize))
}

/// Shape, strides (in elements, possibly negative), and a base offset. A
/// `Layout` may describe a base array or a basic-slice view over the same
/// storage; this crate keeps the math, adapters decide copy vs borrow.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub shape: Vec<usize>,
    pub strides: Vec<isize>,
    pub offset: usize,
}

impl Layout {
    /// C-contiguous layout for `shape` at offset 0.
    pub fn c_contiguous(shape: &[usize]) -> ArrayResult<Self> {
        checked_num_elements(shape)?;
        Ok(Layout {
            shape: shape.to_vec(),
            strides: c_contiguous_strides(shape),
            offset: 0,
        })
    }

    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    pub fn num_elements(&self) -> usize {
        checked_num_elements(&self.shape).expect("Layout shape was validated at construction")
    }

    pub fn is_c_contiguous(&self) -> bool {
        self.strides == c_contiguous_strides(&self.shape)
    }

    /// Plan a basic-index selection, producing a sub-layout over the same
    /// storage. Integer ops drop their axis; slice ops keep it. Fewer ops than
    /// dimensions leaves the trailing axes whole.
    pub fn index(&self, ops: &[IndexOp]) -> ArrayResult<Layout> {
        if ops.len() > self.shape.len() {
            return Err(ArrayError::TooManyIndices {
                ndim: self.shape.len(),
                indices: ops.len(),
            });
        }
        let mut offset = self.offset as isize;
        let mut shape = Vec::with_capacity(self.shape.len());
        let mut strides = Vec::with_capacity(self.shape.len());
        for (axis, (&size, &stride)) in self.shape.iter().zip(self.strides.iter()).enumerate() {
            match ops.get(axis) {
                Some(IndexOp::Index(i)) => {
                    let mut idx = *i;
                    if idx < 0 {
                        idx += size as isize;
                    }
                    if idx < 0 || idx >= size as isize {
                        return Err(ArrayError::IndexOutOfBounds {
                            axis,
                            index: *i,
                            size,
                        });
                    }
                    offset += idx * stride;
                }
                Some(IndexOp::Slice { start, stop, step }) => {
                    let (start_i, length) = resolve_slice(*start, *stop, *step, size)?;
                    if length > 0 {
                        offset += start_i * stride;
                    }
                    shape.push(length);
                    strides.push(
                        stride
                            .checked_mul(*step)
                            .ok_or(ArrayError::Overflow("slice stride"))?,
                    );
                }
                None => {
                    shape.push(size);
                    strides.push(stride);
                }
            }
        }
        Ok(Layout {
            shape,
            strides,
            offset: offset.max(0) as usize,
        })
    }

    /// Iterate storage offsets in row-major logical order.
    pub fn iter_offsets(&self) -> OffsetIter<'_> {
        OffsetIter {
            shape: &self.shape,
            strides: &self.strides,
            base: self.offset,
            counter: vec![0; self.shape.len()],
            remaining: self.num_elements(),
        }
    }
}

/// Row-major iterator over the storage offsets a [`Layout`] selects.
pub struct OffsetIter<'a> {
    shape: &'a [usize],
    strides: &'a [isize],
    base: usize,
    counter: Vec<usize>,
    remaining: usize,
}

impl Iterator for OffsetIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.remaining == 0 {
            return None;
        }
        let mut offset = self.base as isize;
        for (k, &c) in self.counter.iter().enumerate() {
            offset += c as isize * self.strides[k];
        }
        for k in (0..self.shape.len()).rev() {
            self.counter[k] += 1;
            if self.counter[k] < self.shape[k] {
                break;
            }
            self.counter[k] = 0;
        }
        self.remaining -= 1;
        Some(offset as usize)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
