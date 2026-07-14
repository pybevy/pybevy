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

pub use pybevy_storage::FieldType;

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
/// Uses `read_unaligned` for types wider than 4 bytes because ECS column
/// storage may not guarantee 8-byte alignment on 32-bit platforms.
///
/// # Safety
/// The pointer must be valid and not concurrently mutated.
#[inline(always)]
pub unsafe fn read_field_value(ptr: *const u8, field_type: FieldType) -> f64 {
    match field_type {
        FieldType::F32 => unsafe { (ptr as *const f32).read_unaligned() as f64 },
        FieldType::F64 => unsafe { (ptr as *const f64).read_unaligned() },
        FieldType::I32 => unsafe { (ptr as *const i32).read_unaligned() as f64 },
        FieldType::I64 => unsafe { (ptr as *const i64).read_unaligned() as f64 },
        FieldType::U32 => unsafe { (ptr as *const u32).read_unaligned() as f64 },
        FieldType::U64 => unsafe { (ptr as *const u64).read_unaligned() as f64 },
        FieldType::Bool => unsafe { if *(ptr as *const bool) { 1.0 } else { 0.0 } },
        // Vec2/Vec3/Vec4 are composite signal types — the VM decomposes them to individual F32 sub-fields
        // before execution, so these should never appear in read_field_value
        FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => {
            unreachable!("VM should decompose Vec2/Vec3/Vec4 to F32 sub-fields")
        }
    }
}

