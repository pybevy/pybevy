//! Validated in-place expression execution over flat, strided buffers.
//!
//! Adapters describe buffer lanes as synthetic fields. Every compiled program
//! must pass [`validate_buffer_program`] before [`execute_buffer_assignment`]
//! may perform pointer arithmetic.

use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

use bevy_ecs::component::ComponentId;

use crate::{
    bytecode::{CompiledBytecode, FieldType, Op, VM},
    tiled::{TiledScratch, execute_assignment_tiled, supported_program},
    view_runtime::{ProgramIntent, validate_instruction_indices, validate_stack_effects},
};

/// Opaque grouping key for one buffer lens. It is never resolved against a World.
pub type BufferKey = ComponentId;

static NEXT_BUFFER_KEY: AtomicUsize = AtomicUsize::new(i64::MAX as usize);

/// Allocate a process-unique grouping key for a buffer lens.
pub fn alloc_buffer_key() -> BufferKey {
    BufferKey::new(NEXT_BUFFER_KEY.fetch_sub(1, Ordering::Relaxed))
}

/// Why a compiled program cannot execute over a particular buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferLensError {
    ForeignField {
        got: usize,
        expected: usize,
    },
    OffsetOutOfBounds {
        offset: usize,
        size: usize,
        stride: usize,
    },
    MisalignedOffset {
        offset: usize,
        size: usize,
    },
    UnsupportedFieldType {
        field_type: FieldType,
    },
    MalformedProgram(String),
    NotAnAssignment,
}

impl fmt::Display for BufferLensError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferLensError::ForeignField { .. } => formatter.write_str(
                "expression combines fields from different lenses; create one lens and reuse it on both sides of the assignment",
            ),
            BufferLensError::OffsetOutOfBounds {
                offset,
                size,
                stride,
            } => write!(
                formatter,
                "lane at offset {offset} (size {size}) spills past the element stride {stride}"
            ),
            BufferLensError::MisalignedOffset { offset, size } => write!(
                formatter,
                "lane offset {offset} is not a multiple of its element size {size}"
            ),
            BufferLensError::UnsupportedFieldType { field_type } => write!(
                formatter,
                "field type {field_type:?} is not supported for this buffer"
            ),
            BufferLensError::MalformedProgram(reason) => {
                write!(formatter, "malformed buffer-lens program: {reason}")
            }
            BufferLensError::NotAnAssignment => {
                write!(
                    formatter,
                    "program is not a single in-place field assignment"
                )
            }
        }
    }
}

/// Proof that one bytecode program was validated together with the exact
/// strided buffer layout it will execute against.
pub struct ValidatedBufferProgram<'a> {
    bytecode: &'a CompiledBytecode,
    stride: usize,
}

impl Error for BufferLensError {}

/// Validate one compiled in-place assignment against its buffer layout.
pub fn validate_buffer_program<'a>(
    bytecode: &'a CompiledBytecode,
    key: BufferKey,
    stride: usize,
    allowed: &[FieldType],
) -> Result<ValidatedBufferProgram<'a>, BufferLensError> {
    validate_instruction_indices(bytecode)
        .map_err(|error| BufferLensError::MalformedProgram(error.to_string()))?;
    let stores = bytecode
        .bytecode
        .iter()
        .filter(|op| matches!(op, Op::StoreField(_)))
        .count();
    if stores != 1 {
        return Err(BufferLensError::NotAnAssignment);
    }
    let destination = bytecode
        .bytecode
        .iter()
        .find_map(|op| match *op {
            Op::StoreField(index) => Some(bytecode.field_map[usize::from(index)]),
            _ => None,
        })
        .expect("exactly one validated store exists");
    validate_stack_effects(bytecode, ProgramIntent::Assignment { destination })
        .map_err(|error| BufferLensError::MalformedProgram(error.to_string()))?;

    for field in &bytecode.field_map {
        if field.component_id != key {
            return Err(BufferLensError::ForeignField {
                got: field.component_id.index(),
                expected: key.index(),
            });
        }
        if !allowed.contains(&field.field_type) {
            return Err(BufferLensError::UnsupportedFieldType {
                field_type: field.field_type,
            });
        }

        let size = field.field_type.size_bytes();
        if size == 0 || !field.offset.is_multiple_of(size) {
            return Err(BufferLensError::MisalignedOffset {
                offset: field.offset,
                size,
            });
        }
        if field
            .offset
            .checked_add(size)
            .is_none_or(|end| end > stride)
        {
            return Err(BufferLensError::OffsetOutOfBounds {
                offset: field.offset,
                size,
                stride,
            });
        }
    }
    Ok(ValidatedBufferProgram { bytecode, stride })
}

