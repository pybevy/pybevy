//! `DenseArrayCore`: shared storage plus a validated per-array layout.
//! Element-wise numeric execution lives in `pybevy_bytecodevm`; this type owns
//! construction, view planning, casting, and copy materialization.

use std::sync::Arc;

use crate::{
    backing::{ArrayBacking, ArrayReadGuard, ArrayWriteGuard},
    dtype::ArrayDType,
    error::{ArrayError, ArrayResult},
    scalar::Scalar,
    shape::{IndexOp, Layout, c_contiguous_strides, checked_num_elements},
    storage::ArrayStorage,
};

/// A complete- or single-axis reduction operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisReduce {
    Sum,
    Mean,
    Min,
    Max,
    All,
    Any,
}

#[derive(Debug)]
pub struct DenseArrayCore {
    backing: Arc<ArrayBacking>,
    layout: Layout,
    writable: bool,
    is_view: bool,
}

impl DenseArrayCore {
    /// Wrap owned `storage` with a C-contiguous `shape`. Errors if the storage
    /// length does not equal the shape's element count.
    pub fn from_storage(storage: ArrayStorage, shape: &[usize]) -> ArrayResult<Self> {
        let elements = checked_num_elements(shape)?;
        if storage.len() != elements {
            return Err(ArrayError::StorageLengthMismatch {
                shape_elements: elements,
                storage_len: storage.len(),
            });
        }
        // Read-only borrows are frozen; owned and mutable-borrow storage is
        // writable.
        let writable = !storage.is_read_only_borrow();
        Ok(DenseArrayCore {
            backing: ArrayBacking::new(storage),
            layout: Layout::c_contiguous(shape)?,
            writable,
            is_view: false,
        })
    }

    /// Verify the array's data is readable for the current operation (owned data
    /// always is; borrowed data consults its liveness probe). Adapters call this
    /// once at the start of each read operation to surface a clean error.
    pub fn ensure_readable(&self) -> ArrayResult<()> {
        drop(self.read_storage()?);
        Ok(())
    }

    /// Verify the array is writable for the current operation: not frozen, and
    /// (for a mutable borrow) still permitted by the probe on this thread.
    pub fn ensure_writable(&self) -> ArrayResult<()> {
        if !self.writable {
            return Err(ArrayError::NotWritable);
        }
        drop(self.write_storage()?);
        Ok(())
    }

    /// Construct directly from a validated layout and storage (offsets in the
    /// layout must stay within storage). Used by adapters that build views.
    pub fn from_parts(storage: ArrayStorage, layout: Layout, writable: bool) -> ArrayResult<Self> {
        layout.validate_bounds(storage.len())?;
        Ok(DenseArrayCore {
            backing: ArrayBacking::new(storage),
            layout,
            writable,
            is_view: false,
        })
    }

    pub fn zeros(dtype: ArrayDType, shape: &[usize]) -> ArrayResult<Self> {
        let elements = checked_num_elements(shape)?;
        Self::from_storage(ArrayStorage::zeros(dtype, elements)?, shape)
    }

    pub fn ones(dtype: ArrayDType, shape: &[usize]) -> ArrayResult<Self> {
        Self::full(dtype, shape, Scalar::I64(1))
    }

    pub fn full(dtype: ArrayDType, shape: &[usize], value: Scalar) -> ArrayResult<Self> {
        let elements = checked_num_elements(shape)?;
        Self::from_storage(ArrayStorage::filled(dtype, elements, value)?, shape)
    }

    /// `empty` zero-fills and never exposes uninitialized memory.
    pub fn empty(dtype: ArrayDType, shape: &[usize]) -> ArrayResult<Self> {
        Self::zeros(dtype, shape)
    }

    /// Construct a one-dimensional half-open arithmetic progression. Integer
    /// arguments stay in the integer domain for both length calculation and
    /// element generation, so values above f64's exact range are preserved.
    pub fn arange(
        start: Scalar,
        stop: Scalar,
        step: Scalar,
        dtype: ArrayDType,
    ) -> ArrayResult<Self> {
        if dtype.is_integer()
            && let (Some(start), Some(stop), Some(step)) = (
                scalar_integer(start),
                scalar_integer(stop),
                scalar_integer(step),
            )
        {
            if step == 0 {
                return Err(ArrayError::ZeroStep);
            }
            let count = integer_range_len(start, stop, step)?;
            let mut storage = ArrayStorage::zeros(dtype, count)?;
            for i in 0..count {
                let value = i128::from(start) + i as i128 * i128::from(step);
                let value =
                    i64::try_from(value).map_err(|_| ArrayError::Overflow("arange element"))?;
                storage.set(i, Scalar::I64(value));
            }
            return DenseArrayCore::from_storage(storage, &[count]);
        }

        let (start, stop, step) = (start.to_f64(), stop.to_f64(), step.to_f64());
        if step == 0.0 {
            return Err(ArrayError::ZeroStep);
        }
        let raw_count = ((stop - start) / step).ceil();
        // f64::max would turn a NaN length into an empty array.
        if raw_count.is_nan() {
            return Err(ArrayError::NanRangeLength);
        }
        let raw_count = raw_count.max(0.0);
        if !raw_count.is_finite() || raw_count > usize::MAX as f64 {
            return Err(ArrayError::Overflow("arange length"));
        }
        let count = raw_count as usize;
        let mut storage = ArrayStorage::zeros(dtype, count)?;
        for i in 0..count {
            storage.set(i, Scalar::F64(start + i as f64 * step));
        }
        DenseArrayCore::from_storage(storage, &[count])
    }

