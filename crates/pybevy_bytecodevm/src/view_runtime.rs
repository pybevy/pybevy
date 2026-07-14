//! Interpreter-neutral safety boundary for View bytecode execution.
//!
//! A [`ValidatedViewProgram`] is an unforgeable capability: it can only be
//! created after the program's declared components, field identities, byte
//! spans, and write effects have been checked against one resolved View.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::Arc,
};

use bevy_ecs::component::ComponentId;

use crate::{
    bytecode::{CompiledBytecode, FieldId, FieldType, Op},
    view_engine::{self, ViewEngineError, ViewFilter},
};

/// The operation for which a bytecode program is being validated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramIntent {
    /// Evaluate a program without writing component memory.
    ReadOnly,
    /// Assign to exactly one declared mutable field.
    Assignment {
        /// The only field the program may store to.
        destination: FieldId,
    },
}

/// Interpreter-neutral metadata resolved for one View parameter.
///
/// Construction checks that component layouts, allowed fields, and mutable
/// access all describe the same declared component set. Backend-specific type
/// objects and field names deliberately do not cross this boundary.
#[derive(Debug)]
pub struct ResolvedViewSpec {
    filter: ViewFilter,
    mutable_components: HashSet<ComponentId>,
    allowed_fields: HashMap<ComponentId, HashSet<(usize, FieldType)>>,
    component_strides: HashMap<ComponentId, usize>,
}

impl ResolvedViewSpec {
    /// Build a resolved View specification from registration-time metadata.
    pub fn new(
        filter: ViewFilter,
        mutable_components: HashSet<ComponentId>,
        mut allowed_fields: HashMap<ComponentId, HashSet<(usize, FieldType)>>,
        component_strides: HashMap<ComponentId, usize>,
    ) -> Result<Self, ViewRuntimeError> {
        for &component_id in &mutable_components {
            if !filter.component_ids.contains(&component_id) {
                return Err(ViewRuntimeError::SpecComponentNotDeclared {
                    component_id,
                    metadata: "mutable component",
                });
            }
        }

        for &component_id in allowed_fields.keys() {
            if !filter.component_ids.contains(&component_id) {
                return Err(ViewRuntimeError::SpecComponentNotDeclared {
                    component_id,
                    metadata: "allowed field",
                });
            }
        }

        for &component_id in component_strides.keys() {
            if !filter.component_ids.contains(&component_id) {
                return Err(ViewRuntimeError::SpecComponentNotDeclared {
                    component_id,
                    metadata: "component stride",
                });
            }
        }

        for &component_id in &filter.component_ids {
            let stride = component_strides
                .get(&component_id)
                .copied()
                .ok_or(ViewRuntimeError::MissingComponentStride(component_id))?;
            let fields = allowed_fields.entry(component_id).or_default();
            for &(offset, field_type) in fields.iter() {
                if matches!(
                    field_type,
                    FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4
                ) {
                    return Err(ViewRuntimeError::CompositeFieldNotExpanded {
                        component_id,
                        offset,
                        field_type,
                    });
                }
                validate_field_span(component_id, offset, field_type, stride)?;
            }
        }

        Ok(Self {
            filter,
            mutable_components,
            allowed_fields,
            component_strides,
        })
    }

    /// The resolved filter used for exact batch gathering.
    pub fn filter(&self) -> &ViewFilter {
        &self.filter
    }

    /// Components for which this View declared mutable access.
    pub fn mutable_components(&self) -> &HashSet<ComponentId> {
        &self.mutable_components
    }

    /// Legitimate VM-visible field starts and primitive types per component.
    pub fn allowed_fields(&self) -> &HashMap<ComponentId, HashSet<(usize, FieldType)>> {
        &self.allowed_fields
    }

    /// Registered ECS table stride for each declared data component.
    pub fn component_strides(&self) -> &HashMap<ComponentId, usize> {
        &self.component_strides
    }
}

/// Stable, per-parameter View metadata shared across system runs.
///
/// The collision-safe bytecode cache will join this type in the execution
/// migration slice. Keeping the specification here now ensures validated
/// programs remain bound to the same metadata across frames.
#[derive(Debug)]
pub struct CachedViewCore {
    spec: Arc<ResolvedViewSpec>,
}

