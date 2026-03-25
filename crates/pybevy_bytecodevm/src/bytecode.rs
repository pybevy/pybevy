//! Bytecode VM for executing lazy expression trees in batch operations.
//!
//! This module provides a stack-based bytecode interpreter optimized for
//! executing mathematical expressions on component fields in tight loops.
//!
//! # Performance
//!
//! The bytecode VM is designed for minimal overhead (~2-5ns per operation):
//! - Linear bytecode array (perfect cache locality)
//! - Simple stack operations (no recursion)
//! - Read-only bytecode (perfect for par_iter_mut)
//! - Pre-resolved component field offsets
//!
//! # Example Bytecode
//!
//! Expression: `pos.x = pos.x + vel.x * dt`
//!
//! Bytecode:
//! ```text
//! PushField(0)      // Push pos.x
//! PushField(1)      // Push vel.x
//! PushConst(0)      // Push dt from constant pool
//! Mul               // vel.x * dt
//! Add               // pos.x + (vel.x * dt)
//! StoreField(0)     // Store to pos.x
//! ```

use std::collections::HashMap;

use bevy_ecs::component::ComponentId;

/// Value types that can be stored on the VM stack
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StackValue {
    /// Floating-point number (f64 for precision; f32 fields widen losslessly)
    Float(f64),
    /// Boolean value
    Bool(bool),
}

impl StackValue {
    /// Unwrap as float, panic if not a float
    #[inline]
    pub fn as_float(&self) -> f64 {
        match self {
            StackValue::Float(f) => *f,
            StackValue::Bool(_) => panic!("Expected float, got bool"),
        }
    }

    /// Unwrap as bool, panic if not a bool
    #[inline]
    pub fn as_bool(&self) -> bool {
        match self {
            StackValue::Bool(b) => *b,
            StackValue::Float(_) => panic!("Expected bool, got float"),
        }
    }
}

/// Bytecode instruction for the stack-based VM
#[derive(Debug, Clone, Copy)]
pub enum Op {
    /// Push a component field value onto the stack
    /// u16 is an index into the field map
    PushField(u16),

    /// Push a constant from the constant pool onto the stack
    /// u16 is an index into the constants array
    PushConst(u16),

    /// Pop two values, add them, push result
    Add,

    /// Pop two values, subtract (stack[n-1] - stack[n]), push result
    Sub,

    /// Pop two values, multiply them, push result
    Mul,

    /// Pop two values, divide (stack[n-1] / stack[n]), push result
    Div,

    /// Pop two values, raise to power (stack[n-1] ** stack[n]), push result
    Pow,

    /// Pop value and store to a component field
    /// u16 is an index into the field map
    StoreField(u16),

    /// Negate the top stack value (unary minus)
    Neg,

    /// Pop value, push sin(value)
    Sin,

    /// Pop value, push cos(value)
    Cos,

    /// Pop value, push tan(value)
    Tan,

    /// Pop value, push asin(value)
    Asin,

    /// Pop value, push acos(value)
    Acos,

    /// Pop value, push atan(value)
    Atan,

    /// Pop value, push sqrt(value)
    Sqrt,

    /// Pop value, push abs(value)
    Abs,

    /// Pop value, push floor(value)
    Floor,

    /// Pop value, push ceil(value)
    Ceil,

    /// Pop value, push round(value)
    Round,

    /// Pop two values, push min(a, b)
    Min,

    /// Pop two values, push max(a, b)
    Max,

    /// Pop three values (max, min, value), push clamp(value, min, max)
    Clamp,

    /// Pop two floats, push bool (a == b)
    Eq,

    /// Pop two floats, push bool (a != b)
    Ne,

    /// Pop two floats, push bool (a < b)
    Lt,

    /// Pop two floats, push bool (a <= b)
    Le,

    /// Pop two floats, push bool (a > b)
    Gt,

    /// Pop two floats, push bool (a >= b)
    Ge,

    /// Conditional selection: where(condition, true_value, false_value)
    /// Stack before: [false_value: float, true_value: float, condition: bool]
    /// Stack after: [result: float]
    Where,

    /// Pop two bools, push (a && b)
    And,

    /// Pop two bools, push (a || b)
    Or,

    /// Pop one bool, push (!a)
    Not,

    /// Pop value, push e^value
    Exp,

    /// Pop value, push ln(value) (natural logarithm)
    Ln,

    /// Pop value, push log10(value)
    Log10,

    /// Pop value, push log2(value)
    Log2,

    /// Pop value, push sign(value) (-1.0, 0.0, or 1.0)
    Sign,