    pub fn dtype(&self) -> ArrayDType {
        self.backing.dtype()
    }

    pub fn shape(&self) -> &[usize] {
        &self.layout.shape
    }

    pub fn strides(&self) -> &[isize] {
        &self.layout.strides
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub(crate) fn read_storage(&self) -> ArrayResult<ArrayReadGuard> {
        self.backing.try_read()
    }

    /// Mutable storage access for offset-addressed writes (e.g. slice
    /// assignment). Errors if the array is read-only. Callers must keep offsets
    /// within storage; plan them through [`Self::plan`].
    pub(crate) fn write_storage(&self) -> ArrayResult<ArrayWriteGuard> {
        if !self.writable {
            return Err(ArrayError::NotWritable);
        }
        self.backing.try_write()
    }

    /// Acquire the backing's exclusive operation gate without applying the
    /// array's logical writability policy. DLPack export uses this to prevent
    /// every PyBevy read and write while a foreign consumer owns the pointer;
    /// the protocol's read-only flag is advisory and cannot provide exclusion.
    #[cfg(feature = "pyo3")]
    pub(crate) fn exclusive_storage(&self) -> ArrayResult<ArrayWriteGuard> {
        self.backing.try_write()
    }

    /// Cast and write every logical element from an equally-shaped owned
    /// result into this array. The source is materialized before destination
    /// mutation, which keeps self-aliasing and overlapping borrowed views safe.
    pub fn assign_from(&mut self, source: &DenseArrayCore) -> ArrayResult<()> {
        source.ensure_readable()?;
        if self.shape() != source.shape() {
            return Err(ArrayError::BroadcastMismatch {
                left: self.shape().to_vec(),
                right: source.shape().to_vec(),
            });
        }
        let values = source.to_scalars()?;
        let layout = self.layout.clone();
        let mut storage = self.write_storage()?;
        for (offset, value) in layout.iter_offsets().zip(values) {
            storage.set(offset, value);
        }
        Ok(())
    }

    /// Write `bytes` into the `uint8` elements selected by `layout` (a plan
    /// from [`Self::plan`]) in logical order. A C-contiguous selection is one
    /// bulk copy; other selections are written element by element.
    pub fn assign_u8_bytes(&self, layout: &Layout, bytes: &[u8]) -> ArrayResult<()> {
        let dtype = self.dtype();
        if dtype != ArrayDType::Uint8 {
            return Err(ArrayError::UnsupportedDType {
                op: "byte assignment",
                dtype,
            });
        }
        if layout.num_elements() != bytes.len() {
            return Err(ArrayError::ByteLengthMismatch {
                dtype,
                expected: layout.num_elements(),
                actual: bytes.len(),
            });
        }
        layout.validate_bounds(self.backing.len())?;
        let mut storage = self.write_storage()?;
        if layout.is_c_contiguous() && storage.write_u8_range(layout.offset, bytes) {
            return Ok(());
        }
        for (offset, &byte) in layout.iter_offsets().zip(bytes) {
            storage.set(offset, Scalar::I64(i64::from(byte)));
        }
        Ok(())
    }

    pub fn ndim(&self) -> usize {
        self.layout.ndim()
    }

    pub fn size(&self) -> usize {
        self.layout.num_elements()
    }

    pub fn itemsize(&self) -> usize {
        self.dtype().itemsize()
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn is_c_contiguous(&self) -> bool {
        self.layout.is_c_contiguous()
    }

    /// Whether this array was created by a storage-sharing view operation.
    pub fn is_view(&self) -> bool {
        self.is_view
    }

    pub fn set_read_only(&mut self) {
        self.writable = false;
    }

    /// Create another layout over the same backing storage.
    pub fn view(&self, layout: Layout) -> ArrayResult<Self> {
        self.ensure_readable()?;
        layout.validate_bounds(self.backing.len())?;
        Ok(Self {
            backing: self.backing.clone(),
            layout,
            writable: self.writable,
            is_view: true,
        })
    }

    fn flat_offset(&self, indices: &[usize]) -> ArrayResult<usize> {
        if indices.len() != self.layout.ndim() {
            return Err(ArrayError::TooManyIndices {
                ndim: self.layout.ndim(),
                indices: indices.len(),
            });
        }
        let mut offset = self.layout.offset as isize;
        for (axis, ((&index, &size), &stride)) in indices
            .iter()
            .zip(self.layout.shape.iter())
            .zip(self.layout.strides.iter())
            .enumerate()
        {
            if index >= size {
                return Err(ArrayError::IndexOutOfBounds {
                    axis,
                    index: index as isize,
                    size,
                });
            }
            offset += index as isize * stride;
        }
        Ok(offset as usize)
    }

    /// Read one element by full multi-index.
    pub fn get(&self, indices: &[usize]) -> ArrayResult<Scalar> {
        let storage = self.read_storage()?;
        Ok(storage.get(self.flat_offset(indices)?))
    }

    /// Write one element by full multi-index. Errors if read-only or if a
    /// mutable borrow's probe rejects the write (e.g. after the system ends).
    pub fn set(&mut self, indices: &[usize], value: Scalar) -> ArrayResult<()> {
        let mut storage = self.write_storage()?;
        let offset = self.flat_offset(indices)?;
        storage.set(offset, value);
        Ok(())
    }

    /// Plan a basic-index selection without moving data. Adapters apply the
    /// plan either by copying ([`Self::slice_copy`]) or by borrowing.
    pub fn plan(&self, ops: &[IndexOp]) -> ArrayResult<Layout> {
        self.layout.index(ops)
    }

    /// All elements in row-major order as neutral scalars.
    pub fn to_scalars(&self) -> ArrayResult<Vec<Scalar>> {
        let storage = self.read_storage()?;
        Ok(self
            .layout
            .iter_offsets()
            .map(|off| storage.get(off))
            .collect())
    }

    fn materialize(&self, layout: &Layout) -> ArrayResult<DenseArrayCore> {
        let storage = self.read_storage()?;
        let dtype = self.dtype();
        let mut out = ArrayStorage::zeros(dtype, layout.num_elements())?;
        for (i, off) in layout.iter_offsets().enumerate() {
            if dtype == ArrayDType::Float16 {
                out.set_float16_bits(
                    i,
                    storage
                        .float16_bits(off)
                        .expect("float16 dtype has float16 storage"),
                );
            } else {
                out.set(i, storage.get(off));
            }
        }
        DenseArrayCore::from_storage(out, &layout.shape)
    }

    /// A C-contiguous, writable, independent copy of this array's elements.
    pub fn copy(&self) -> ArrayResult<DenseArrayCore> {
        self.materialize(&self.layout)
    }

    /// Copy the selection described by `ops` into a new contiguous array.
    pub fn slice_copy(&self, ops: &[IndexOp]) -> ArrayResult<DenseArrayCore> {
        let plan = self.layout.index(ops)?;
        self.materialize(&plan)
    }

    /// Share the selection described by `ops` with this array's backing.
    pub fn slice_view(&self, ops: &[IndexOp]) -> ArrayResult<DenseArrayCore> {
        self.view(self.layout.index(ops)?)
    }

    /// Cast to `dtype`, producing a new contiguous array (NumPy `astype`).
    pub fn astype(&self, dtype: ArrayDType) -> ArrayResult<DenseArrayCore> {
        if dtype == self.dtype() {
            return self.copy();
        }
        let storage = self.read_storage()?;
        let mut out = ArrayStorage::zeros(dtype, self.size())?;
        for (i, off) in self.layout.iter_offsets().enumerate() {
            out.set(i, storage.get(off));
        }
        DenseArrayCore::from_storage(out, &self.layout.shape)
    }

    /// Reshape to `new_shape` (same element count). C-contiguous arrays share
    /// their backing storage; other layouts produce a writable contiguous copy.
    pub fn reshape(&self, new_shape: &[usize]) -> ArrayResult<DenseArrayCore> {
        let want = checked_num_elements(new_shape)?;
        if want != self.size() {
            return Err(ArrayError::ReshapeMismatch {
                from: self.layout.shape.clone(),
                to: new_shape.to_vec(),
            });
        }
        let mut layout = Layout::c_contiguous(new_shape)?;
        if self.is_c_contiguous() {
            layout.offset = self.layout.offset;
            self.view(layout)
        } else {
            let mut copied = self.copy()?;
            copied.layout = layout;
            Ok(copied)
        }
    }

    /// Flatten to one dimension, sharing C-contiguous storage when possible.
    pub fn ravel(&self) -> ArrayResult<DenseArrayCore> {
        let n = self.size();
        self.reshape(&[n])
    }

    /// Reduce along a single axis, removing it. `Sum`/`Mean` keep the input
    /// float dtype, `Min`/`Max` keep the input dtype, and `All`/`Any` produce
    /// `bool`. Callers restrict `Sum`/`Mean` to float dtypes.
    pub fn reduce_axis(&self, axis: usize, op: AxisReduce) -> ArrayResult<DenseArrayCore> {
        let shape = self.layout.shape.clone();
        if axis >= shape.len() {
            return Err(ArrayError::AxisOutOfBounds {
                axis,
                ndim: shape.len(),
            });
        }
        // Min/Max have no identity for an empty axis; NumPy raises rather than producing a value.
        if shape[axis] == 0 && matches!(op, AxisReduce::Min | AxisReduce::Max) {
            return Err(ArrayError::ZeroSizeReduction);
        }
        let scalars = self.to_scalars()?; // row-major
        let strides = c_contiguous_strides(&shape);
        let axis_len = shape[axis];
        let axis_stride = strides[axis] as usize;

        let mut result_shape = shape.clone();
        result_shape.remove(axis);
        let out_n = checked_num_elements(&result_shape)?;
        let out_dtype = axis_out_dtype(self.dtype(), op);
        let mut out = ArrayStorage::zeros(out_dtype, out_n)?;

        let mut counter = vec![0usize; result_shape.len()];
        for position in 0..out_n {
            // Reconstruct the full input index by inserting 0 at `axis`.
            let mut base = 0usize;
            let mut ri = 0;
            for (d, &stride) in strides.iter().enumerate() {
                if d == axis {
                    continue;
                }
                base += counter[ri] * stride as usize;
                ri += 1;
            }
            let group: Vec<Scalar> = (0..axis_len)
                .map(|j| scalars[base + j * axis_stride])
                .collect();
            out.set(position, reduce_group(&group, op));

            for k in (0..result_shape.len()).rev() {
                counter[k] += 1;
                if counter[k] < result_shape[k] {
                    break;
                }
                counter[k] = 0;
            }
        }
        DenseArrayCore::from_storage(out, &result_shape)
    }

    fn validate_mask(&self, mask: &[bool], mask_shape: &[usize]) -> ArrayResult<()> {
        if mask_shape != self.shape() {
            return Err(ArrayError::MaskShapeMismatch {
                mask: mask_shape.to_vec(),
                array: self.shape().to_vec(),
            });
        }
        if mask.len() != self.size() {
            return Err(ArrayError::MaskLengthMismatch {
                mask_len: mask.len(),
                size: self.size(),
            });
        }
        Ok(())
    }

    /// Select the elements where `mask` is true, returning a 1-D array. The
    /// mask shape must equal the array shape.
    pub fn mask_select(&self, mask: &[bool], mask_shape: &[usize]) -> ArrayResult<DenseArrayCore> {
        self.validate_mask(mask, mask_shape)?;
        if self.dtype() == ArrayDType::Float16 {
            let source = self.read_storage()?;
            let selected = self
                .layout
                .iter_offsets()
                .zip(mask)
                .filter(|(_, keep)| **keep)
                .map(|(offset, _)| {
                    source
                        .float16_bits(offset)
                        .expect("float16 dtype has float16 storage")
                })
                .collect::<Vec<_>>();
            let selected_len = selected.len();
            return DenseArrayCore::from_storage(ArrayStorage::Float16(selected), &[selected_len]);
        }
        let scalars = self.to_scalars()?;
        let selected: Vec<Scalar> = scalars
            .iter()
            .zip(mask)
            .filter_map(|(&s, &m)| m.then_some(s))
            .collect();
        let mut storage = ArrayStorage::zeros(self.dtype(), selected.len())?;
        for (i, &s) in selected.iter().enumerate() {
            storage.set(i, s);
        }
        DenseArrayCore::from_storage(storage, &[selected.len()])
    }

    /// Assign into the elements where `mask` is true. `values` is either a
    /// single broadcast scalar or one value per selected element (row-major).
    pub fn mask_assign(
        &mut self,
        mask: &[bool],
        mask_shape: &[usize],
        values: &[Scalar],
    ) -> ArrayResult<()> {
        // Gate on the probe first so an expired mutable borrow reports that, not a shape error.
        self.ensure_writable()?;
        self.validate_mask(mask, mask_shape)?;
        let selected = mask.iter().filter(|&&item| item).count();
        if values.len() != 1 && values.len() != selected {
            return Err(ArrayError::MaskValueCountMismatch {
                values_len: values.len(),
                selected,
            });
        }
        let offsets = self.layout.iter_offsets();
        let mut storage = self.write_storage()?;
        let mut vi = 0;
        for (offset, &m) in offsets.zip(mask) {
            if m {
                let value = if values.len() == 1 {
                    values[0]
                } else {
                    values[vi]
                };
                storage.set(offset, value);
                vi += 1;
            }
        }
        Ok(())
    }
}

fn scalar_integer(value: Scalar) -> Option<i64> {
    match value {
        Scalar::I64(value) => Some(value),
        Scalar::Bool(value) => Some(i64::from(value)),
        Scalar::F64(_) => None,
    }
}

fn integer_range_len(start: i64, stop: i64, step: i64) -> ArrayResult<usize> {
    let (start, stop, step) = (i128::from(start), i128::from(stop), i128::from(step));
    let count = if step > 0 {
        if start >= stop {
            0
        } else {
            (stop - start - 1) / step + 1
        }
    } else if start <= stop {
        0
    } else {
        (start - stop - 1) / -step + 1
    };
    usize::try_from(count).map_err(|_| ArrayError::Overflow("arange length"))
}

fn axis_out_dtype(input: ArrayDType, op: AxisReduce) -> ArrayDType {
    match op {
        AxisReduce::All | AxisReduce::Any => ArrayDType::Bool,
        AxisReduce::Sum | AxisReduce::Mean | AxisReduce::Min | AxisReduce::Max
            if input == ArrayDType::Float16 =>
        {
            ArrayDType::Float32
        }
        AxisReduce::Sum | AxisReduce::Mean | AxisReduce::Min | AxisReduce::Max => input,
    }
}

fn reduce_group(group: &[Scalar], op: AxisReduce) -> Scalar {
    match op {
        AxisReduce::All => Scalar::Bool(group.iter().all(|s| s.to_bool())),
        AxisReduce::Any => Scalar::Bool(group.iter().any(|s| s.to_bool())),
        AxisReduce::Sum => Scalar::F64(group.iter().map(|s| s.to_f64()).sum()),
        AxisReduce::Mean => {
            let total: f64 = group.iter().map(|s| s.to_f64()).sum();
            Scalar::F64(total / group.len() as f64)
        }
        AxisReduce::Min | AxisReduce::Max => {
            let want_max = op == AxisReduce::Max;
            if matches!(group[0], Scalar::F64(_)) {
                let vals = group.iter().map(|s| s.to_f64());
                let mut best = f64::NAN;
                let mut first = true;
                for v in vals {
                    if v.is_nan() {
                        return Scalar::F64(f64::NAN); // NumPy min/max propagate NaN
                    }
                    if first || (want_max && v > best) || (!want_max && v < best) {
                        best = v;
                        first = false;
                    }
                }
                Scalar::F64(best)
            } else {
                let vals = group.iter().map(|s| s.to_i64_trunc());
                let best = if want_max {
                    vals.max().expect("non-empty group")
                } else {
                    vals.min().expect("non-empty group")
                };
                Scalar::I64(best)
            }
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;
    use crate::storage::{ArrayStorage, BorrowProbe};

    fn f64_array(data: &[f64], shape: &[usize]) -> DenseArrayCore {
        DenseArrayCore::from_storage(ArrayStorage::Float64(data.to_vec()), shape).unwrap()
    }

    #[test]
    fn arange_rejects_a_nan_bound_or_step() {
        for (start, stop, step) in [
            (f64::NAN, 1.0, 1.0),
            (0.0, f64::NAN, 1.0),
            (0.0, 5.0, f64::NAN),
        ] {
            let result = DenseArrayCore::arange(
                Scalar::F64(start),
                Scalar::F64(stop),
                Scalar::F64(step),
                ArrayDType::Float64,
            );
            assert_eq!(result.err(), Some(ArrayError::NanRangeLength));
        }
    }

    #[test]
    fn from_storage_validates_length() {
        let err = DenseArrayCore::from_storage(ArrayStorage::Float64(vec![1.0; 5]), &[2, 3]);
        assert!(matches!(err, Err(ArrayError::StorageLengthMismatch { .. })));
    }

    #[test]
    fn zeros_shape_and_dtype() {
        let a = DenseArrayCore::zeros(ArrayDType::Float32, &[2, 3]).unwrap();
        assert_eq!(a.shape(), &[2, 3]);
        assert_eq!(a.dtype(), ArrayDType::Float32);
        assert_eq!(a.size(), 6);
        assert_eq!(a.itemsize(), 4);
        assert!(
            a.to_scalars()
                .unwrap()
                .iter()
                .all(|&s| s == Scalar::F64(0.0))
        );
    }

    #[test]
    fn integer_arange_is_exact_above_f64_range() {
        let start = 9_007_199_254_740_992_i64;
        let array = DenseArrayCore::arange(
            Scalar::I64(start),
            Scalar::I64(start + 3),
            Scalar::I64(1),
            ArrayDType::Int64,
        )
        .unwrap();
        assert_eq!(
            array.to_scalars().unwrap(),
            vec![
                Scalar::I64(start),
                Scalar::I64(start + 1),
                Scalar::I64(start + 2)
            ]
        );
    }

    #[test]
    fn integer_arange_handles_negative_steps() {
        let array = DenseArrayCore::arange(
            Scalar::I64(5),
            Scalar::I64(-2),
            Scalar::I64(-2),
            ArrayDType::Int64,
        )
        .unwrap();
        assert_eq!(
            array.to_scalars().unwrap(),
            vec![
                Scalar::I64(5),
                Scalar::I64(3),
                Scalar::I64(1),
                Scalar::I64(-1)
            ]
        );
    }

    #[test]
    fn get_set_roundtrip() {
        let mut a = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        assert_eq!(a.get(&[1, 2]).unwrap(), Scalar::F64(6.0));
        a.set(&[0, 0], Scalar::F64(9.0)).unwrap();
        assert_eq!(a.get(&[0, 0]).unwrap(), Scalar::F64(9.0));
    }

    #[test]
    fn read_only_rejects_set() {
        let mut a = f64_array(&[1.0, 2.0], &[2]);
        a.set_read_only();
        assert!(matches!(
            a.set(&[0], Scalar::F64(0.0)),
            Err(ArrayError::NotWritable)
        ));
    }

    #[test]
    fn slice_copy_column_is_independent() {
        let base = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let mut col = base
            .slice_copy(&[IndexOp::full(), IndexOp::Index(1)])
            .unwrap();
        assert_eq!(col.shape(), &[2]);
        assert_eq!(
            col.to_scalars().unwrap(),
            vec![Scalar::F64(2.0), Scalar::F64(5.0)]
        );
        // Mutating the copy must not touch the base (no aliasing).
        col.set(&[0], Scalar::F64(99.0)).unwrap();
        assert_eq!(base.get(&[0, 1]).unwrap(), Scalar::F64(2.0));
    }

    #[test]
    fn slice_view_column_shares_storage() {
        let mut base = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let mut column = base
            .slice_view(&[IndexOp::full(), IndexOp::Index(1)])
            .unwrap();

        column.set(&[0], Scalar::F64(20.0)).unwrap();
        base.set(&[1, 1], Scalar::F64(50.0)).unwrap();

        assert_eq!(base.get(&[0, 1]).unwrap(), Scalar::F64(20.0));
        assert_eq!(column.get(&[1]).unwrap(), Scalar::F64(50.0));
    }

    #[test]
    fn nested_slice_views_compose_signed_strides() {
        let base = f64_array(
            &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0],
            &[3, 4],
        );
        let every_other_column = base
            .slice_view(&[
                IndexOp::full(),
                IndexOp::Slice {
                    start: None,
                    stop: None,
                    step: 2,
                },
            ])
            .unwrap();
        let mut reversed_row = every_other_column
            .slice_view(&[
                IndexOp::Index(1),
                IndexOp::Slice {
                    start: None,
                    stop: None,
                    step: -1,
                },
            ])
            .unwrap();

        assert_eq!(
            reversed_row.to_scalars().unwrap(),
            vec![Scalar::F64(6.0), Scalar::F64(4.0)]
        );
        reversed_row.set(&[0], Scalar::F64(60.0)).unwrap();
        assert_eq!(base.get(&[1, 2]).unwrap(), Scalar::F64(60.0));
    }

    #[test]
    fn slice_view_keeps_backing_alive_after_source_drop() {
        let mut row = {
            let base = f64_array(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
            base.slice_view(&[IndexOp::Index(1)]).unwrap()
        };

        row.set(&[0], Scalar::F64(30.0)).unwrap();
        assert_eq!(
            row.to_scalars().unwrap(),
            vec![Scalar::F64(30.0), Scalar::F64(4.0)]
        );
    }

    #[test]
    fn slice_view_inherits_read_only_state() {
        let mut base = f64_array(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
        base.set_read_only();
        let mut row = base.slice_view(&[IndexOp::Index(0)]).unwrap();

        assert!(!row.is_writable());
        assert!(matches!(
            row.set(&[0], Scalar::F64(10.0)),
            Err(ArrayError::NotWritable)
        ));
    }

    #[test]
    fn reshape_preserves_row_major_order() {
        let mut a = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let mut r = a.reshape(&[3, 2]).unwrap();
        assert_eq!(r.shape(), &[3, 2]);
        assert_eq!(r.get(&[2, 1]).unwrap(), Scalar::F64(6.0));
        assert!(r.is_c_contiguous());
        r.set(&[0, 1], Scalar::F64(20.0)).unwrap();
        assert_eq!(a.get(&[0, 1]).unwrap(), Scalar::F64(20.0));
        a.set(&[1, 2], Scalar::F64(60.0)).unwrap();
        assert_eq!(r.get(&[2, 1]).unwrap(), Scalar::F64(60.0));
    }

    #[test]
    fn reshape_size_mismatch_errors() {
        let a = f64_array(&[1.0, 2.0, 3.0, 4.0], &[4]);
        assert!(matches!(
            a.reshape(&[3]),
            Err(ArrayError::ReshapeMismatch { .. })
        ));
    }

    #[test]
    fn reshape_of_a_view_reorders_correctly() {
        // Column view [3,6] is not C-contiguous, so reshape follows logical
        // order and detaches from the source.
        let mut base = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let col = base
            .slice_view(&[IndexOp::full(), IndexOp::Index(2)])
            .unwrap();
        let mut reshaped = col.reshape(&[2, 1]).unwrap();
        assert_eq!(
            reshaped.to_scalars().unwrap(),
            vec![Scalar::F64(3.0), Scalar::F64(6.0)]
        );
        reshaped.set(&[0, 0], Scalar::F64(30.0)).unwrap();
        assert_eq!(base.get(&[0, 2]).unwrap(), Scalar::F64(3.0));
        base.set(&[1, 2], Scalar::F64(60.0)).unwrap();
        assert_eq!(reshaped.get(&[1, 0]).unwrap(), Scalar::F64(6.0));
    }

    #[test]
    fn astype_float_to_int_truncates() {
        let a = f64_array(&[1.7, -2.9, 3.2], &[3]);
        let i = a.astype(ArrayDType::Int32).unwrap();
        assert_eq!(i.dtype(), ArrayDType::Int32);
        assert_eq!(
            i.to_scalars().unwrap(),
            vec![Scalar::I64(1), Scalar::I64(-2), Scalar::I64(3)]
        );
    }

    #[test]
    fn ravel_flattens() {
        let mut a = f64_array(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
        let mut r = a.ravel().unwrap();
        assert_eq!(r.shape(), &[4]);
        r.set(&[2], Scalar::F64(30.0)).unwrap();
        assert_eq!(a.get(&[1, 0]).unwrap(), Scalar::F64(30.0));
        a.set(&[0, 1], Scalar::F64(20.0)).unwrap();
        assert_eq!(r.get(&[1]).unwrap(), Scalar::F64(20.0));
    }

    #[test]
    fn reshape_inherits_read_only_state() {
        let mut a = f64_array(&[1.0, 2.0], &[2]);
        a.set_read_only();
        let mut reshaped = a.reshape(&[1, 2]).unwrap();
        assert!(!reshaped.is_writable());
        assert!(matches!(
            reshaped.set(&[0, 0], Scalar::F64(3.0)),
            Err(ArrayError::NotWritable)
        ));
    }

    #[test]
    fn zero_d_array_has_one_element() {
        let a = DenseArrayCore::full(ArrayDType::Float64, &[], Scalar::F64(7.0)).unwrap();
        assert_eq!(a.size(), 1);
        assert_eq!(a.get(&[]).unwrap(), Scalar::F64(7.0));
    }

    #[test]
    fn reduce_axis_sum_over_rows_and_cols() {
        // [[1,2,3],[4,5,6]]
        let a = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let axis0 = a.reduce_axis(0, AxisReduce::Sum).unwrap();
        assert_eq!(axis0.shape(), &[3]);
        assert_eq!(
            axis0.to_scalars().unwrap(),
            vec![Scalar::F64(5.0), Scalar::F64(7.0), Scalar::F64(9.0)]
        );
        let axis1 = a.reduce_axis(1, AxisReduce::Sum).unwrap();
        assert_eq!(axis1.shape(), &[2]);
        assert_eq!(
            axis1.to_scalars().unwrap(),
            vec![Scalar::F64(6.0), Scalar::F64(15.0)]
        );
    }

    #[test]
    fn reduce_axis_mean_and_minmax() {
        let a = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let mean0 = a.reduce_axis(0, AxisReduce::Mean).unwrap();
        assert_eq!(
            mean0.to_scalars().unwrap(),
            vec![Scalar::F64(2.5), Scalar::F64(3.5), Scalar::F64(4.5)]
        );
        let max1 = a.reduce_axis(1, AxisReduce::Max).unwrap();
        assert_eq!(
            max1.to_scalars().unwrap(),
            vec![Scalar::F64(3.0), Scalar::F64(6.0)]
        );
    }

    #[test]
    fn reduce_axis_min_is_exact_for_int64() {
        let big = 9_007_199_254_740_993_i64;
        let a =
            DenseArrayCore::from_storage(ArrayStorage::Int64(vec![big, big - 1, 10, 20]), &[2, 2])
                .unwrap();
        let min1 = a.reduce_axis(1, AxisReduce::Min).unwrap();
        assert_eq!(min1.dtype(), ArrayDType::Int64);
        assert_eq!(
            min1.to_scalars().unwrap(),
            vec![Scalar::I64(big - 1), Scalar::I64(10)]
        );
    }

    #[test]
    fn reduce_axis_all_any_produce_bool() {
        let a = DenseArrayCore::from_storage(
            ArrayStorage::Bool(vec![true, false, true, true]),
            &[2, 2],
        )
        .unwrap();
        let all1 = a.reduce_axis(1, AxisReduce::All).unwrap();
        assert_eq!(all1.dtype(), ArrayDType::Bool);
        assert_eq!(
            all1.to_scalars().unwrap(),
            vec![Scalar::Bool(false), Scalar::Bool(true)]
        );
        let any0 = a.reduce_axis(0, AxisReduce::Any).unwrap();
        assert_eq!(
            any0.to_scalars().unwrap(),
            vec![Scalar::Bool(true), Scalar::Bool(true)]
        );
    }

    #[test]
    fn reduce_axis_out_of_bounds_errors() {
        let a = f64_array(&[1.0, 2.0], &[2]);
        assert!(matches!(
            a.reduce_axis(1, AxisReduce::Sum),
            Err(ArrayError::AxisOutOfBounds { .. })
        ));
    }

    #[test]
    fn mask_select_returns_matching_elements() {
        let a = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let mask = [false, true, false, true, false, true];
        let sel = a.mask_select(&mask, &[2, 3]).unwrap();
        assert_eq!(sel.shape(), &[3]);
        assert_eq!(
            sel.to_scalars().unwrap(),
            vec![Scalar::F64(2.0), Scalar::F64(4.0), Scalar::F64(6.0)]
        );
    }

    #[test]
    fn mask_assign_scalar_broadcasts() {
        let mut a = f64_array(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
        let mask = [true, false, false, true];
        a.mask_assign(&mask, &[2, 2], &[Scalar::F64(0.0)]).unwrap();
        assert_eq!(
            a.to_scalars().unwrap(),
            vec![
                Scalar::F64(0.0),
                Scalar::F64(2.0),
                Scalar::F64(3.0),
                Scalar::F64(0.0)
            ]
        );
    }

    #[test]
    fn mask_assign_uses_layout_offset_and_strides() {
        let storage = ArrayStorage::Float64(vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        let layout = Layout {
            shape: vec![2],
            strides: vec![2],
            offset: 1,
        };
        let mut view = DenseArrayCore::from_parts(storage, layout, true).unwrap();

        view.mask_assign(&[true, false], &[2], &[Scalar::F64(9.0)])
            .unwrap();

        assert_eq!(
            view.to_scalars().unwrap(),
            vec![Scalar::F64(9.0), Scalar::F64(3.0)]
        );
    }

    #[test]
    fn mask_assign_rejects_wrong_value_count_without_writing() {
        let mut a = f64_array(&[1.0, 2.0, 3.0, 4.0], &[4]);

        let result = a.mask_assign(
            &[true, true, true, false],
            &[4],
            &[Scalar::F64(8.0), Scalar::F64(9.0)],
        );

        assert!(matches!(
            result,
            Err(ArrayError::MaskValueCountMismatch {
                values_len: 2,
                selected: 3
            })
        ));
        assert_eq!(
            a.to_scalars().unwrap(),
            vec![
                Scalar::F64(1.0),
                Scalar::F64(2.0),
                Scalar::F64(3.0),
                Scalar::F64(4.0)
            ]
        );
    }

    #[test]
    fn mask_assign_accepts_no_values_when_nothing_is_selected() {
        let mut a = f64_array(&[1.0, 2.0], &[2]);

        a.mask_assign(&[false, false], &[2], &[]).unwrap();

        assert_eq!(
            a.to_scalars().unwrap(),
            vec![Scalar::F64(1.0), Scalar::F64(2.0)]
        );
    }

    #[test]
    fn mask_length_mismatch_errors() {
        let a = f64_array(&[1.0, 2.0], &[2]);
        assert!(matches!(
            a.mask_select(&[true], &[2]),
            Err(ArrayError::MaskLengthMismatch { .. })
        ));
    }

    #[test]
    fn mask_shape_mismatch_errors_even_when_length_matches() {
        let a = f64_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        assert!(matches!(
            a.mask_select(&[true; 6], &[6]),
            Err(ArrayError::MaskShapeMismatch { .. })
        ));
    }

    #[test]
    fn reduce_axis_minmax_over_empty_axis_errors_but_sum_is_zero() {
        let a = DenseArrayCore::zeros(ArrayDType::Float64, &[0, 3]).unwrap();
        assert!(matches!(
            a.reduce_axis(0, AxisReduce::Min),
            Err(ArrayError::ZeroSizeReduction)
        ));
        // Sum over an empty axis is 0.0, not an error.
        let s = a.reduce_axis(0, AxisReduce::Sum).unwrap();
        assert_eq!(s.shape(), &[3]);
        assert_eq!(s.to_scalars().unwrap(), vec![Scalar::F64(0.0); 3]);
    }

    #[test]
    fn mask_assign_rejects_write_after_borrow_expires() {
        #[derive(Debug)]
        struct Probe(AtomicBool);
        impl BorrowProbe for Probe {
            fn check_read(&self) -> Result<(), String> {
                if self.0.load(Ordering::Acquire) {
                    Ok(())
                } else {
                    Err("expired".into())
                }
            }
            fn check_write(&self) -> Result<(), String> {
                self.check_read()
            }
        }

        let mut data = [1.0f32, 2.0];
        let probe = Arc::new(Probe(AtomicBool::new(true)));
        // SAFETY: `data` outlives `core`; the probe gates every access.
        let storage =
            unsafe { ArrayStorage::borrowed_mut_f32(data.as_mut_ptr(), 2, probe.clone()) };
        let mut core = DenseArrayCore::from_storage(storage, &[2]).unwrap();
        // While live, a masked write succeeds.
        core.mask_assign(&[true, false], &[2], &[Scalar::F64(9.0)])
            .unwrap();
        // An expired borrow must reject the write rather than write through freed memory.
        probe.0.store(false, Ordering::Release);
        assert!(matches!(
            core.mask_assign(&[true, false], &[2], &[Scalar::F64(5.0)]),
            Err(ArrayError::BorrowExpired(_))
        ));
    }

    #[test]
    fn assign_u8_bytes_writes_borrowed_ranges_and_rejects_expired_borrows() {
        #[derive(Debug)]
        struct Probe(AtomicBool);
        impl BorrowProbe for Probe {
            fn check_read(&self) -> Result<(), String> {
                if self.0.load(Ordering::Acquire) {
                    Ok(())
                } else {
                    Err("expired".into())
                }
            }
            fn check_write(&self) -> Result<(), String> {
                self.check_read()
            }
        }

        let mut data = [0u8; 6];
        let probe = Arc::new(Probe(AtomicBool::new(true)));
        // SAFETY: `data` outlives `core`; the probe gates every access.
        let storage = unsafe { ArrayStorage::borrowed_mut_u8(data.as_mut_ptr(), 6, probe.clone()) };
        let core = DenseArrayCore::from_storage(storage, &[6]).unwrap();
        let contiguous = core
            .plan(&[IndexOp::Slice {
                start: Some(1),
                stop: Some(4),
                step: 1,
            }])
            .unwrap();
        core.assign_u8_bytes(&contiguous, &[7, 8, 9]).unwrap();
        let strided = core
            .plan(&[IndexOp::Slice {
                start: None,
                stop: None,
                step: -2,
            }])
            .unwrap();
        core.assign_u8_bytes(&strided, &[1, 2, 3]).unwrap();
        assert!(matches!(
            core.assign_u8_bytes(&contiguous, &[1, 2]),
            Err(ArrayError::ByteLengthMismatch {
                expected: 3,
                actual: 2,
                ..
            })
        ));
        probe.0.store(false, Ordering::Release);
        assert!(matches!(
            core.assign_u8_bytes(&contiguous, &[4, 5, 6]),
            Err(ArrayError::BorrowExpired(_))
        ));
        drop(core);
        assert_eq!(data, [0, 3, 8, 2, 0, 1]);
    }
}