/// Write an f64 value to a raw pointer, converting to the target field type.
///
/// Uses `write_unaligned` for types wider than 4 bytes (see [`read_field_value`]).
///
/// # Safety
/// The pointer must be valid and not concurrently read.
#[inline(always)]
pub unsafe fn write_field_value(ptr: *mut u8, value: f64, field_type: FieldType) {
    match field_type {
        FieldType::F32 => unsafe {
            (ptr as *mut f32).write_unaligned(value as f32);
        },
        FieldType::F64 => unsafe {
            (ptr as *mut f64).write_unaligned(value);
        },
        FieldType::I32 => unsafe {
            (ptr as *mut i32).write_unaligned(value as i32);
        },
        FieldType::I64 => unsafe {
            (ptr as *mut i64).write_unaligned(value as i64);
        },
        FieldType::U32 => unsafe {
            (ptr as *mut u32).write_unaligned(value as u32);
        },
        FieldType::U64 => unsafe {
            (ptr as *mut u64).write_unaligned(value as u64);
        },
        FieldType::Bool => unsafe {
            *(ptr as *mut bool) = value >= 0.5;
        },
        FieldType::Vec2 | FieldType::Vec3 | FieldType::Vec4 => {
            unreachable!("VM should decompose Vec2/Vec3/Vec4 to F32 sub-fields")
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

    /// Check if this guard still holds a VM (for testing)
    #[cfg(test)]
    pub fn has_vm(&self) -> bool {
        self.vm.is_some()
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
    ///
    /// # Safety
    ///
    /// Same contract as [`Self::execute_batch`]: `base_ptr` must point to valid
    /// component storage for `count` entities, `component_stride` must match the
    /// actual component layout, all field offsets in bytecode must be valid within
    /// the component, and no other code may mutate the same memory during execution.
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
                            p0.write_unaligned(p0.read_unaligned() + constant_f32);
                            p1.write_unaligned(p1.read_unaligned() + constant_f32);
                            p2.write_unaligned(p2.read_unaligned() + constant_f32);
                            p3.write_unaligned(p3.read_unaligned() + constant_f32);
                        }
                        for i in (chunks * 4)..(chunks * 4 + remainder) {
                            let ptr = base_ptr.add(i * component_stride + offset) as *mut f32;
                            ptr.write_unaligned(ptr.read_unaligned() + constant_f32);
                        }
                        return true;
                    } else if field_id.field_type == FieldType::F64 {
                        // Fast path: field += const for f64
                        let offset = field_id.offset;
                        for i in 0..count {
                            let p = base_ptr.add(i * component_stride + offset) as *mut f64;
                            p.write_unaligned(p.read_unaligned() + constant);
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
                            ptr.write_unaligned(ptr.read_unaligned() * constant_f32);
                        }
                        return true;
                    } else if field_id.field_type == FieldType::F64 {
                        // Fast path: field *= const for f64
                        for i in 0..count {
                            let p =
                                base_ptr.add(i * component_stride + field_id.offset) as *mut f64;
                            p.write_unaligned(p.read_unaligned() * constant);
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
                        ptr.write_unaligned(constant_f32);
                    }
                    return true;
                } else if field_id.field_type == FieldType::F64 {
                    // Fast path: field = const for f64
                    for i in 0..count {
                        let p = base_ptr.add(i * component_stride + field_id.offset) as *mut f64;
                        p.write_unaligned(constant);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_arithmetic() {
        let mut compiler = Compiler::new();

        // Expression: 5.0 + 3.0 * 2.0 = 11.0
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_2 = compiler.add_constant(2.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_2));
        compiler.emit(Op::Mul); // 3.0 * 2.0 = 6.0
        compiler.emit(Op::Add); // 5.0 + 6.0 = 11.0

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // No field access, just check the stack
        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 11.0);
    }

    #[test]
    fn test_constant_folding() {
        let mut compiler = Compiler::new();

        // Expression: 5.0 + 3.0 (should fold to 8.0)
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Add);

        compiler.optimize();

        let bytecode = compiler.finalize();

        // After optimization, should be just PushConst(8.0)
        assert_eq!(bytecode.bytecode.len(), 1);
        assert!(matches!(bytecode.bytecode[0], Op::PushConst(_)));
    }

    #[test]
    fn test_field_store() {
        let mut compiler = Compiler::new();

        // Expression: field[0] = field[0] + 10.0
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Create a test field
        let mut field_value = 5.0_f32;
        let field_ptr = &mut field_value as *mut f32 as *mut u8;

        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert_eq!(field_value, 15.0);
    }

    #[test]
    fn test_trig_functions() {
        use std::f64::consts::PI;
        let mut compiler = Compiler::new();

        // Test sin(PI/2) = 1.0
        let const_pi_2 = compiler.add_constant(PI / 2.0);
        compiler.emit(Op::PushConst(const_pi_2));
        compiler.emit(Op::Sin);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert!((vm.stack[0].as_float() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_numeric_functions() {
        let mut compiler = Compiler::new();

        // Test sqrt(16.0) = 4.0
        let const_16 = compiler.add_constant(16.0);
        compiler.emit(Op::PushConst(const_16));
        compiler.emit(Op::Sqrt);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 4.0);
    }

    #[test]
    fn test_abs_floor_ceil() {
        let mut compiler = Compiler::new();

        // Test abs(-5.3)
        let const_neg = compiler.add_constant(-5.3);
        compiler.emit(Op::PushConst(const_neg));
        compiler.emit(Op::Abs);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 5.3);
    }

    #[test]
    fn test_min_max() {
        let mut compiler = Compiler::new();

        // Test min(10.0, 5.0) = 5.0
        let const_10 = compiler.add_constant(10.0);
        let const_5 = compiler.add_constant(5.0);
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::Min);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 5.0);
    }

    #[test]
    fn test_clamp() {
        let mut compiler = Compiler::new();

        // Test clamp(15.0, 0.0, 10.0) = 10.0
        let const_15 = compiler.add_constant(15.0);
        let const_0 = compiler.add_constant(0.0);
        let const_10 = compiler.add_constant(10.0);
        compiler.emit(Op::PushConst(const_15));
        compiler.emit(Op::PushConst(const_0));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Clamp);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 10.0);
    }

    #[test]
    fn test_complex_expression() {
        let mut compiler = Compiler::new();

        // Test: sqrt(abs(-16.0)) + 2.0 = 6.0
        let const_neg16 = compiler.add_constant(-16.0);
        let const_2 = compiler.add_constant(2.0);

        compiler.emit(Op::PushConst(const_neg16));
        compiler.emit(Op::Abs); // 16.0
        compiler.emit(Op::Sqrt); // 4.0
        compiler.emit(Op::PushConst(const_2));
        compiler.emit(Op::Add); // 6.0

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 6.0);
    }

    #[test]
    fn test_comparison_operators() {
        let mut compiler = Compiler::new();

        // Test: 5.0 < 3.0 => false
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Lt);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_bool(), false);

        // Test: 3.0 <= 5.0 => true
        let mut compiler = Compiler::new();
        let const_3 = compiler.add_constant(3.0);
        let const_5 = compiler.add_constant(5.0);

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::Le);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_bool(), true);

        // Test: 5.0 == 5.0 => true
        let mut compiler = Compiler::new();
        let const_5a = compiler.add_constant(5.0);
        let const_5b = compiler.add_constant(5.0);

        compiler.emit(Op::PushConst(const_5a));
        compiler.emit(Op::PushConst(const_5b));
        compiler.emit(Op::Eq);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_bool(), true);
    }

    #[test]
    fn test_where_conditional() {
        let mut compiler = Compiler::new();

        // Test: where(5.0 > 3.0, 100.0, 200.0) => 100.0 (true branch)
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_100 = compiler.add_constant(100.0);
        let const_200 = compiler.add_constant(200.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true
        compiler.emit(Op::PushConst(const_100)); // true value
        compiler.emit(Op::PushConst(const_200)); // false value
        compiler.emit(Op::Where);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 100.0);

        // Test: where(2.0 > 5.0, 100.0, 200.0) => 200.0 (false branch)
        let mut compiler = Compiler::new();
        let const_2 = compiler.add_constant(2.0);
        let const_5 = compiler.add_constant(5.0);
        let const_100 = compiler.add_constant(100.0);
        let const_200 = compiler.add_constant(200.0);

        compiler.emit(Op::PushConst(const_2));
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::Gt); // 2.0 > 5.0 => false
        compiler.emit(Op::PushConst(const_100)); // true value
        compiler.emit(Op::PushConst(const_200)); // false value
        compiler.emit(Op::Where);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_float(), 200.0);
    }

    #[test]
    fn test_logical_operators() {
        // Test: true && false => false
        let mut compiler = Compiler::new();
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false

        compiler.emit(Op::And); // true && false => false

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_bool(), false);

        // Test: true || false => true
        let mut compiler = Compiler::new();
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false

        compiler.emit(Op::Or); // true || false => true

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_bool(), true);

        // Test: !false => true
        let mut compiler = Compiler::new();
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false
        compiler.emit(Op::Not); // !false => true

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_bool(), true);

        // Test complex: (true && false) || !false => true
        let mut compiler = Compiler::new();
        let const_5 = compiler.add_constant(5.0);
        let const_3 = compiler.add_constant(3.0);
        let const_10 = compiler.add_constant(10.0);

        // Left side: true && false
        compiler.emit(Op::PushConst(const_5));
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::Gt); // 5.0 > 3.0 => true

        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false

        compiler.emit(Op::And); // true && false => false

        // Right side: !false
        compiler.emit(Op::PushConst(const_3));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Gt); // 3.0 > 10.0 => false
        compiler.emit(Op::Not); // !false => true

        compiler.emit(Op::Or); // false || true => true

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[], 0);
        }

        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0].as_bool(), true);
    }

    #[test]
    fn test_pooled_vm_acquire_and_release() {
        // Acquire a VM from pool
        let mut pooled = PooledVM::acquire();
        let vm = pooled.get_mut();

        // VM should be valid with pre-allocated stack
        assert_eq!(vm.stack.capacity(), 32);

        // Drop should return to pool (no panic)
        drop(pooled);

        // Acquire again - should get a recycled VM
        let pooled2 = PooledVM::acquire();
        drop(pooled2);
    }

    #[test]
    fn test_pooled_vm_reset() {
        let mut pooled = PooledVM::acquire();
        let vm = pooled.get_mut();

        // Add some state
        vm.stack.push(StackValue::Float(42.0));
        vm.entity_index = 999;

        // Reset should clear state
        vm.reset();

        assert!(vm.stack.is_empty());
        assert_eq!(vm.entity_index, 0);

        // But stack capacity should be preserved
        assert_eq!(vm.stack.capacity(), 32);
    }

    #[test]
    fn test_pooled_vm_executes_correctly() {
        // Create simple bytecode: value = value + 10
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();

        // Test with pooled VM
        let mut value = 5.0_f32;
        let ptr = &mut value as *mut f32 as *mut u8;

        {
            let mut pooled = PooledVM::acquire();
            let vm = pooled.get_mut();
            vm.reset();
            unsafe {
                vm.execute(&bytecode, &[ptr], 0);
            }
        }

        assert_eq!(value, 15.0);

        // Execute again with same pooled VM pattern
        {
            let mut pooled = PooledVM::acquire();
            let vm = pooled.get_mut();
            vm.reset();
            unsafe {
                vm.execute(&bytecode, &[ptr], 0);
            }
        }

        assert_eq!(value, 25.0);
    }

    #[test]
    fn test_pooled_vm_multiple_entities() {
        // Simulate processing multiple entities with pooled VM
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_1 = compiler.add_constant(1.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_1));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();

        // Process 100 entities
        let mut values: Vec<f32> = (0..100).map(|i| i as f32).collect();

        for (i, value) in values.iter_mut().enumerate() {
            let mut pooled = PooledVM::acquire();
            let vm = pooled.get_mut();
            vm.reset();
            let ptr = value as *mut f32 as *mut u8;
            unsafe {
                vm.execute(&bytecode, &[ptr], i);
            }
        }

        // Each value should be incremented by 1
        for (i, value) in values.iter().enumerate() {
            assert_eq!(*value, (i + 1) as f32);
        }
    }

    #[test]
    fn test_pool_bounded_size() {
        // Acquire many VMs to test pool size bounding
        let mut vms: Vec<PooledVM> = Vec::new();

        // Acquire 20 VMs (more than pool limit of 16)
        for _ in 0..20 {
            vms.push(PooledVM::acquire());
        }

        // Drop all - only 16 should go back to pool
        drop(vms);

        // Verify pool works after this
        let pooled = PooledVM::acquire();
        assert!(pooled.has_vm());
    }

    #[test]
    fn test_f64_field_store() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 5.0_f64;
        let field_ptr = &mut field_value as *mut f64 as *mut u8;

        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert_eq!(field_value, 15.0);
    }

    #[test]
    fn test_f64_precision_preserved() {
        // 0.1 + 0.2 in f64 has a specific representation that differs from f32
        // This value would lose precision if truncated to f32 and back
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);

        // Store a value that requires f64 precision: 1_000_000.123_456_789
        let precise_value: f64 = 1_000_000.123_456_789;
        let const_val = compiler.add_constant(precise_value);

        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 0.0_f64;
        let field_ptr = &mut field_value as *mut f64 as *mut u8;

        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        // f64 preserves this exactly; f32 would give 1000000.125
        assert_eq!(field_value, precise_value);
    }

    #[test]
    fn test_f32_roundtrip_lossless() {
        // f32 → f64 (load) → f64 arithmetic → f32 (store) should be lossless
        // for values that are exactly representable in f32
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(0.0);

        // field = field + 0.0 (identity operation to force roundtrip)
        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Test with various f32 values
        let test_values: Vec<f32> = vec![
            0.0,
            1.0,
            -1.0,
            0.5,
            f32::MAX,
            f32::MIN,
            f32::MIN_POSITIVE,
            std::f32::consts::PI,
            123.456,
        ];

        for &original in &test_values {
            let mut field_value = original;
            let field_ptr = &mut field_value as *mut f32 as *mut u8;

            unsafe {
                vm.reset();
                vm.execute(&bytecode, &[field_ptr], 0);
            }

            assert_eq!(
                field_value, original,
                "f32 roundtrip failed for {}",
                original
            );
        }
    }

    #[test]
    fn test_i64_field_load_store() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::I64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_10 = compiler.add_constant(10.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_10));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 42_i64;
        let field_ptr = &mut field_value as *mut i64 as *mut u8;

        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert_eq!(field_value, 52);
    }

    #[test]
    fn test_bool_field_load_store() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::Bool,
        };
        let field_idx = compiler.add_field(field_id);

        // Load bool, push to stack — true should become 1.0
        compiler.emit(Op::PushField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = true;
        let field_ptr = &mut field_value as *mut bool as *mut u8;

        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert_eq!(vm.stack[0].as_float(), 1.0);

        // Now test store: value >= 0.5 → true, < 0.5 → false
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::Bool,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(0.3); // < 0.5, should store false

        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = true;
        let field_ptr = &mut field_value as *mut bool as *mut u8;

        unsafe {
            vm.execute(&bytecode, &[field_ptr], 0);
        }

        assert!(!field_value);
    }

    #[test]
    fn test_execute_batch_f64_add_const() {
        // Tests the f64 fast path: field += const
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(100.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Contiguous f64 values (stride = 8 bytes)
        let count = 10;
        let mut values: Vec<f64> = (0..count).map(|i| i as f64).collect();
        let base_ptr = values.as_mut_ptr() as *mut u8;

        unsafe {
            vm.execute_batch(&bytecode, base_ptr, std::mem::size_of::<f64>(), count);
        }

        for (i, value) in values.iter().enumerate() {
            assert_eq!(
                *value,
                i as f64 + 100.0,
                "batch f64 add failed at index {}",
                i
            );
        }
    }

    #[test]
    fn test_execute_batch_f64_mul_const() {
        // Tests the f64 fast path: field *= const
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(2.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Mul);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let count = 10;
        let mut values: Vec<f64> = (1..=count).map(|i| i as f64).collect();
        let base_ptr = values.as_mut_ptr() as *mut u8;

        unsafe {
            vm.execute_batch(&bytecode, base_ptr, std::mem::size_of::<f64>(), count);
        }

        for (i, value) in values.iter().enumerate() {
            assert_eq!(
                *value,
                (i + 1) as f64 * 2.0,
                "batch f64 mul failed at index {}",
                i
            );
        }
    }

    #[test]
    fn test_execute_batch_f64_set_const() {
        // Tests the f64 fast path: field = const
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(42.5);

        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let count = 10;
        let mut values: Vec<f64> = vec![0.0; count];
        let base_ptr = values.as_mut_ptr() as *mut u8;

        unsafe {
            vm.execute_batch(&bytecode, base_ptr, std::mem::size_of::<f64>(), count);
        }

        for value in &values {
            assert_eq!(*value, 42.5);
        }
    }

    #[test]
    fn test_execute_batch_f32_preserves_precision() {
        // Verify f32 batch fast path still works correctly
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F32,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(1.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        // Use > 4 entities to test the unrolled loop
        let count = 10;
        let mut values: Vec<f32> = (0..count).map(|i| i as f32).collect();
        let base_ptr = values.as_mut_ptr() as *mut u8;

        unsafe {
            vm.execute_batch(&bytecode, base_ptr, std::mem::size_of::<f32>(), count);
        }

        for (i, value) in values.iter().enumerate() {
            assert_eq!(
                *value,
                (i + 1) as f32,
                "batch f32 add failed at index {}",
                i
            );
        }
    }

    #[test]
    fn test_execute_and_reduce_f64() {
        let mut compiler = Compiler::new();

        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_val = compiler.add_constant(3.0);

        // Expression: field * 3.0
        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_val));
        compiler.emit(Op::Mul);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        let mut field_value = 7.0_f64;
        let field_ptr = &mut field_value as *mut f64 as *mut u8;

        let result = unsafe { vm.execute_and_reduce(&bytecode, &[field_ptr], 0) };

        assert_eq!(result, 21.0);
        // Original value should be unchanged (reduce doesn't store)
        assert_eq!(field_value, 7.0);
    }

    /// Regression test: read/write f64 fields at 4-byte-aligned (but not
    /// 8-byte-aligned) addresses must not panic.
    #[test]
    fn test_unaligned_f64_field_access() {
        // Allocate a buffer with deliberate 4-byte misalignment for f64.
        // Layout: [4-byte padding] [f64 value] [f64 value]
        let mut buf = vec![0u8; 32];
        // Find a 4-byte-aligned-but-not-8-byte-aligned address within buf
        let base = buf.as_mut_ptr() as usize;
        let offset = if base % 8 == 0 { 4 } else { 0 };
        let misaligned_ptr = unsafe { buf.as_mut_ptr().add(offset) };
        assert_eq!(
            misaligned_ptr as usize % 8,
            4,
            "test setup: pointer should be 4-byte aligned but NOT 8-byte aligned"
        );

        // Write a known f64 value at the misaligned address
        unsafe {
            write_field_value(misaligned_ptr, 123.456, FieldType::F64);
        }

        // Read it back — must not panic
        let read_back = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::F64) };
        assert_eq!(read_back, 123.456);

        // Now test via the VM: compile `field = field + 1.0`
        let mut compiler = Compiler::new();
        let field_id = FieldId {
            component_id: ComponentId::new(0),
            offset: 0,
            field_type: FieldType::F64,
        };
        let field_idx = compiler.add_field(field_id);
        let const_1 = compiler.add_constant(1.0);

        compiler.emit(Op::PushField(field_idx));
        compiler.emit(Op::PushConst(const_1));
        compiler.emit(Op::Add);
        compiler.emit(Op::StoreField(field_idx));

        let bytecode = compiler.finalize();
        let mut vm = VM::new();

        unsafe {
            vm.execute(&bytecode, &[misaligned_ptr], 0);
        }

        let result = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::F64) };
        assert_eq!(result, 124.456);
    }

    #[test]
    fn test_unaligned_i64_u64_field_access() {
        let mut buf = vec![0u8; 32];
        let base = buf.as_mut_ptr() as usize;
        let offset = if base % 8 == 0 { 4 } else { 0 };
        let misaligned_ptr = unsafe { buf.as_mut_ptr().add(offset) };
        assert_eq!(misaligned_ptr as usize % 8, 4);

        // i64
        unsafe {
            write_field_value(misaligned_ptr, 42.0, FieldType::I64);
        }
        let v = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::I64) };
        assert_eq!(v, 42.0);

        // u64
        unsafe {
            write_field_value(misaligned_ptr, 99.0, FieldType::U64);
        }
        let v = unsafe { read_field_value(misaligned_ptr as *const u8, FieldType::U64) };
        assert_eq!(v, 99.0);
    }

    /// Regression test: the f32 batch fast paths (add/mul/assign) must handle
    /// misaligned field addresses, since ECS column bytes have no alignment
    /// guarantee for embedded fields.
    #[test]
    fn test_unaligned_f32_batch_fast_paths() {
        let compile = |ops: &[Op], constant: f64| {
            let mut compiler = Compiler::new();
            let field_id = FieldId {
                component_id: ComponentId::new(0),
                offset: 0,
                field_type: FieldType::F32,
            };
            let field_idx = compiler.add_field(field_id);
            let const_val = compiler.add_constant(constant);
            for op in ops {
                compiler.emit(match op {
                    Op::PushField(_) => Op::PushField(field_idx),
                    Op::PushConst(_) => Op::PushConst(const_val),
                    Op::StoreField(_) => Op::StoreField(field_idx),
                    other => other.clone(),
                });
            }
            compiler.finalize()
        };

        // Odd stride keeps every entity's f32 misaligned; count > 4 exercises
        // the unrolled add loop.
        let count = 10;
        let stride = 5;
        let mut buf = vec![0u8; count * stride + 1];
        let base_ptr = unsafe { buf.as_mut_ptr().add(1) };
        assert_ne!(base_ptr as usize % 4, 0, "base pointer must be misaligned");

        let read_all = |buf: &[u8]| -> Vec<f32> {
            (0..count)
                .map(|i| unsafe {
                    (buf.as_ptr().add(1 + i * stride) as *const f32).read_unaligned()
                })
                .collect()
        };

        let mut vm = VM::new();

        // field = const
        let assign = compile(&[Op::PushConst(0), Op::StoreField(0)], 2.0);
        unsafe {
            vm.execute_batch(&assign, base_ptr, stride, count);
        }
        assert!(read_all(&buf).iter().all(|v| *v == 2.0));

        // field = field + const (unrolled loop + remainder)
        let add = compile(
            &[
                Op::PushField(0),
                Op::PushConst(0),
                Op::Add,
                Op::StoreField(0),
            ],
            1.5,
        );
        unsafe {
            vm.execute_batch(&add, base_ptr, stride, count);
        }
        assert!(read_all(&buf).iter().all(|v| *v == 3.5));

        // field = field * const
        let mul = compile(
            &[
                Op::PushField(0),
                Op::PushConst(0),
                Op::Mul,
                Op::StoreField(0),
            ],
            2.0,
        );
        unsafe {
            vm.execute_batch(&mul, base_ptr, stride, count);
        }
        assert!(read_all(&buf).iter().all(|v| *v == 7.0));
    }

    /// Eq uses exact comparison, not epsilon. Nearly-equal values must
    /// compare as not-equal (regression: old batch modes used epsilon).
    #[test]
    fn test_eq_exact_not_epsilon() {
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(1.0);
        let b = compiler.add_constant(1.0 + f64::EPSILON);

        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Eq);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert_eq!(vm.stack[0].as_bool(), false, "1.0 != 1.0+EPSILON (exact)");
    }

    #[test]
    fn test_ne_operator() {
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(3.0);
        let b = compiler.add_constant(4.0);

        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Ne);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert_eq!(vm.stack[0].as_bool(), true, "3.0 != 4.0");

        // Same values should be not Ne
        let mut compiler = Compiler::new();
        let a = compiler.add_constant(5.0);
        let b = compiler.add_constant(5.0);

        compiler.emit(Op::PushConst(a));
        compiler.emit(Op::PushConst(b));
        compiler.emit(Op::Ne);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert_eq!(vm.stack[0].as_bool(), false, "5.0 == 5.0 so Ne is false");
    }

    /// Sign returns 0.0 for NaN (not NaN like f64::signum would).
    #[test]
    fn test_sign_nan_returns_zero() {
        let mut compiler = Compiler::new();
        let nan = compiler.add_constant(f64::NAN);

        compiler.emit(Op::PushConst(nan));
        compiler.emit(Op::Sign);

        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };

        assert_eq!(vm.stack[0].as_float(), 0.0, "sign(NaN) should be 0.0");
    }

    #[test]
    fn test_sign_positive_negative_zero() {
        // Positive
        let mut compiler = Compiler::new();
        let c = compiler.add_constant(42.0);
        compiler.emit(Op::PushConst(c));
        compiler.emit(Op::Sign);
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert_eq!(vm.stack[0].as_float(), 1.0);

        // Negative
        let mut compiler = Compiler::new();
        let c = compiler.add_constant(-7.5);
        compiler.emit(Op::PushConst(c));
        compiler.emit(Op::Sign);
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert_eq!(vm.stack[0].as_float(), -1.0);

        // Zero
        let mut compiler = Compiler::new();
        let c = compiler.add_constant(0.0);
        compiler.emit(Op::PushConst(c));
        compiler.emit(Op::Sign);
        let bytecode = compiler.finalize();
        let mut vm = VM::new();
        unsafe { vm.execute(&bytecode, &[], 0) };
        assert_eq!(vm.stack[0].as_float(), 0.0);
    }
}