impl CachedViewCore {
    /// Create a stable cache owner around one resolved View specification.
    pub fn new(spec: ResolvedViewSpec) -> Self {
        Self {
            spec: Arc::new(spec),
        }
    }

    /// Return the exact specification shared by this cache owner.
    pub fn spec(&self) -> &Arc<ResolvedViewSpec> {
        &self.spec
    }
}

/// First run-scoped portion of the shared View runtime.
///
/// This initial form is the sole constructor of validated programs. Run-scoped
/// world access, validity, operation fencing, and batch leases are added in the
/// subsequent migration slice.
#[derive(Debug)]
pub struct ViewRuntimeCore {
    cached: Arc<CachedViewCore>,
}

impl ViewRuntimeCore {
    /// Create a run-scoped core from stable per-parameter metadata.
    pub fn new(cached: Arc<CachedViewCore>) -> Self {
        Self { cached }
    }

    /// Return the stable cache owner shared across runs.
    pub fn cached(&self) -> &Arc<CachedViewCore> {
        &self.cached
    }

    /// Return the exact specification owned by this runtime's cache.
    pub fn spec(&self) -> &Arc<ResolvedViewSpec> {
        self.cached.spec()
    }

    /// Validate bytecode for one explicit read or assignment operation.
    ///
    /// Validation is deliberately centralized and ordered: instruction-map
    /// integrity, declared components, real field identities, in-layout byte
    /// spans, then operation-specific write effects.
    pub fn validate_program(
        &self,
        bytecode: Arc<CompiledBytecode>,
        intent: ProgramIntent,
    ) -> Result<ValidatedViewProgram, ViewRuntimeError> {
        validate_instruction_indices(&bytecode)?;
        let spec = self.cached.spec();
        view_engine::validate_bytecode_components(&bytecode, &spec.filter.component_ids)?;
        view_engine::validate_bytecode_field_types(&bytecode, &spec.allowed_fields)?;
        validate_bytecode_spans(&bytecode, &spec.component_strides)?;
        validate_write_effects(&bytecode, intent, &spec.mutable_components)?;

        Ok(ValidatedViewProgram {
            bytecode,
            intent,
            spec: Arc::clone(spec),
        })
    }

    /// Reject a capability validated for a different View runtime.
    pub fn check_program(&self, program: &ValidatedViewProgram) -> Result<(), ViewRuntimeError> {
        if Arc::ptr_eq(self.cached.spec(), &program.spec) {
            Ok(())
        } else {
            Err(ViewRuntimeError::ProgramFromDifferentRuntime)
        }
    }
}

/// A bytecode program validated for one exact View specification and intent.
///
/// Its fields are private so safe code cannot forge the capability or replace
/// the checked bytecode, intent, or owning specification.
#[derive(Debug)]
pub struct ValidatedViewProgram {
    bytecode: Arc<CompiledBytecode>,
    intent: ProgramIntent,
    spec: Arc<ResolvedViewSpec>,
}

impl ValidatedViewProgram {
    /// The immutable bytecode that passed validation.
    pub fn bytecode(&self) -> &CompiledBytecode {
        &self.bytecode
    }

    /// The operation for which this program was validated.
    pub fn intent(&self) -> ProgramIntent {
        self.intent
    }
}

/// Failure while resolving a View or validating a program capability.
#[derive(Debug)]
pub enum ViewRuntimeError {
    /// One metadata map names a component outside the View's data declaration.
    SpecComponentNotDeclared {
        /// The inconsistent component id.
        component_id: ComponentId,
        /// The metadata category that named it.
        metadata: &'static str,
    },
    /// A declared data component has no resolved table stride.
    MissingComponentStride(ComponentId),
    /// VM-visible vector fields must be expanded into primitive `F32` lanes.
    CompositeFieldNotExpanded {
        /// Component containing the composite field.
        component_id: ComponentId,
        /// Byte offset of the unexpanded field.
        offset: usize,
        /// Composite vector field type.
        field_type: FieldType,
    },
    /// An instruction references a field-map entry that does not exist.
    InvalidFieldIndex {
        /// Instruction that contains the invalid index.
        instruction: &'static str,
        /// Referenced field-map index.
        index: u16,
        /// Number of entries in the field map.
        field_count: usize,
    },
    /// A `PushConst` instruction references a missing constant.
    InvalidConstantIndex {
        /// Referenced constant-pool index.
        index: u16,
        /// Number of entries in the constant pool.
        constant_count: usize,
    },
    /// A read-only operation contains one or more stores.
    StoreInReadOnlyProgram,
    /// An assignment does not contain exactly one store.
    AssignmentStoreCount {
        /// Number of stores found in the bytecode.
        actual: usize,
    },
    /// The assignment store does not name its declared destination.
    AssignmentDestinationMismatch {
        /// Destination declared by the operation.
        expected: FieldId,
        /// Destination encoded by the bytecode store.
        actual: FieldId,
    },
    /// The assignment destination's component is read-only in this View.
    AssignmentDestinationReadOnly(ComponentId),
    /// A valid capability was presented to a different View runtime.
    ProgramFromDifferentRuntime,
    /// One of the existing component/field/layout engine checks failed.
    Engine(ViewEngineError),
}

