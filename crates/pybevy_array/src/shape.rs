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

    // Divide by |step| as unsigned: negating isize::MIN overflows, and a[::-2**63] is valid Python.
    let magnitude = step.unsigned_abs();
    let length = if step < 0 {
        if stop_i < start_i {
            (start_i - stop_i - 1) as usize / magnitude + 1
        } else {
            0
        }
    } else if start_i < stop_i {
        (stop_i - start_i - 1) as usize / magnitude + 1
    } else {
        0
    };

    Ok((start_i, length))
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
        if self.shape.contains(&0) {
            return true;
        }
        let mut expected = 1_i128;
        for (&size, &stride) in self.shape.iter().zip(&self.strides).rev() {
            if size > 1 && stride as i128 != expected {
                return false;
            }
            let Ok(size) = i128::try_from(size) else {
                return false;
            };
            let Some(next) = expected.checked_mul(size) else {
                return false;
            };
            expected = next;
        }
        true
    }

    /// Validate that every offset selected by this layout is within a backing
    /// allocation of `storage_len` elements.
    pub fn validate_bounds(&self, storage_len: usize) -> ArrayResult<()> {
        if self.shape.len() != self.strides.len() {
            return Err(ArrayError::InvalidLayout(
                "shape and stride ranks do not match",
            ));
        }
        let elements = checked_num_elements(&self.shape)?;
        if elements == 0 {
            if self.offset <= storage_len {
                return Ok(());
            }
            return Err(ArrayError::LayoutOutOfBounds { storage_len });
        }

        let mut minimum =
            i128::try_from(self.offset).map_err(|_| ArrayError::Overflow("layout base offset"))?;
        let mut maximum = minimum;
        for (&size, &stride) in self.shape.iter().zip(&self.strides) {
            let count =
                i128::try_from(size - 1).map_err(|_| ArrayError::Overflow("layout axis length"))?;
            let span = count
                .checked_mul(stride as i128)
                .ok_or(ArrayError::Overflow("layout stride span"))?;
            if span < 0 {
                minimum = minimum
                    .checked_add(span)
                    .ok_or(ArrayError::Overflow("layout minimum offset"))?;
            } else {
                maximum = maximum
                    .checked_add(span)
                    .ok_or(ArrayError::Overflow("layout maximum offset"))?;
            }
        }

        if minimum < 0
            || usize::try_from(maximum)
                .ok()
                .is_none_or(|offset| offset >= storage_len)
        {
            return Err(ArrayError::LayoutOutOfBounds { storage_len });
        }
        Ok(())
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
                    strides.push(match stride.checked_mul(*step) {
                        Some(product) => product,
                        // Unused for a length<=1 axis; keep the parent stride instead of erroring.
                        None if length <= 1 => stride,
                        None => return Err(ArrayError::Overflow("slice stride")),
                    });
                }
                None => {
                    shape.push(size);
                    strides.push(stride);
                }
            }
        }
        // Empty selections address no element; keep their base within any backing allocation.
        let offset = if shape.contains(&0) {
            0
        } else {
            usize::try_from(offset).map_err(|_| ArrayError::Overflow("layout offset"))?
        };
        Ok(Layout {
            shape,
            strides,
            offset,
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn num_elements_of_empty_shape_is_one() {
        assert_eq!(checked_num_elements(&[]).unwrap(), 1);
        assert_eq!(Layout::c_contiguous(&[]).unwrap().num_elements(), 1);
    }

    #[test]
    fn overflow_is_reported() {
        let err = checked_num_elements(&[usize::MAX, 2]).unwrap_err();
        assert!(matches!(err, ArrayError::Overflow(_)));
    }

    #[test]
    fn rank_above_limit_is_reported() {
        let shape = vec![1; MAX_NDIM + 1];
        assert!(matches!(
            checked_num_elements(&shape),
            Err(ArrayError::TooManyDimensions { .. })
        ));
    }

    #[test]
    fn c_contiguous_strides_2x3() {
        assert_eq!(c_contiguous_strides(&[2, 3]), vec![3, 1]);
        assert_eq!(c_contiguous_strides(&[2, 3, 4]), vec![12, 4, 1]);
    }

    #[test]
    fn contiguity_ignores_empty_and_singleton_axis_strides() {
        assert!(
            Layout {
                shape: vec![0, 3],
                strides: vec![-100, 7],
                offset: 0,
            }
            .is_c_contiguous()
        );
        assert!(
            Layout {
                shape: vec![1, 3],
                strides: vec![100, 1],
                offset: 0,
            }
            .is_c_contiguous()
        );
        assert!(
            !Layout {
                shape: vec![3, 1],
                strides: vec![2, 100],
                offset: 0,
            }
            .is_c_contiguous()
        );
    }

    #[test]
    fn contiguous_offsets_are_sequential() {
        let l = Layout::c_contiguous(&[2, 3]).unwrap();
        let offsets: Vec<usize> = l.iter_offsets().collect();
        assert_eq!(offsets, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn integer_index_drops_axis() {
        let l = Layout::c_contiguous(&[2, 3]).unwrap();
        let row = l.index(&[IndexOp::Index(1)]).unwrap();
        assert_eq!(row.shape, vec![3]);
        assert_eq!(row.iter_offsets().collect::<Vec<_>>(), vec![3, 4, 5]);
    }

    #[test]
    fn negative_index_wraps() {
        let l = Layout::c_contiguous(&[4]).unwrap();
        let v = l.index(&[IndexOp::Index(-1)]).unwrap();
        assert_eq!(v.shape, Vec::<usize>::new());
        assert_eq!(v.iter_offsets().collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn out_of_bounds_index_errors() {
        let l = Layout::c_contiguous(&[3]).unwrap();
        assert!(matches!(
            l.index(&[IndexOp::Index(3)]),
            Err(ArrayError::IndexOutOfBounds { .. })
        ));
        assert!(matches!(
            l.index(&[IndexOp::Index(-4)]),
            Err(ArrayError::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn column_view_strides() {
        // arr[:, 1] over a 2x3 array -> offsets 1, 4
        let l = Layout::c_contiguous(&[2, 3]).unwrap();
        let col = l.index(&[IndexOp::full(), IndexOp::Index(1)]).unwrap();
        assert_eq!(col.shape, vec![2]);
        assert_eq!(col.strides, vec![3]);
        assert_eq!(col.iter_offsets().collect::<Vec<_>>(), vec![1, 4]);
    }

    #[test]
    fn strided_slice() {
        // arr[1:6:2] over length 8 -> indices 1,3,5
        let l = Layout::c_contiguous(&[8]).unwrap();
        let s = l
            .index(&[IndexOp::Slice {
                start: Some(1),
                stop: Some(6),
                step: 2,
            }])
            .unwrap();
        assert_eq!(s.shape, vec![3]);
        assert_eq!(s.iter_offsets().collect::<Vec<_>>(), vec![1, 3, 5]);
    }

    #[test]
    fn negative_step_reverses() {
        // arr[::-1] over length 4 -> 3,2,1,0
        let l = Layout::c_contiguous(&[4]).unwrap();
        let s = l
            .index(&[IndexOp::Slice {
                start: None,
                stop: None,
                step: -1,
            }])
            .unwrap();
        assert_eq!(s.shape, vec![4]);
        assert_eq!(s.strides, vec![-1]);
        assert_eq!(s.iter_offsets().collect::<Vec<_>>(), vec![3, 2, 1, 0]);
    }

    #[test]
    fn empty_slice_has_no_offsets() {
        let l = Layout::c_contiguous(&[4]).unwrap();
        let s = l
            .index(&[IndexOp::Slice {
                start: Some(3),
                stop: Some(1),
                step: 1,
            }])
            .unwrap();
        assert_eq!(s.shape, vec![0]);
        assert_eq!(s.iter_offsets().count(), 0);
    }

    #[test]
    fn extreme_negative_step_selects_one_element() {
        // `-isize::MIN` overflows, so the length must divide by the unsigned
        // magnitude instead of negating.
        let l = Layout::c_contiguous(&[8]).unwrap();
        let s = l
            .index(&[IndexOp::Slice {
                start: None,
                stop: None,
                step: isize::MIN,
            }])
            .unwrap();
        assert_eq!(s.shape, vec![1]);
        assert_eq!(s.iter_offsets().collect::<Vec<_>>(), vec![7]);
    }

    #[test]
    fn extreme_step_on_a_wide_stride_axis_keeps_the_row() {
        // `stride * step` overflows for the outer axis of a 4x3 array, but the
        // one selected row makes that product unreachable.
        let l = Layout::c_contiguous(&[4, 3]).unwrap();
        let s = l
            .index(&[IndexOp::Slice {
                start: None,
                stop: None,
                step: isize::MIN,
            }])
            .unwrap();
        assert_eq!(s.shape, vec![1, 3]);
        assert_eq!(s.iter_offsets().collect::<Vec<_>>(), vec![9, 10, 11]);
    }

    #[test]
    fn extreme_positive_step_selects_first_element() {
        let l = Layout::c_contiguous(&[8]).unwrap();
        let s = l
            .index(&[IndexOp::Slice {
                start: None,
                stop: None,
                step: isize::MAX,
            }])
            .unwrap();
        assert_eq!(s.shape, vec![1]);
        assert_eq!(s.iter_offsets().collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn zero_step_errors() {
        let l = Layout::c_contiguous(&[4]).unwrap();
        assert!(matches!(
            l.index(&[IndexOp::Slice {
                start: None,
                stop: None,
                step: 0
            }]),
            Err(ArrayError::ZeroStep)
        ));
    }

    #[test]
    fn too_many_indices_errors() {
        let l = Layout::c_contiguous(&[4]).unwrap();
        assert!(matches!(
            l.index(&[IndexOp::Index(0), IndexOp::Index(0)]),
            Err(ArrayError::TooManyIndices { .. })
        ));
    }

    #[test]
    fn empty_index_selections_keep_a_valid_storage_base() {
        let cases = [
            (
                vec![0, 3],
                vec![
                    IndexOp::Slice {
                        start: None,
                        stop: None,
                        step: 1,
                    },
                    IndexOp::Slice {
                        start: Some(1),
                        stop: None,
                        step: 1,
                    },
                ],
                vec![0, 2],
            ),
            (
                vec![2, 0, 3],
                vec![
                    IndexOp::Slice {
                        start: None,
                        stop: None,
                        step: 1,
                    },
                    IndexOp::Slice {
                        start: None,
                        stop: None,
                        step: 1,
                    },
                    IndexOp::Slice {
                        start: None,
                        stop: None,
                        step: -1,
                    },
                ],
                vec![2, 0, 3],
            ),
        ];
        for (shape, ops, expected) in cases {
            let result = Layout::c_contiguous(&shape).unwrap().index(&ops).unwrap();
            assert_eq!(result.shape, expected);
            assert_eq!(result.offset, 0);
            assert!(result.validate_bounds(0).is_ok());
            assert_eq!(result.iter_offsets().count(), 0);
        }
    }

    #[test]
    fn layout_bounds_accept_negative_strides_with_a_valid_base() {
        let layout = Layout {
            shape: vec![3],
            strides: vec![-1],
            offset: 2,
        };
        assert!(layout.validate_bounds(3).is_ok());
    }

    #[test]
    fn layout_bounds_accept_zero_strides_and_empty_end_offsets() {
        let broadcast = Layout {
            shape: vec![3],
            strides: vec![0],
            offset: 1,
        };
        assert!(broadcast.validate_bounds(2).is_ok());

        let empty = Layout {
            shape: vec![0],
            strides: vec![isize::MIN],
            offset: 2,
        };
        assert!(empty.validate_bounds(2).is_ok());
    }

    #[test]
    fn layout_bounds_reject_mismatched_rank_and_escaped_offsets() {
        let mismatched = Layout {
            shape: vec![2],
            strides: vec![],
            offset: 0,
        };
        assert!(matches!(
            mismatched.validate_bounds(2),
            Err(ArrayError::InvalidLayout(_))
        ));

        let escaped = Layout {
            shape: vec![2],
            strides: vec![1],
            offset: 1,
        };
        assert!(matches!(
            escaped.validate_bounds(2),
            Err(ArrayError::LayoutOutOfBounds { storage_len: 2 })
        ));

        let escaped_empty = Layout {
            shape: vec![0],
            strides: vec![1],
            offset: 3,
        };
        assert!(matches!(
            escaped_empty.validate_bounds(2),
            Err(ArrayError::LayoutOutOfBounds { storage_len: 2 })
        ));
    }
}