    /// Linear interpolation: lerp(a, b, t) = a + t * (b - a)
    /// Stack before: [t: float, b: float, a: float]
    /// Stack after: [result: float]
    Lerp,

    /// Pop value, push fract(value) (fractional part)
    Fract,

    /// Modulo operation: a % b
    /// Stack before: [b: float, a: float]
    /// Stack after: [result: float]
    Mod,

    /// Generate random float in [0.0, 1.0)
    /// Uses entity index as seed for deterministic per-entity randomness
    Random,

    /// Generate random float in [min, max)
    /// Stack before: [max: float, min: float]
    /// Stack after: [result: float]
    RandomRange,
}

/// Field types supported by the VM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldType {
    F32,
    F64,
    I32,
    I64,
    U32,
    U64,
    Bool,
    /// Composite signal type: 3 × f32 (12 bytes). VM decomposes to individual F32 sub-fields.
    Vec3,
    /// Composite signal type: 2 × f32 (8 bytes). VM decomposes to individual F32 sub-fields.
    Vec2,
}

impl FieldType {
    /// Size in bytes of this field type
    pub const fn size_bytes(&self) -> usize {
        match self {
            FieldType::F32 => 4,
            FieldType::F64 => 8,
            FieldType::I32 => 4,
            FieldType::I64 => 8,
            FieldType::U32 => 4,
            FieldType::U64 => 8,
            FieldType::Bool => 1,
            FieldType::Vec3 => 12,
            FieldType::Vec2 => 8,
        }
    }
}

/// Identifies a specific field within a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId {
    /// The Bevy ComponentId
    pub component_id: ComponentId,
    /// Byte offset within the component struct
    pub offset: usize,
    /// Type of the field
    pub field_type: FieldType,
}

/// Compiled bytecode ready for execution
#[derive(Debug, Clone)]
pub struct CompiledBytecode {
    /// The bytecode instructions
    pub bytecode: Vec<Op>,
    /// Constant pool (f64 values referenced by PushConst)
    pub constants: Vec<f64>,
    /// Maps field indices (used in bytecode) to actual component field locations
    pub field_map: Vec<FieldId>,
}

/// Compiler that converts expression AST to bytecode
pub struct Compiler {
    bytecode: Vec<Op>,
    constants: Vec<f64>,
    constant_map: HashMap<u64, u16>, // f64 bits -> constant pool index
    field_map: Vec<FieldId>,
    field_index_map: HashMap<FieldId, u16>, // FieldId -> field map index
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// Create a new bytecode compiler
    pub fn new() -> Self {
        Self {
            bytecode: Vec::new(),
            constants: Vec::new(),
            constant_map: HashMap::new(),
            field_map: Vec::new(),
            field_index_map: HashMap::new(),
        }
    }

    /// Emit a bytecode instruction
    pub fn emit(&mut self, op: Op) {
        self.bytecode.push(op);
    }

    /// Add a constant to the pool and return its index
    pub fn add_constant(&mut self, value: f64) -> u16 {
        // Use bit representation for HashMap key (handles NaN/±0.0 correctly)
        let bits = value.to_bits();

        if let Some(&index) = self.constant_map.get(&bits) {
            return index;
        }

        let index = self.constants.len() as u16;
        self.constants.push(value);
        self.constant_map.insert(bits, index);
        index
    }

    /// Add a field to the field map and return its index
    pub fn add_field(&mut self, field_id: FieldId) -> u16 {
        if let Some(&index) = self.field_index_map.get(&field_id) {
            return index;
        }

        let index = self.field_map.len() as u16;
        self.field_map.push(field_id);
        self.field_index_map.insert(field_id, index);
        index
    }

    /// Finalize compilation and return the bytecode
    pub fn finalize(self) -> CompiledBytecode {
        CompiledBytecode {
            bytecode: self.bytecode,
            constants: self.constants,
            field_map: self.field_map,
        }
    }

