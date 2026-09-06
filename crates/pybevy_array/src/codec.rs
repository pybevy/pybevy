//! Interpreter-neutral tensor byte codecs.
//!
//! The wire/storage order is always little-endian and logical C order. No
//! codec retains an interpreter object or exposes a backing pointer.

use crate::{
    ArrayDType, ArrayError, ArrayResult, ArrayStorage, DenseArrayCore, checked_num_elements,
};

/// Decode one exact little-endian tensor value into owned read-only storage.
pub fn decode_read_only_le_bytes(
    dtype: ArrayDType,
    bytes: &[u8],
    shape: &[usize],
) -> ArrayResult<DenseArrayCore> {
    let elements = checked_num_elements(shape)?;
    let expected = elements
        .checked_mul(dtype.itemsize())
        .ok_or(ArrayError::Overflow("array byte size"))?;
    if bytes.len() != expected {
        return Err(ArrayError::ByteLengthMismatch {
            dtype,
            expected,
            actual: bytes.len(),
        });
    }

    let storage = match dtype {
        ArrayDType::Float16 => {
            ArrayStorage::Float16(decode_numeric(bytes, dtype, elements, u16::from_le_bytes)?)
        }
        ArrayDType::Float32 => {
            ArrayStorage::Float32(decode_numeric(bytes, dtype, elements, f32::from_le_bytes)?)
        }
        ArrayDType::Float64 => {
            ArrayStorage::Float64(decode_numeric(bytes, dtype, elements, f64::from_le_bytes)?)
        }
        ArrayDType::Int64 => {
            ArrayStorage::Int64(decode_numeric(bytes, dtype, elements, i64::from_le_bytes)?)
        }
        ArrayDType::Int32 => {
            ArrayStorage::Int32(decode_numeric(bytes, dtype, elements, i32::from_le_bytes)?)
        }
        ArrayDType::Uint32 => {
            ArrayStorage::Uint32(decode_numeric(bytes, dtype, elements, u32::from_le_bytes)?)
        }
        ArrayDType::Uint16 => {
            ArrayStorage::Uint16(decode_numeric(bytes, dtype, elements, u16::from_le_bytes)?)
        }
        ArrayDType::Uint8 => ArrayStorage::Uint8(copy_bytes(bytes, dtype, elements)?),
        ArrayDType::Bool => {
            let mut values = reserved_vec(dtype, elements)?;
            values.extend(bytes.iter().map(|value| *value != 0));
            ArrayStorage::Bool(values)
        }
    };
    let mut core = DenseArrayCore::from_storage(storage, shape)?;
    core.set_read_only();
    Ok(core)
}

/// Copy an array into `(dtype, shape, bytes)` in logical C and little-endian order.
///
/// Views are materialized in their logical order. Borrowed storage is checked
/// once while acquiring the operation-scoped read guard and is never retained.
pub fn encode_contiguous_le_bytes(
    array: &DenseArrayCore,
) -> ArrayResult<(ArrayDType, Vec<usize>, Vec<u8>)> {
    let dtype = array.dtype();
    let elements = array.size();
    let byte_len = elements
        .checked_mul(dtype.itemsize())
        .ok_or(ArrayError::Overflow("array byte size"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(byte_len)
        .map_err(|_| ArrayError::AllocationFailed { dtype, elements })?;
    let storage = array.read_storage()?;
    storage.append_le_bytes(array.layout().iter_offsets(), &mut bytes);
    debug_assert_eq!(bytes.len(), byte_len);
    Ok((dtype, array.shape().to_vec(), bytes))
}

fn reserved_vec<T>(dtype: ArrayDType, elements: usize) -> ArrayResult<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(elements)
        .map_err(|_| ArrayError::AllocationFailed { dtype, elements })?;
    Ok(values)
}

fn copy_bytes(bytes: &[u8], dtype: ArrayDType, elements: usize) -> ArrayResult<Vec<u8>> {
    let mut values = reserved_vec(dtype, elements)?;
    values.extend_from_slice(bytes);
    Ok(values)
}

fn decode_numeric<const N: usize, T>(
    bytes: &[u8],
    dtype: ArrayDType,
    elements: usize,
    decode: impl Fn([u8; N]) -> T,
) -> ArrayResult<Vec<T>> {
    let mut values = reserved_vec(dtype, elements)?;
    values.extend(
        bytes
            .chunks_exact(N)
            .map(|chunk| decode(chunk.try_into().expect("validated fixed-width chunk"))),
    );
    Ok(values)
}
