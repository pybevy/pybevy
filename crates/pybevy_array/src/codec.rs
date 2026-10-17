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
        ArrayDType::Int16 => {
            ArrayStorage::Int16(decode_numeric(bytes, dtype, elements, i16::from_le_bytes)?)
        }
        ArrayDType::Int8 => ArrayStorage::Int8(
            copy_bytes(bytes, dtype, elements)?
                .into_iter()
                .map(|value| value as i8)
                .collect(),
        ),
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
    if array.is_c_contiguous() {
        let start = array.layout().offset;
        storage.append_le_bytes(start..start + elements, &mut bytes);
    } else {
        storage.append_le_bytes(array.layout().iter_offsets(), &mut bytes);
    }
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{BorrowProbe, IndexOp, Scalar};

    #[derive(Debug)]
    struct Readable;

    impl BorrowProbe for Readable {
        fn check_read(&self) -> Result<(), String> {
            Ok(())
        }
    }

    fn roundtrip(dtype: ArrayDType, bytes: &[u8], shape: &[usize]) {
        let decoded = decode_read_only_le_bytes(dtype, bytes, shape).unwrap();
        assert!(!decoded.is_writable());
        assert_eq!(
            encode_contiguous_le_bytes(&decoded).unwrap(),
            (dtype, shape.to_vec(), bytes.to_vec())
        );
    }

    #[test]
    fn every_dtype_roundtrips_exact_little_endian_bytes() {
        roundtrip(
            ArrayDType::Float16,
            &[
                0x00, 0x00, // +0
                0x00, 0x80, // -0
                0x01, 0x00, // smallest subnormal
                0x01, 0x7e, // NaN payload
                0x00, 0x7c, // +inf
                0x00, 0xfc, // -inf
            ],
            &[6],
        );
        roundtrip(
            ArrayDType::Float32,
            &[0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0xc0, 0x7f],
            &[2],
        );
        roundtrip(
            ArrayDType::Float64,
            &[
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                0xf8, 0x7f,
            ],
            &[2],
        );
        roundtrip(
            ArrayDType::Int64,
            &(-0x0102_0304_0506_0708_i64).to_le_bytes(),
            &[1],
        );
        roundtrip(ArrayDType::Int32, &(-0x0102_0304_i32).to_le_bytes(), &[1]);
        roundtrip(ArrayDType::Int16, &(-0x0102_i16).to_le_bytes(), &[1]);
        roundtrip(ArrayDType::Int8, &[0x80, 0xff, 0x7f], &[3]);
        roundtrip(ArrayDType::Uint32, &0x0102_0304_u32.to_le_bytes(), &[1]);
        roundtrip(ArrayDType::Uint16, &0x0102_u16.to_le_bytes(), &[1]);
        roundtrip(ArrayDType::Uint8, &[0, 127, 255], &[3]);
        roundtrip(ArrayDType::Bool, &[0, 1, 1], &[3]);
    }

    #[test]
    fn boolean_decode_accepts_nonzero_and_encodes_canonically() {
        let decoded = decode_read_only_le_bytes(ArrayDType::Bool, &[0, 2, 255], &[3]).unwrap();
        assert_eq!(
            decoded.to_scalars().unwrap(),
            vec![Scalar::Bool(false), Scalar::Bool(true), Scalar::Bool(true)]
        );
        assert_eq!(encode_contiguous_le_bytes(&decoded).unwrap().2, [0, 1, 1]);
    }

    #[test]
    fn scalar_and_empty_shapes_keep_their_distinct_element_counts() {
        roundtrip(ArrayDType::Uint16, &[0x34, 0x12], &[]);
        roundtrip(ArrayDType::Uint16, &[], &[0, 4]);
    }

    #[test]
    fn wrong_byte_length_and_overflow_are_rejected_before_decode() {
        assert_eq!(
            decode_read_only_le_bytes(ArrayDType::Float32, &[0; 7], &[2]).unwrap_err(),
            ArrayError::ByteLengthMismatch {
                dtype: ArrayDType::Float32,
                expected: 8,
                actual: 7,
            }
        );
        assert_eq!(
            decode_read_only_le_bytes(ArrayDType::Float64, &[], &[usize::MAX, 2]).unwrap_err(),
            ArrayError::Overflow("shape element count")
        );
    }

    #[test]
    fn noncontiguous_views_encode_in_logical_c_order() {
        let core =
            DenseArrayCore::from_storage(ArrayStorage::Int32(vec![1, 2, 3, 4, 5, 6]), &[2, 3])
                .unwrap();
        let reversed = core
            .slice_view(&[
                IndexOp::full(),
                IndexOp::Slice {
                    start: None,
                    stop: None,
                    step: -1,
                },
            ])
            .unwrap();
        let (_, shape, bytes) = encode_contiguous_le_bytes(&reversed).unwrap();
        assert_eq!(shape, [2, 3]);
        let values = bytes
            .chunks_exact(4)
            .map(|chunk| i32::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(values, [3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn borrowed_data_is_copied_while_the_read_probe_is_live() {
        let data = [1.25_f32, -2.5];
        // SAFETY: `data` remains alive and immutable through the synchronous encode.
        let storage =
            unsafe { ArrayStorage::borrowed_f32(data.as_ptr(), data.len(), Arc::new(Readable)) };
        let core = DenseArrayCore::from_storage(storage, &[2]).unwrap();
        let bytes = encode_contiguous_le_bytes(&core).unwrap().2;
        assert_eq!(&bytes[..4], &1.25_f32.to_le_bytes());
        assert_eq!(&bytes[4..], &(-2.5_f32).to_le_bytes());
    }

    #[test]
    fn conflicting_storage_access_is_reported_without_partial_output() {
        let core = DenseArrayCore::from_storage(ArrayStorage::Uint8(vec![1, 2]), &[2]).unwrap();
        let _write = core.write_storage().unwrap();
        assert_eq!(
            encode_contiguous_le_bytes(&core).unwrap_err(),
            ArrayError::AccessConflict
        );
    }
}