    /// Simple peephole optimization pass
    pub fn optimize(&mut self) {
        // Constant folding example:
        // PushConst(a), PushConst(b), Add -> PushConst(a+b)

        let mut i = 0;
        while i + 2 < self.bytecode.len() {
            match (
                &self.bytecode[i],
                &self.bytecode[i + 1],
                &self.bytecode[i + 2],
            ) {
                (Op::PushConst(a), Op::PushConst(b), Op::Add) => {
                    let val_a = self.constants[*a as usize];
                    let val_b = self.constants[*b as usize];
                    let result = val_a + val_b;
                    let result_idx = self.add_constant(result);

                    self.bytecode[i] = Op::PushConst(result_idx);
                    self.bytecode.remove(i + 1);
                    self.bytecode.remove(i + 1);
                    // Don't increment i - check this position again
                }
                (Op::PushConst(a), Op::PushConst(b), Op::Mul) => {
                    let val_a = self.constants[*a as usize];
                    let val_b = self.constants[*b as usize];
                    let result = val_a * val_b;
                    let result_idx = self.add_constant(result);

                    self.bytecode[i] = Op::PushConst(result_idx);
                    self.bytecode.remove(i + 1);
                    self.bytecode.remove(i + 1);
                }
                (Op::PushConst(a), Op::PushConst(b), Op::Sub) => {
                    let val_a = self.constants[*a as usize];
                    let val_b = self.constants[*b as usize];
                    let result = val_a - val_b;
                    let result_idx = self.add_constant(result);

                    self.bytecode[i] = Op::PushConst(result_idx);
                    self.bytecode.remove(i + 1);
                    self.bytecode.remove(i + 1);
                }
                (Op::PushConst(a), Op::PushConst(b), Op::Div) => {
                    let val_a = self.constants[*a as usize];
                    let val_b = self.constants[*b as usize];
                    if val_b != 0.0 {
                        let result = val_a / val_b;
                        let result_idx = self.add_constant(result);

                        self.bytecode[i] = Op::PushConst(result_idx);
                        self.bytecode.remove(i + 1);
                        self.bytecode.remove(i + 1);
                    } else {
                        i += 1;
                    }
                }
                (Op::PushConst(a), Op::PushConst(b), Op::Pow) => {
                    let val_a = self.constants[*a as usize];
                    let val_b = self.constants[*b as usize];
                    let result = val_a.powf(val_b);
                    let result_idx = self.add_constant(result);

                    self.bytecode[i] = Op::PushConst(result_idx);
                    self.bytecode.remove(i + 1);
                    self.bytecode.remove(i + 1);
                }
                _ => {
                    i += 1;
                }
            }
        }
    }
}

/// Read a field value from a raw pointer based on its type, returning f64.
///
/// # Safety
/// The pointer must be valid, aligned for the given field type, and not concurrently mutated.
#[inline(always)]
pub unsafe fn read_field_value(ptr: *const u8, field_type: FieldType) -> f64 {
    match field_type {
        FieldType::F32 => unsafe { *(ptr as *const f32) as f64 },
        FieldType::F64 => unsafe { *(ptr as *const f64) },
        FieldType::I32 => unsafe { *(ptr as *const i32) as f64 },
        FieldType::I64 => unsafe { *(ptr as *const i64) as f64 },
        FieldType::U32 => unsafe { *(ptr as *const u32) as f64 },
        FieldType::U64 => unsafe { *(ptr as *const u64) as f64 },
        FieldType::Bool => unsafe { if *(ptr as *const bool) { 1.0 } else { 0.0 } },
        // Vec3/Vec2 are composite signal types — the VM decomposes them to individual F32 sub-fields
        // before execution, so these should never appear in read_field_value
        FieldType::Vec3 | FieldType::Vec2 => {
            unreachable!("VM should decompose Vec3/Vec2 to F32 sub-fields")
        }
    }
}

/// Write an f64 value to a raw pointer, converting to the target field type.
///
/// # Safety
/// The pointer must be valid, aligned for the given field type, and not concurrently read.
#[inline(always)]
pub unsafe fn write_field_value(ptr: *mut u8, value: f64, field_type: FieldType) {
    match field_type {
        FieldType::F32 => unsafe {
            *(ptr as *mut f32) = value as f32;
        },
        FieldType::F64 => unsafe {
            *(ptr as *mut f64) = value;
        },
        FieldType::I32 => unsafe {
            *(ptr as *mut i32) = value as i32;
        },
        FieldType::I64 => unsafe {
            *(ptr as *mut i64) = value as i64;
        },
        FieldType::U32 => unsafe {
            *(ptr as *mut u32) = value as u32;
        },
        FieldType::U64 => unsafe {
            *(ptr as *mut u64) = value as u64;
        },
        FieldType::Bool => unsafe {
            *(ptr as *mut bool) = value >= 0.5;
        },
        FieldType::Vec3 | FieldType::Vec2 => {
            unreachable!("VM should decompose Vec3/Vec2 to F32 sub-fields")
        }
    }
}