impl fmt::Display for ViewRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpecComponentNotDeclared {
                component_id,
                metadata,
            } => write!(
                f,
                "Resolved View {metadata} metadata references undeclared component {component_id:?}"
            ),
            Self::MissingComponentStride(component_id) => write!(
                f,
                "Resolved View is missing the table stride for component {component_id:?}"
            ),
            Self::CompositeFieldNotExpanded {
                component_id,
                offset,
                field_type,
            } => write!(
                f,
                "Resolved View field at offset {offset} for component {component_id:?} uses \
                 unexpanded composite type {field_type:?}"
            ),
            Self::InvalidFieldIndex {
                instruction,
                index,
                field_count,
            } => write!(
                f,
                "View bytecode {instruction} references field index {index}, but the field map has \
                 {field_count} entries"
            ),
            Self::InvalidConstantIndex {
                index,
                constant_count,
            } => write!(
                f,
                "View bytecode PushConst references constant index {index}, but the constant pool \
                 has {constant_count} entries"
            ),
            Self::StoreInReadOnlyProgram => {
                write!(
                    f,
                    "Read-only View program contains a StoreField instruction"
                )
            }
            Self::AssignmentStoreCount { actual } => write!(
                f,
                "View assignment must contain exactly one StoreField instruction, found {actual}"
            ),
            Self::AssignmentDestinationMismatch { expected, actual } => write!(
                f,
                "View assignment stores to {actual:?}, not its declared destination {expected:?}"
            ),
            Self::AssignmentDestinationReadOnly(component_id) => write!(
                f,
                "View assignment destination component {component_id:?} was not declared mutable"
            ),
            Self::ProgramFromDifferentRuntime => {
                write!(f, "Validated View program belongs to a different runtime")
            }
            Self::Engine(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ViewRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ViewEngineError> for ViewRuntimeError {
    fn from(value: ViewEngineError) -> Self {
        Self::Engine(value)
    }
}

fn validate_instruction_indices(bytecode: &CompiledBytecode) -> Result<(), ViewRuntimeError> {
    for op in &bytecode.bytecode {
        match *op {
            Op::PushField(index) | Op::StoreField(index)
                if usize::from(index) >= bytecode.field_map.len() =>
            {
                return Err(ViewRuntimeError::InvalidFieldIndex {
                    instruction: match op {
                        Op::PushField(_) => "PushField",
                        Op::StoreField(_) => "StoreField",
                        _ => unreachable!(),
                    },
                    index,
                    field_count: bytecode.field_map.len(),
                });
            }
            Op::PushConst(index) if usize::from(index) >= bytecode.constants.len() => {
                return Err(ViewRuntimeError::InvalidConstantIndex {
                    index,
                    constant_count: bytecode.constants.len(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_bytecode_spans(
    bytecode: &CompiledBytecode,
    component_strides: &HashMap<ComponentId, usize>,
) -> Result<(), ViewRuntimeError> {
    for field in &bytecode.field_map {
        let stride = component_strides
            .get(&field.component_id)
            .copied()
            .ok_or(ViewRuntimeError::MissingComponentStride(field.component_id))?;
        validate_field_span(field.component_id, field.offset, field.field_type, stride)?;
    }
    Ok(())
}

fn validate_field_span(
    component_id: ComponentId,
    offset: usize,
    field_type: FieldType,
    layout_size: usize,
) -> Result<(), ViewRuntimeError> {
    let type_size = field_type.size_bytes();
    let span = offset.checked_add(type_size).ok_or({
        ViewRuntimeError::Engine(ViewEngineError::FieldOffsetOutOfBounds {
            component_id,
            offset,
            type_size,
            layout_size,
        })
    })?;
    if span > layout_size {
        return Err(ViewEngineError::FieldOffsetOutOfBounds {
            component_id,
            offset,
            type_size,
            layout_size,
        }
        .into());
    }
    Ok(())
}

fn validate_write_effects(
    bytecode: &CompiledBytecode,
    intent: ProgramIntent,
    mutable_components: &HashSet<ComponentId>,
) -> Result<(), ViewRuntimeError> {
    let stores: Vec<FieldId> = bytecode
        .bytecode
        .iter()
        .filter_map(|op| match *op {
            Op::StoreField(index) => Some(bytecode.field_map[usize::from(index)]),
            _ => None,
        })
        .collect();

    match intent {
        ProgramIntent::ReadOnly => {
            if stores.is_empty() {
                Ok(())
            } else {
                Err(ViewRuntimeError::StoreInReadOnlyProgram)
            }
        }
        ProgramIntent::Assignment { destination } => {
            if stores.len() != 1 {
                return Err(ViewRuntimeError::AssignmentStoreCount {
                    actual: stores.len(),
                });
            }
            let actual = stores[0];
            if actual != destination {
                return Err(ViewRuntimeError::AssignmentDestinationMismatch {
                    expected: destination,
                    actual,
                });
            }
            if !mutable_components.contains(&destination.component_id) {
                return Err(ViewRuntimeError::AssignmentDestinationReadOnly(
                    destination.component_id,
                ));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRIDE: usize = 8;

    fn field(component_id: ComponentId, offset: usize, field_type: FieldType) -> FieldId {
        FieldId {
            component_id,
            offset,
            field_type,
        }
    }

    fn filter(component_id: ComponentId) -> ViewFilter {
        ViewFilter {
            component_ids: HashSet::from([component_id]),
            with_ids: Vec::new(),
            without_ids: Vec::new(),
            changed_ids: Vec::new(),
            added_ids: Vec::new(),
        }
    }

    fn runtime(component_id: ComponentId, mutable: bool) -> ViewRuntimeCore {
        let mutable_components = if mutable {
            HashSet::from([component_id])
        } else {
            HashSet::new()
        };
        let allowed_fields = HashMap::from([(
            component_id,
            HashSet::from([
                (0, FieldType::F32),
                (4, FieldType::Bool),
                (5, FieldType::Bool),
            ]),
        )]);
        let component_strides = HashMap::from([(component_id, STRIDE)]);
        let spec = ResolvedViewSpec::new(
            filter(component_id),
            mutable_components,
            allowed_fields,
            component_strides,
        )
        .unwrap();
        ViewRuntimeCore::new(Arc::new(CachedViewCore::new(spec)))
    }

    fn read_program(field: FieldId) -> Arc<CompiledBytecode> {
        Arc::new(CompiledBytecode {
            bytecode: vec![Op::PushField(0)],
            constants: Vec::new(),
            field_map: vec![field],
        })
    }

    fn assignment_program(destination: FieldId) -> Arc<CompiledBytecode> {
        Arc::new(CompiledBytecode {
            bytecode: vec![Op::PushConst(0), Op::StoreField(0)],
            constants: vec![42.0],
            field_map: vec![destination],
        })
    }

    #[test]
    fn resolved_spec_normalizes_empty_allowed_field_sets() {
        let component_id = ComponentId::new(1);
        let spec = ResolvedViewSpec::new(
            filter(component_id),
            HashSet::new(),
            HashMap::new(),
            HashMap::from([(component_id, 0)]),
        )
        .unwrap();

        assert_eq!(spec.allowed_fields()[&component_id], HashSet::new());
    }

    #[test]
    fn resolved_spec_rejects_mutability_for_undeclared_component() {
        let declared = ComponentId::new(1);
        let forged = ComponentId::new(2);
        let result = ResolvedViewSpec::new(
            filter(declared),
            HashSet::from([forged]),
            HashMap::new(),
            HashMap::from([(declared, STRIDE)]),
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::SpecComponentNotDeclared {
                component_id,
                metadata: "mutable component",
            }) if component_id == forged
        ));
    }

    #[test]
    fn resolved_spec_rejects_fields_for_undeclared_component() {
        let declared = ComponentId::new(1);
        let forged = ComponentId::new(2);
        let result = ResolvedViewSpec::new(
            filter(declared),
            HashSet::new(),
            HashMap::from([(forged, HashSet::from([(0, FieldType::F32)]))]),
            HashMap::from([(declared, STRIDE)]),
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::SpecComponentNotDeclared {
                component_id,
                metadata: "allowed field",
            }) if component_id == forged
        ));
    }

    #[test]
    fn resolved_spec_requires_exact_declared_stride_set() {
        let declared = ComponentId::new(1);
        let extra = ComponentId::new(2);

        let missing = ResolvedViewSpec::new(
            filter(declared),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );
        assert!(matches!(
            missing,
            Err(ViewRuntimeError::MissingComponentStride(id)) if id == declared
        ));

        let extra_result = ResolvedViewSpec::new(
            filter(declared),
            HashSet::new(),
            HashMap::new(),
            HashMap::from([(declared, STRIDE), (extra, STRIDE)]),
        );
        assert!(matches!(
            extra_result,
            Err(ViewRuntimeError::SpecComponentNotDeclared {
                component_id,
                metadata: "component stride",
            }) if component_id == extra
        ));
    }

    #[test]
    fn resolved_spec_rejects_out_of_bounds_and_overflowing_fields() {
        let component_id = ComponentId::new(1);
        for offset in [STRIDE, usize::MAX] {
            let result = ResolvedViewSpec::new(
                filter(component_id),
                HashSet::new(),
                HashMap::from([(component_id, HashSet::from([(offset, FieldType::F32)]))]),
                HashMap::from([(component_id, STRIDE)]),
            );
            assert!(matches!(
                result,
                Err(ViewRuntimeError::Engine(
                    ViewEngineError::FieldOffsetOutOfBounds { .. }
                ))
            ));
        }
    }

    #[test]
    fn resolved_spec_rejects_unexpanded_vector_fields() {
        let component_id = ComponentId::new(1);
        let result = ResolvedViewSpec::new(
            filter(component_id),
            HashSet::new(),
            HashMap::from([(component_id, HashSet::from([(0, FieldType::Vec2)]))]),
            HashMap::from([(component_id, STRIDE)]),
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::CompositeFieldNotExpanded {
                component_id: id,
                offset: 0,
                field_type: FieldType::Vec2,
            }) if id == component_id
        ));
    }

    #[test]
    fn read_only_validation_accepts_declared_real_field() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, false);
        let bytecode = read_program(field(component_id, 0, FieldType::F32));

        let program = runtime
            .validate_program(Arc::clone(&bytecode), ProgramIntent::ReadOnly)
            .unwrap();

        assert!(std::ptr::eq(program.bytecode(), bytecode.as_ref()));
        assert_eq!(program.intent(), ProgramIntent::ReadOnly);
        assert!(runtime.check_program(&program).is_ok());
    }

    #[test]
    fn assignment_validation_accepts_one_exact_mutable_store() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, true);

        let program = runtime
            .validate_program(
                assignment_program(destination),
                ProgramIntent::Assignment { destination },
            )
            .unwrap();

        assert_eq!(program.intent(), ProgramIntent::Assignment { destination });
    }

    #[test]
    fn read_only_validation_rejects_forged_store() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, true);
        let result = runtime.validate_program(
            assignment_program(field(component_id, 0, FieldType::F32)),
            ProgramIntent::ReadOnly,
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::StoreInReadOnlyProgram)
        ));
    }

    #[test]
    fn assignment_validation_requires_exactly_one_store() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, true);

        let no_store = runtime.validate_program(
            read_program(destination),
            ProgramIntent::Assignment { destination },
        );
        assert!(matches!(
            no_store,
            Err(ViewRuntimeError::AssignmentStoreCount { actual: 0 })
        ));

        let two_stores = Arc::new(CompiledBytecode {
            bytecode: vec![
                Op::PushConst(0),
                Op::StoreField(0),
                Op::PushConst(0),
                Op::StoreField(0),
            ],
            constants: vec![1.0],
            field_map: vec![destination],
        });
        let result =
            runtime.validate_program(two_stores, ProgramIntent::Assignment { destination });
        assert!(matches!(
            result,
            Err(ViewRuntimeError::AssignmentStoreCount { actual: 2 })
        ));
    }

    #[test]
    fn assignment_validation_rejects_different_destination() {
        let component_id = ComponentId::new(1);
        let declared = field(component_id, 0, FieldType::F32);
        let actual = field(component_id, 4, FieldType::Bool);
        let runtime = runtime(component_id, true);

        let result = runtime.validate_program(
            assignment_program(actual),
            ProgramIntent::Assignment {
                destination: declared,
            },
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::AssignmentDestinationMismatch {
                expected,
                actual: encoded,
            }) if expected == declared && encoded == actual
        ));
    }

    #[test]
    fn assignment_validation_rejects_read_only_destination() {
        let component_id = ComponentId::new(1);
        let destination = field(component_id, 0, FieldType::F32);
        let runtime = runtime(component_id, false);

        let result = runtime.validate_program(
            assignment_program(destination),
            ProgramIntent::Assignment { destination },
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::AssignmentDestinationReadOnly(id)) if id == component_id
        ));
    }

    #[test]
    fn validation_rejects_undeclared_component_before_field_lookup() {
        let declared = ComponentId::new(1);
        let forged = ComponentId::new(2);
        let runtime = runtime(declared, false);

        let result = runtime.validate_program(
            read_program(field(forged, 0, FieldType::F32)),
            ProgramIntent::ReadOnly,
        );

        assert!(matches!(
            result,
            Err(ViewRuntimeError::Engine(
                ViewEngineError::FieldComponentNotDeclared(id)
            )) if id == forged
        ));
    }

    #[test]
    fn validation_rejects_type_confusion_and_mid_field_offsets() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, false);

        for forged in [
            field(component_id, 0, FieldType::Bool),
            field(component_id, 2, FieldType::F32),
        ] {
            let result = runtime.validate_program(read_program(forged), ProgramIntent::ReadOnly);
            assert!(matches!(
                result,
                Err(ViewRuntimeError::Engine(
                    ViewEngineError::FieldTypeMismatch { .. }
                ))
            ));
        }
    }

    #[test]
    fn validation_rejects_invalid_instruction_indices() {
        let component_id = ComponentId::new(1);
        let runtime = runtime(component_id, true);

        for (op, instruction) in [
            (Op::PushField(1), "PushField"),
            (Op::StoreField(1), "StoreField"),
        ] {
            let result = runtime.validate_program(
                Arc::new(CompiledBytecode {
                    bytecode: vec![op],
                    constants: Vec::new(),
                    field_map: vec![field(component_id, 0, FieldType::F32)],
                }),
                ProgramIntent::ReadOnly,
            );
            assert!(matches!(
                result,
                Err(ViewRuntimeError::InvalidFieldIndex {
                    instruction: actual,
                    index: 1,
                    field_count: 1,
                }) if actual == instruction
            ));
        }

        let result = runtime.validate_program(
            Arc::new(CompiledBytecode {
                bytecode: vec![Op::PushConst(0)],
                constants: Vec::new(),
                field_map: Vec::new(),
            }),
            ProgramIntent::ReadOnly,
        );
        assert!(matches!(
            result,
            Err(ViewRuntimeError::InvalidConstantIndex {
                index: 0,
                constant_count: 0,
            })
        ));
    }

    #[test]
    fn validated_program_is_shared_across_runs_of_same_cache() {
        let component_id = ComponentId::new(1);
        let first = runtime(component_id, false);
        let second = ViewRuntimeCore::new(Arc::clone(first.cached()));
        let program = first
            .validate_program(
                read_program(field(component_id, 0, FieldType::F32)),
                ProgramIntent::ReadOnly,
            )
            .unwrap();

        assert!(first.check_program(&program).is_ok());
        assert!(second.check_program(&program).is_ok());
    }

    #[test]
    fn validated_program_is_rejected_by_equivalent_but_distinct_cache() {
        let component_id = ComponentId::new(1);
        let first = runtime(component_id, false);
        let second = runtime(component_id, false);
        let program = first
            .validate_program(
                read_program(field(component_id, 0, FieldType::F32)),
                ProgramIntent::ReadOnly,
            )
            .unwrap();

        assert!(matches!(
            second.check_program(&program),
            Err(ViewRuntimeError::ProgramFromDifferentRuntime)
        ));
    }
}