/// Execute a validated assignment over `count` buffer elements.
///
/// # Safety
/// For every field in `bytecode.field_map` and every element in `0..count`,
/// `base.add(element * validated.stride + field.offset)` must identify a valid,
/// exclusively writable value of the declared field type for the duration of
/// this call. `validated` binds bytecode and stride to one successful
/// [`validate_buffer_program`] call.
pub unsafe fn execute_buffer_assignment(
    validated: &ValidatedBufferProgram<'_>,
    base: *mut u8,
    count: usize,
) {
    let bytecode = validated.bytecode;
    let stride = validated.stride;
    if count == 0 || bytecode.field_map.is_empty() {
        return;
    }

    let field_bases: Vec<*mut u8> = bytecode
        .field_map
        .iter()
        .map(|field| {
            // SAFETY: validation proved `field.offset + size <= stride`; the
            // function contract covers the complete strided run.
            unsafe { base.add(field.offset) }
        })
        .collect();
    let strides = vec![stride; field_bases.len()];

    if supported_program(bytecode) {
        let mut scratch = TiledScratch::new();
        // SAFETY: forwards the function's pointer and exclusivity contract.
        unsafe { execute_assignment_tiled(bytecode, &field_bases, &strides, count, &mut scratch) }
            .expect("supported_program checked");
    } else {
        let mut vm = VM::new();
        // SAFETY: forwards the function's pointer and exclusivity contract.
        unsafe { vm.execute_batch_multi(bytecode, &field_bases, &strides, count) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bytecode::FieldId, expr::RustExpr, view_engine::compile_assignment};

    fn field(key: BufferKey, offset: usize) -> RustExpr {
        RustExpr::Field {
            component_id: key,
            offset,
            field_type: FieldType::F32,
        }
    }

    fn field_u8(key: BufferKey, offset: usize) -> RustExpr {
        RustExpr::Field {
            component_id: key,
            offset,
            field_type: FieldType::U8,
        }
    }

    #[test]
    fn keys_are_unique_and_outside_realistic_component_ranges() {
        let first = alloc_buffer_key();
        let second = alloc_buffer_key();
        assert_ne!(first, second);
        assert!(first.index() > u32::MAX as usize);
        assert!(second.index() > u32::MAX as usize);
    }

    #[test]
    fn validates_well_formed_assignment() {
        let key = alloc_buffer_key();
        let expression = RustExpr::Add(
            Box::new(RustExpr::Mul(
                Box::new(field(key, 0)),
                Box::new(RustExpr::Const(2.0)),
            )),
            Box::new(field(key, 8)),
        );
        let bytecode = compile_assignment(key, 4, FieldType::F32, &expression);
        assert!(validate_buffer_program(&bytecode, key, 12, &[FieldType::F32]).is_ok());
    }

    #[test]
    fn rejects_foreign_field() {
        let key = alloc_buffer_key();
        let other = alloc_buffer_key();
        let bytecode = compile_assignment(key, 0, FieldType::F32, &field(other, 0));
        assert!(matches!(
            validate_buffer_program(&bytecode, key, 12, &[FieldType::F32]),
            Err(BufferLensError::ForeignField { .. })
        ));
    }

    #[test]
    fn rejects_out_of_bounds_and_overflowing_offsets() {
        let key = alloc_buffer_key();
        let out_of_bounds = compile_assignment(key, 12, FieldType::F32, &field(key, 0));
        assert!(matches!(
            validate_buffer_program(&out_of_bounds, key, 12, &[FieldType::F32]),
            Err(BufferLensError::OffsetOutOfBounds { .. })
        ));

        let overflow =
            compile_assignment(key, usize::MAX, FieldType::U8, &field_u8(key, usize::MAX));
        assert!(matches!(
            validate_buffer_program(&overflow, key, 4, &[FieldType::U8]),
            Err(BufferLensError::OffsetOutOfBounds {
                offset: usize::MAX,
                size: 1,
                stride: 4,
            })
        ));
    }

    #[test]
    fn rejects_misaligned_or_disallowed_fields() {
        let key = alloc_buffer_key();
        let misaligned = compile_assignment(key, 0, FieldType::F32, &field(key, 2));
        assert!(matches!(
            validate_buffer_program(&misaligned, key, 12, &[FieldType::F32]),
            Err(BufferLensError::MisalignedOffset { .. })
        ));

        let f64_field = RustExpr::Field {
            component_id: key,
            offset: 0,
            field_type: FieldType::F64,
        };
        let disallowed = compile_assignment(key, 0, FieldType::F64, &f64_field);
        assert!(matches!(
            validate_buffer_program(&disallowed, key, 12, &[FieldType::F32]),
            Err(BufferLensError::UnsupportedFieldType { .. })
        ));
    }

    #[test]
    fn rejects_malformed_instruction_indices_and_stack_shape() {
        let key = alloc_buffer_key();
        let invalid_index = CompiledBytecode {
            bytecode: vec![Op::PushField(9), Op::StoreField(0)],
            constants: vec![],
            field_map: vec![FieldId {
                component_id: key,
                offset: 0,
                field_type: FieldType::F32,
            }],
        };
        assert!(matches!(
            validate_buffer_program(&invalid_index, key, 4, &[FieldType::F32]),
            Err(BufferLensError::MalformedProgram(_))
        ));

        let stack_underflow = CompiledBytecode {
            bytecode: vec![Op::Add, Op::StoreField(0)],
            constants: vec![],
            field_map: vec![FieldId {
                component_id: key,
                offset: 0,
                field_type: FieldType::F32,
            }],
        };
        assert!(matches!(
            validate_buffer_program(&stack_underflow, key, 4, &[FieldType::F32]),
            Err(BufferLensError::MalformedProgram(_))
        ));
    }

    #[test]
    fn executes_strided_f32_assignment_in_place() {
        let key = alloc_buffer_key();
        let expression = RustExpr::Add(
            Box::new(RustExpr::Mul(
                Box::new(field(key, 0)),
                Box::new(RustExpr::Const(2.0)),
            )),
            Box::new(field(key, 8)),
        );
        let bytecode = compile_assignment(key, 4, FieldType::F32, &expression);
        let validated = validate_buffer_program(&bytecode, key, 12, &[FieldType::F32]).unwrap();

        let mut values = [[1.0f32, 0.0, 3.0], [5.0, 0.0, 7.0], [2.0, 9.0, 1.0]];
        // SAFETY: `values` is exclusively borrowed and has a 12-byte row stride.
        unsafe { execute_buffer_assignment(&validated, values.as_mut_ptr().cast(), values.len()) };

        assert_eq!(values, [[1.0, 5.0, 3.0], [5.0, 17.0, 7.0], [2.0, 5.0, 1.0]]);
    }

    #[test]
    fn executes_u8_with_wrapping_arithmetic() {
        let key = alloc_buffer_key();
        let expression = RustExpr::Sub(
            Box::new(RustExpr::Mul(
                Box::new(field_u8(key, 0)),
                Box::new(RustExpr::Const(4.0)),
            )),
            Box::new(RustExpr::Const(10.0)),
        );
        let bytecode = compile_assignment(key, 0, FieldType::U8, &expression);
        let validated = validate_buffer_program(&bytecode, key, 4, &[FieldType::U8]).unwrap();

        let mut values = [[10u8, 1, 2, 255], [100, 0, 0, 255], [2, 0, 0, 255]];
        // SAFETY: `values` is exclusively borrowed and has a four-byte row stride.
        unsafe { execute_buffer_assignment(&validated, values.as_mut_ptr().cast(), values.len()) };

        assert_eq!(
            values,
            [[30, 1, 2, 255], [134, 0, 0, 255], [254, 0, 0, 255]]
        );
    }

    #[test]
    fn supports_destination_self_alias() {
        let key = alloc_buffer_key();
        let expression = RustExpr::Add(
            Box::new(RustExpr::Mul(
                Box::new(field(key, 0)),
                Box::new(RustExpr::Const(3.0)),
            )),
            Box::new(field(key, 4)),
        );
        let bytecode = compile_assignment(key, 0, FieldType::F32, &expression);
        let validated = validate_buffer_program(&bytecode, key, 12, &[FieldType::F32]).unwrap();

        let mut values = [[1.0f32, 10.0, 0.0], [2.0, 20.0, 0.0]];
        // SAFETY: `values` is exclusively borrowed and has a 12-byte row stride.
        unsafe { execute_buffer_assignment(&validated, values.as_mut_ptr().cast(), values.len()) };

        assert_eq!(values, [[13.0, 10.0, 0.0], [26.0, 20.0, 0.0]]);
    }
}