/// Stack-based virtual machine for executing bytecode
pub struct VM {
    /// Evaluation stack (supports both floats and booleans)
    pub(crate) stack: Vec<StackValue>,
    /// Current entity index for deterministic random number generation
    entity_index: usize,
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-local VM pool for reuse across entities
thread_local! {
    static VM_POOL: std::cell::RefCell<Vec<VM>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// RAII guard that returns VM to pool on drop
pub struct PooledVM {
    vm: Option<VM>,
}

impl PooledVM {
    /// Get a VM from the thread-local pool, or create a new one
    pub fn acquire() -> Self {
        let vm = VM_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_default();
        Self { vm: Some(vm) }
    }

    /// Get mutable reference to the VM
    #[inline]
    pub fn get_mut(&mut self) -> &mut VM {
        self.vm.as_mut().expect("VM already released")
    }
}

impl Drop for PooledVM {
    fn drop(&mut self) {
        if let Some(vm) = self.vm.take() {
            VM_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                // Keep pool size bounded to avoid memory leaks
                if pool.len() < 16 {
                    pool.push(vm);
                }
            });
        }
    }
}

impl VM {
    /// Create a new VM with pre-allocated stack
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(32), // Most expressions need < 32 stack slots
            entity_index: 0,
        }
    }

    /// Reset VM state for reuse (keeps stack allocation)
    #[inline]
    pub fn reset(&mut self) {
        self.stack.clear();
        self.entity_index = 0;
    }

    /// Simple hash-based random number generator for deterministic per-entity randomness
    /// Uses the entity index as seed to ensure reproducibility
    #[inline]
    fn random(&self) -> f64 {
        // Simple LCG-style hash for deterministic randomness
        let mut x = self.entity_index as u32;
        x = x.wrapping_mul(747796405).wrapping_add(2891336453);
        x = (x ^ (x >> 13)).wrapping_mul(1597334677);
        x = x ^ (x >> 16);

        // Convert to [0.0, 1.0)
        (x as f64) / (u32::MAX as f64)
    }

    /// Dispatch a single stack-only operation (everything except PushField/StoreField).
    ///
    /// Returns `false` for PushField and StoreField, which require pointer-dependent
    /// handling by the caller. Returns `true` for all other ops.
    #[inline(always)]
    fn dispatch_stack_op(&mut self, op: &Op, bytecode: &CompiledBytecode) -> bool {
        match op {
            Op::PushField(_) | Op::StoreField(_) => return false,

            Op::PushConst(const_idx) => {
                self.stack
                    .push(StackValue::Float(bytecode.constants[*const_idx as usize]));
            }

            Op::Add => {
                let b = self.stack.pop().expect("Stack underflow on Add").as_float();
                let a = self.stack.pop().expect("Stack underflow on Add").as_float();
                self.stack.push(StackValue::Float(a + b));
            }
            Op::Sub => {
                let b = self.stack.pop().expect("Stack underflow on Sub").as_float();
                let a = self.stack.pop().expect("Stack underflow on Sub").as_float();
                self.stack.push(StackValue::Float(a - b));
            }
            Op::Mul => {
                let b = self.stack.pop().expect("Stack underflow on Mul").as_float();
                let a = self.stack.pop().expect("Stack underflow on Mul").as_float();
                self.stack.push(StackValue::Float(a * b));
            }
            Op::Div => {
                let b = self.stack.pop().expect("Stack underflow on Div").as_float();
                let a = self.stack.pop().expect("Stack underflow on Div").as_float();
                self.stack.push(StackValue::Float(a / b));
            }
            Op::Pow => {
                let b = self.stack.pop().expect("Stack underflow on Pow").as_float();
                let a = self.stack.pop().expect("Stack underflow on Pow").as_float();
                self.stack.push(StackValue::Float(a.powf(b)));
            }
            Op::Mod => {
                let b = self.stack.pop().expect("Stack underflow on Mod").as_float();
                let a = self.stack.pop().expect("Stack underflow on Mod").as_float();
                self.stack.push(StackValue::Float(a % b));
            }
            Op::Neg => {
                let a = self.stack.pop().expect("Stack underflow on Neg").as_float();
                self.stack.push(StackValue::Float(-a));
            }

            // Trigonometric
            Op::Sin => {
                let x = self.stack.pop().expect("Stack underflow on Sin").as_float();
                self.stack.push(StackValue::Float(x.sin()));
            }
            Op::Cos => {
                let x = self.stack.pop().expect("Stack underflow on Cos").as_float();
                self.stack.push(StackValue::Float(x.cos()));
            }
            Op::Tan => {
                let x = self.stack.pop().expect("Stack underflow on Tan").as_float();
                self.stack.push(StackValue::Float(x.tan()));
            }
            Op::Asin => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Asin")
                    .as_float();
                self.stack.push(StackValue::Float(x.asin()));
            }
            Op::Acos => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Acos")
                    .as_float();
                self.stack.push(StackValue::Float(x.acos()));
            }
            Op::Atan => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Atan")
                    .as_float();
                self.stack.push(StackValue::Float(x.atan()));
            }

            // Math
            Op::Sqrt => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Sqrt")
                    .as_float();
                self.stack.push(StackValue::Float(x.sqrt()));
            }
            Op::Abs => {
                let x = self.stack.pop().expect("Stack underflow on Abs").as_float();
                self.stack.push(StackValue::Float(x.abs()));
            }
            Op::Floor => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Floor")
                    .as_float();
                self.stack.push(StackValue::Float(x.floor()));
            }
            Op::Ceil => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Ceil")
                    .as_float();
                self.stack.push(StackValue::Float(x.ceil()));
            }
            Op::Round => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Round")
                    .as_float();
                self.stack.push(StackValue::Float(x.round()));
            }
            Op::Exp => {
                let x = self.stack.pop().expect("Stack underflow on Exp").as_float();
                self.stack.push(StackValue::Float(x.exp()));
            }
            Op::Ln => {
                let x = self.stack.pop().expect("Stack underflow on Ln").as_float();
                self.stack.push(StackValue::Float(x.ln()));
            }
            Op::Log10 => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Log10")
                    .as_float();
                self.stack.push(StackValue::Float(x.log10()));
            }
            Op::Log2 => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Log2")
                    .as_float();
                self.stack.push(StackValue::Float(x.log2()));
            }
            Op::Sign => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Sign")
                    .as_float();
                let sign = if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                };
                self.stack.push(StackValue::Float(sign));
            }
            Op::Fract => {
                let x = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Fract")
                    .as_float();
                self.stack.push(StackValue::Float(x.fract()));
            }
            Op::Lerp => {
                let t = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Lerp")
                    .as_float();
                let b = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Lerp")
                    .as_float();
                let a = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Lerp")
                    .as_float();
                self.stack.push(StackValue::Float(a + t * (b - a)));
            }

            // Min/Max/Clamp
            Op::Min => {
                let b = self.stack.pop().expect("Stack underflow on Min").as_float();
                let a = self.stack.pop().expect("Stack underflow on Min").as_float();
                self.stack.push(StackValue::Float(a.min(b)));
            }
            Op::Max => {
                let b = self.stack.pop().expect("Stack underflow on Max").as_float();
                let a = self.stack.pop().expect("Stack underflow on Max").as_float();
                self.stack.push(StackValue::Float(a.max(b)));
            }
            Op::Clamp => {
                let max = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Clamp")
                    .as_float();
                let min = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Clamp")
                    .as_float();
                let value = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Clamp")
                    .as_float();
                self.stack.push(StackValue::Float(value.clamp(min, max)));
            }

            // Comparison (exact equality — consistent across all execution modes)
            Op::Eq => {
                let b = self.stack.pop().expect("Stack underflow on Eq").as_float();
                let a = self.stack.pop().expect("Stack underflow on Eq").as_float();
                self.stack.push(StackValue::Bool(a == b));
            }
            Op::Ne => {
                let b = self.stack.pop().expect("Stack underflow on Ne").as_float();
                let a = self.stack.pop().expect("Stack underflow on Ne").as_float();
                self.stack.push(StackValue::Bool(a != b));
            }
            Op::Lt => {
                let b = self.stack.pop().expect("Stack underflow on Lt").as_float();
                let a = self.stack.pop().expect("Stack underflow on Lt").as_float();
                self.stack.push(StackValue::Bool(a < b));
            }
            Op::Le => {
                let b = self.stack.pop().expect("Stack underflow on Le").as_float();
                let a = self.stack.pop().expect("Stack underflow on Le").as_float();
                self.stack.push(StackValue::Bool(a <= b));
            }
            Op::Gt => {
                let b = self.stack.pop().expect("Stack underflow on Gt").as_float();
                let a = self.stack.pop().expect("Stack underflow on Gt").as_float();
                self.stack.push(StackValue::Bool(a > b));
            }
            Op::Ge => {
                let b = self.stack.pop().expect("Stack underflow on Ge").as_float();
                let a = self.stack.pop().expect("Stack underflow on Ge").as_float();
                self.stack.push(StackValue::Bool(a >= b));
            }

            // Logical
            Op::And => {
                let b = self.stack.pop().expect("Stack underflow on And").as_bool();
                let a = self.stack.pop().expect("Stack underflow on And").as_bool();
                self.stack.push(StackValue::Bool(a && b));
            }
            Op::Or => {
                let b = self.stack.pop().expect("Stack underflow on Or").as_bool();
                let a = self.stack.pop().expect("Stack underflow on Or").as_bool();
                self.stack.push(StackValue::Bool(a || b));
            }
            Op::Not => {
                let a = self.stack.pop().expect("Stack underflow on Not").as_bool();
                self.stack.push(StackValue::Bool(!a));
            }

            // Conditional
            Op::Where => {
                let false_val = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Where")
                    .as_float();
                let true_val = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Where")
                    .as_float();
                let condition = self
                    .stack
                    .pop()
                    .expect("Stack underflow on Where")
                    .as_bool();
                self.stack.push(StackValue::Float(if condition {
                    true_val
                } else {
                    false_val
                }));
            }

            // Random
            Op::Random => {
                self.stack.push(StackValue::Float(self.random()));
            }
            Op::RandomRange => {
                let max = self
                    .stack
                    .pop()
                    .expect("Stack underflow on RandomRange")
                    .as_float();
                let min = self
                    .stack
                    .pop()
                    .expect("Stack underflow on RandomRange")
                    .as_float();
                let rand = self.random();
                self.stack.push(StackValue::Float(min + rand * (max - min)));
            }
        }
        true
    }

    /// Execute bytecode on a single entity
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - All field pointers in `field_ptrs` are valid and aligned for their respective types
    /// - Field pointers remain valid for the duration of this call
    /// - No other code mutates the same memory during execution
    /// - Each pointer's alignment matches the requirements of its FieldType (8 bytes for f64/i64/u64)
    pub unsafe fn execute(
        &mut self,
        bytecode: &CompiledBytecode,
        field_ptrs: &[*mut u8],
        entity_index: usize,
    ) {
        self.stack.clear();
        self.entity_index = entity_index;

        for op in &bytecode.bytecode {
            if self.dispatch_stack_op(op, bytecode) {
                continue;
            }
            match op {
                Op::PushField(field_idx) => {
                    let field_id = &bytecode.field_map[*field_idx as usize];
                    let ptr = field_ptrs[*field_idx as usize];
                    let value = unsafe { read_field_value(ptr, field_id.field_type) };
                    self.stack.push(StackValue::Float(value));
                }
                Op::StoreField(field_idx) => {
                    let field_id = &bytecode.field_map[*field_idx as usize];
                    let value = self
                        .stack
                        .pop()
                        .expect("Stack underflow on StoreField")
                        .as_float();
                    let ptr = field_ptrs[*field_idx as usize];
                    unsafe { write_field_value(ptr, value, field_id.field_type) };
                }
                _ => unreachable!(),
            }
        }
    }

    /// Execute bytecode in reduction mode and return the final value
    ///
    /// Evaluates the expression but does not store results - instead
    /// returns the top of the stack after execution.
    ///
    /// # Safety
    ///
    /// Same safety requirements as `execute()`:
    /// - All field pointers in `field_ptrs` must be valid and aligned for their respective types
    /// - Field pointers must remain valid for the duration of this call
    /// - Each pointer's alignment matches the requirements of its FieldType (8 bytes for f64/i64/u64)
    pub unsafe fn execute_and_reduce(
        &mut self,
        bytecode: &CompiledBytecode,
        field_ptrs: &[*mut u8],
        entity_index: usize,
    ) -> f64 {
        self.stack.clear();
        self.entity_index = entity_index;

        for op in &bytecode.bytecode {
            if self.dispatch_stack_op(op, bytecode) {
                continue;
            }
            match op {
                Op::PushField(field_idx) => {
                    let field_id = &bytecode.field_map[*field_idx as usize];
                    let ptr = field_ptrs[*field_idx as usize];
                    let value = unsafe { read_field_value(ptr, field_id.field_type) };
                    self.stack.push(StackValue::Float(value));
                }
                // In reduction mode, we don't store - just pop the value
                Op::StoreField(_) => {
                    self.stack
                        .pop()
                        .expect("Stack underflow on StoreField in reduce mode");
                }
                _ => unreachable!(),
            }
        }

        // Return the top of stack as the final result
        // Convert bool to float (true=1.0, false=0.0) for reduction operations
        let result = self.stack.pop().expect("Stack empty after reduction");
        match result {
            StackValue::Float(f) => f,
            StackValue::Bool(b) => {
                if b {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Try to execute using a fast path for common patterns.
    /// Returns true if a fast path was used, false otherwise.
    ///
    /// Fast paths bypass the bytecode interpreter entirely for simple patterns:
    /// - `field = field + const` (compound add)
    /// - `field = field * const` (compound multiply)
    /// - `field = const` (simple assignment)
    #[inline]
    unsafe fn try_fast_path(
        &self,
        bytecode: &CompiledBytecode,
        base_ptr: *mut u8,
        component_stride: usize,
        count: usize,
    ) -> bool {
        unsafe {
            let ops = &bytecode.bytecode;

            // Pattern: field = field + const (4 ops: PushField, PushConst, Add, StoreField)
            if ops.len() == 4 {
                if let (
                    Op::PushField(read_idx),
                    Op::PushConst(const_idx),
                    Op::Add,
                    Op::StoreField(write_idx),
                ) = (&ops[0], &ops[1], &ops[2], &ops[3])
                    && read_idx == write_idx
                {
                    let field_id = &bytecode.field_map[*read_idx as usize];
                    let constant = bytecode.constants[*const_idx as usize];

                    if field_id.field_type == FieldType::F32 {
                        // Fast path: field += const for f32
                        let constant_f32 = constant as f32;
                        let offset = field_id.offset;
                        let chunks = count / 4;
                        let remainder = count % 4;

                        for chunk in 0..chunks {
                            let base = chunk * 4;
                            let p0 = base_ptr.add(base * component_stride + offset) as *mut f32;
                            let p1 =
                                base_ptr.add((base + 1) * component_stride + offset) as *mut f32;
                            let p2 =
                                base_ptr.add((base + 2) * component_stride + offset) as *mut f32;
                            let p3 =
                                base_ptr.add((base + 3) * component_stride + offset) as *mut f32;
                            *p0 += constant_f32;
                            *p1 += constant_f32;
                            *p2 += constant_f32;
                            *p3 += constant_f32;
                        }
                        for i in (chunks * 4)..(chunks * 4 + remainder) {
                            let ptr = base_ptr.add(i * component_stride + offset) as *mut f32;
                            *ptr += constant_f32;
                        }
                        return true;
                    } else if field_id.field_type == FieldType::F64 {
                        // Fast path: field += const for f64
                        let offset = field_id.offset;
                        let chunks = count / 4;
                        let remainder = count % 4;

                        for chunk in 0..chunks {
                            let base = chunk * 4;
                            let p0 = base_ptr.add(base * component_stride + offset) as *mut f64;
                            let p1 =
                                base_ptr.add((base + 1) * component_stride + offset) as *mut f64;
                            let p2 =
                                base_ptr.add((base + 2) * component_stride + offset) as *mut f64;
                            let p3 =
                                base_ptr.add((base + 3) * component_stride + offset) as *mut f64;
                            *p0 += constant;
                            *p1 += constant;
                            *p2 += constant;
                            *p3 += constant;
                        }
                        for i in (chunks * 4)..(chunks * 4 + remainder) {
                            let ptr = base_ptr.add(i * component_stride + offset) as *mut f64;
                            *ptr += constant;
                        }
                        return true;
                    }
                }

                // Pattern: field = field * const
                if let (
                    Op::PushField(read_idx),
                    Op::PushConst(const_idx),
                    Op::Mul,
                    Op::StoreField(write_idx),
                ) = (&ops[0], &ops[1], &ops[2], &ops[3])
                    && read_idx == write_idx
                {
                    let field_id = &bytecode.field_map[*read_idx as usize];
                    let constant = bytecode.constants[*const_idx as usize];

                    if field_id.field_type == FieldType::F32 {
                        // Fast path: field *= const for f32
                        let constant_f32 = constant as f32;
                        for i in 0..count {
                            let ptr =
                                base_ptr.add(i * component_stride + field_id.offset) as *mut f32;
                            *ptr *= constant_f32;
                        }
                        return true;
                    } else if field_id.field_type == FieldType::F64 {
                        // Fast path: field *= const for f64
                        for i in 0..count {
                            let ptr =
                                base_ptr.add(i * component_stride + field_id.offset) as *mut f64;
                            *ptr *= constant;
                        }
                        return true;
                    }
                }
            }

            // Pattern: field = const (2 ops: PushConst, StoreField)
            if ops.len() == 2
                && let (Op::PushConst(const_idx), Op::StoreField(field_idx)) = (&ops[0], &ops[1])
            {
                let field_id = &bytecode.field_map[*field_idx as usize];
                let constant = bytecode.constants[*const_idx as usize];

                if field_id.field_type == FieldType::F32 {
                    // Fast path: field = const for f32
                    let constant_f32 = constant as f32;
                    for i in 0..count {
                        let ptr = base_ptr.add(i * component_stride + field_id.offset) as *mut f32;
                        *ptr = constant_f32;
                    }
                    return true;
                } else if field_id.field_type == FieldType::F64 {
                    // Fast path: field = const for f64
                    for i in 0..count {
                        let ptr = base_ptr.add(i * component_stride + field_id.offset) as *mut f64;
                        *ptr = constant;
                    }
                    return true;
                }
            }

            false
        }
    }

    /// Execute bytecode on a batch of entities using stride-based pointer arithmetic.
    ///
    /// This is the high-performance batch execution path that avoids per-entity:
    /// - VM allocation (reuses single VM across all entities)
    /// - Field pointer array allocation (computes pointers via stride)
    /// - Closure dispatch overhead (single call processes all entities)
    ///
    /// # Arguments
    ///
    /// * `bytecode` - Compiled bytecode to execute
    /// * `base_ptr` - Pointer to the first entity's component data
    /// * `component_stride` - Bytes between consecutive entities' component data
    /// * `count` - Number of entities to process
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - `base_ptr` points to valid component storage for `count` entities
    /// - `component_stride` matches the actual component layout
    /// - All field offsets in bytecode are valid within the component
    /// - No other code mutates the same memory during execution
    #[inline]
    pub unsafe fn execute_batch(
        &mut self,
        bytecode: &CompiledBytecode,
        base_ptr: *mut u8,
        component_stride: usize,
        count: usize,
    ) {
        unsafe {
            // Try fast paths for common patterns (bypasses bytecode interpreter)
            if self.try_fast_path(bytecode, base_ptr, component_stride, count) {
                return;
            }

            // Fallback: Process each entity using bytecode interpretation
            for entity_idx in 0..count {
                self.stack.clear();
                self.entity_index = entity_idx;

                // Calculate base pointer for this entity
                let entity_base = base_ptr.add(entity_idx * component_stride);

                for op in &bytecode.bytecode {
                    if self.dispatch_stack_op(op, bytecode) {
                        continue;
                    }
                    match op {
                        Op::PushField(field_idx) => {
                            let field_id = &bytecode.field_map[*field_idx as usize];
                            let ptr = entity_base.add(field_id.offset);
                            let value = read_field_value(ptr, field_id.field_type);
                            self.stack.push(StackValue::Float(value));
                        }
                        Op::StoreField(field_idx) => {
                            let field_id = &bytecode.field_map[*field_idx as usize];
                            let value = self
                                .stack
                                .pop()
                                .expect("Stack underflow on StoreField")
                                .as_float();
                            let ptr = entity_base.add(field_id.offset);
                            write_field_value(ptr, value, field_id.field_type);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    /// Execute bytecode in batch mode with multiple components.
    ///
    /// Each field can have a different base pointer and stride, allowing
    /// cross-component expressions like `pos.x = pos.x + vel.x`.
    ///
    /// # Arguments
    /// * `bytecode` - The compiled bytecode to execute
    /// * `field_bases` - Base pointer for each field (component_base + field_offset)
    /// * `field_strides` - Stride for each field's component
    /// * `count` - Number of entities to process
    ///
    /// # Safety
    /// - All pointers in field_bases must be valid for count * stride bytes
    /// - All field types must match the actual memory layout
    #[inline]
    pub unsafe fn execute_batch_multi(
        &mut self,
        bytecode: &CompiledBytecode,
        field_bases: &[*mut u8],
        field_strides: &[usize],
        count: usize,
    ) {
        unsafe {
            debug_assert_eq!(field_bases.len(), bytecode.field_map.len());
            debug_assert_eq!(field_strides.len(), bytecode.field_map.len());

            for entity_idx in 0..count {
                self.stack.clear();
                self.entity_index = entity_idx;

                for op in &bytecode.bytecode {
                    if self.dispatch_stack_op(op, bytecode) {
                        continue;
                    }
                    match op {
                        Op::PushField(field_idx) => {
                            let idx = *field_idx as usize;
                            let field_id = &bytecode.field_map[idx];
                            let ptr = field_bases[idx].add(entity_idx * field_strides[idx]);
                            let value = read_field_value(ptr, field_id.field_type);
                            self.stack.push(StackValue::Float(value));
                        }
                        Op::StoreField(field_idx) => {
                            let idx = *field_idx as usize;
                            let field_id = &bytecode.field_map[idx];
                            let value = self
                                .stack
                                .pop()
                                .expect("Stack underflow on StoreField")
                                .as_float();
                            let ptr = field_bases[idx].add(entity_idx * field_strides[idx]);
                            write_field_value(ptr, value, field_id.field_type);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}
