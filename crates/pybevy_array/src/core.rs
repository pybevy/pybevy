//! `DenseArrayCore`: owned storage plus a validated layout. Element-wise
//! numeric execution lives in `pybevy_bytecodevm`; this type owns construction,
//! layout, basic-slice planning, casting, and copy materialization.

use crate::{
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

#[derive(Debug, PartialEq)]
pub struct DenseArrayCore {
    storage: ArrayStorage,
    layout: Layout,
    writable: bool,
}

impl Clone for DenseArrayCore {
    fn clone(&self) -> Self {
        // `ArrayStorage::clone` downgrades a mutable borrow to a read-only one,
        // so a clone can never become a second writable alias.
        let storage = self.storage.clone();
        let writable = self.writable && !storage.is_read_only_borrow();
        DenseArrayCore {
            storage,
            layout: self.layout.clone(),
            writable,
        }
    }
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
            storage,
            layout: Layout::c_contiguous(shape)?,
            writable,
        })
    }

    /// Verify the array's data is readable for the current operation (owned data
    /// always is; borrowed data consults its liveness probe). Adapters call this
    /// once at the start of each read operation to surface a clean error.
    pub fn ensure_readable(&self) -> ArrayResult<()> {
        self.storage.ensure_readable()
    }

    /// Verify the array is writable for the current operation: not frozen, and
    /// (for a mutable borrow) still permitted by the probe on this thread.
    pub fn ensure_writable(&self) -> ArrayResult<()> {
        if !self.writable {
            return Err(ArrayError::NotWritable);
        }
        self.storage.ensure_writable()
    }

    /// Construct directly from a validated layout and storage (offsets in the
    /// layout must stay within storage). Used by adapters that build views.
    pub fn from_parts(storage: ArrayStorage, layout: Layout, writable: bool) -> Self {
        DenseArrayCore {
            storage,
            layout,
            writable,
        }
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
        let raw_count = ((stop - start) / step).ceil().max(0.0);
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
        self.storage.dtype()
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

    pub fn storage(&self) -> &ArrayStorage {
        &self.storage
    }

    /// Mutable storage access for offset-addressed writes (e.g. slice
    /// assignment). Errors if the array is read-only. Callers must keep offsets
    /// within storage; plan them through [`Self::plan`].
    pub fn storage_mut(&mut self) -> ArrayResult<&mut ArrayStorage> {
        self.ensure_writable()?;
        Ok(&mut self.storage)
    }

    /// Cast and write every logical element from an equally-shaped owned
    /// result into this array. The source is materialized before destination
    /// mutation, which keeps self-aliasing and overlapping borrowed views safe.
    pub fn assign_from(&mut self, source: &DenseArrayCore) -> ArrayResult<()> {
        self.ensure_writable()?;
        source.ensure_readable()?;
        if self.shape() != source.shape() {
            return Err(ArrayError::BroadcastMismatch {
                left: self.shape().to_vec(),
                right: source.shape().to_vec(),
            });
        }
        let values = source.to_scalars()?;
        let layout = self.layout.clone();
        for (offset, value) in layout.iter_offsets().zip(values) {
            self.storage.set(offset, value);
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
        self.storage.dtype().itemsize()
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn is_c_contiguous(&self) -> bool {
        self.layout.is_c_contiguous()
    }

    pub fn set_read_only(&mut self) {
        self.writable = false;
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
        self.storage.ensure_readable()?;
        Ok(self.storage.get(self.flat_offset(indices)?))
    }

    /// Write one element by full multi-index. Errors if read-only or if a
    /// mutable borrow's probe rejects the write (e.g. after the system ends).
    pub fn set(&mut self, indices: &[usize], value: Scalar) -> ArrayResult<()> {
        self.ensure_writable()?;
        let offset = self.flat_offset(indices)?;
        self.storage.set(offset, value);
        Ok(())
    }

    /// Plan a basic-index selection without moving data. Adapters apply the
    /// plan either by copying ([`Self::slice_copy`]) or by borrowing.
    pub fn plan(&self, ops: &[IndexOp]) -> ArrayResult<Layout> {
        self.layout.index(ops)
    }

    /// All elements in row-major order as neutral scalars.
    pub fn to_scalars(&self) -> ArrayResult<Vec<Scalar>> {
        self.storage.ensure_readable()?;
        Ok(self
            .layout
            .iter_offsets()
            .map(|off| self.storage.get(off))
            .collect())
    }

    fn materialize(&self, layout: &Layout) -> ArrayResult<DenseArrayCore> {
        self.storage.ensure_readable()?;
        let dtype = self.storage.dtype();
        let mut out = ArrayStorage::zeros(dtype, layout.num_elements())?;
        for (i, off) in layout.iter_offsets().enumerate() {
            out.set(i, self.storage.get(off));
        }
        Ok(DenseArrayCore {
            storage: out,
            layout: Layout::c_contiguous(&layout.shape)
                .expect("sub-selection element count already bounded"),
            writable: true,
        })
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

    /// Cast to `dtype`, producing a new contiguous array (NumPy `astype`).
    pub fn astype(&self, dtype: ArrayDType) -> ArrayResult<DenseArrayCore> {
        self.storage.ensure_readable()?;
        let mut out = ArrayStorage::zeros(dtype, self.size())?;
        for (i, off) in self.layout.iter_offsets().enumerate() {
            out.set(i, self.storage.get(off));
        }
        Ok(DenseArrayCore {
            storage: out,
            layout: Layout::c_contiguous(&self.layout.shape)
                .expect("shape element count already bounded"),
            writable: true,
        })
    }

    /// Reshape to `new_shape` (same element count), returning a contiguous copy.
    pub fn reshape(&self, new_shape: &[usize]) -> ArrayResult<DenseArrayCore> {
        let want = checked_num_elements(new_shape)?;
        if want != self.size() {
            return Err(ArrayError::ReshapeMismatch {
                from: self.layout.shape.clone(),
                to: new_shape.to_vec(),
            });
        }
        let mut copied = self.copy()?;
        copied.layout = Layout::c_contiguous(new_shape)?;
        Ok(copied)
    }

    /// Flatten to one dimension (contiguous copy).
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
        // Min/Max have no identity for an empty axis; NumPy raises rather than
        // producing a value.
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

    /// Select the elements where `mask` is true, returning a 1-D array. `mask`
    /// must have one entry per element in row-major order.
    pub fn mask_select(&self, mask: &[bool]) -> ArrayResult<DenseArrayCore> {
        if mask.len() != self.size() {
            return Err(ArrayError::MaskLengthMismatch {
                mask_len: mask.len(),
                size: self.size(),
            });
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
    pub fn mask_assign(&mut self, mask: &[bool], values: &[Scalar]) -> ArrayResult<()> {
        if mask.len() != self.size() {
            return Err(ArrayError::MaskLengthMismatch {
                mask_len: mask.len(),
                size: self.size(),
            });
        }
        // Gate on the probe (not just the frozen flag), so a mutable borrow
        // whose system has ended or whose context has closed rejects the write
        // instead of dereferencing freed memory.
        self.ensure_writable()?;
        // Contiguous base array (offset 0): row-major position == storage index.
        let storage = &mut self.storage;
        let mut vi = 0;
        for (pos, &m) in mask.iter().enumerate() {
            if m {
                let value = if values.len() == 1 {
                    values[0]
                } else {
                    values[vi]
                };
                storage.set(pos, value);
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
