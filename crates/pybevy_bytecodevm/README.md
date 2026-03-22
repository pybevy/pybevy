# pybevy_bytecodevm

A high-performance bytecode virtual machine for executing lazy mathematical expressions on ECS component fields in batch operations.

## Overview

`pybevy_bytecodevm` is a stack-based bytecode compiler and interpreter designed for minimal overhead execution of mathematical expressions across large numbers of entities in the Bevy ECS. It powers PyBevy's View API, enabling 30-50x performance improvements over traditional Python Query iteration.

## Features

- **Stack-based VM**: Simple, cache-friendly bytecode execution (~2-5ns per operation)
- **Expression AST**: Parse Python expressions into an optimizable intermediate representation
- **Bytecode Compilation**: Convert expression trees to linear bytecode with constant folding
- **Type Support**: Multiple field types (f32, f64, i32, i64, u32, u64, bool) with f64 internal precision
- **Rich Operations**: Arithmetic, trigonometric, comparison, logical, and random operations
- **Deterministic Random**: Per-entity seeded randomness for reproducible simulations
- **Reduction Mode**: Evaluate expressions without storing results (for filtering/aggregation)

## Architecture

```
Python Expression → RustExpr AST → Compiler → CompiledBytecode → VM Execution
```

### Components

1. **`RustExpr`** (`expr.rs`): Abstract Syntax Tree representing mathematical expressions
2. **`Compiler`** (`bytecode.rs`): Converts AST to optimized bytecode with constant pool
3. **`VM`** (`bytecode.rs`): Executes bytecode on entity component data
4. **`CompiledBytecode`** (`bytecode.rs`): Compiled bytecode ready for execution

## Performance

The bytecode VM is optimized for:

- **Cache locality**: Linear bytecode array (no tree traversal)
- **Minimal overhead**: Simple stack operations with no recursion
- **Parallel execution**: Read-only bytecode perfect for `par_iter_mut`
- **Pre-resolved offsets**: Component field locations computed at compile time

## Example Usage

### Compiling an Expression

```rust
use pybevy_bytecodevm::{Compiler, RustExpr, Op};

// Expression: pos.x = pos.x + vel.x * dt
let mut compiler = Compiler::new();

// Build expression AST
let expr = RustExpr::Add(
    Box::new(RustExpr::Field { /* pos.x */ }),
    Box::new(RustExpr::Mul(
        Box::new(RustExpr::Field { /* vel.x */ }),
        Box::new(RustExpr::Const(dt))
    ))
);

// Compile to bytecode
expr.compile(&mut compiler);
compiler.optimize(); // Apply peephole optimizations
let bytecode = compiler.finalize();
```

### Executing Bytecode

```rust
use pybevy_bytecodevm::VM;

let mut vm = VM::new();

// field_ptrs: one pointer per field registered with the compiler,
// pointing to the field's memory for this entity.
// entity_index: the entity's index, used for deterministic random seeding.
let mut pos_x: f32 = 100.0;
let mut vel_x: f32 = 5.0;
let field_ptrs: &[*mut u8] = &[
    &mut pos_x as *mut f32 as *mut u8,  // field 0: pos.x
    &mut vel_x as *mut f32 as *mut u8,  // field 1: vel.x
];
let entity_index: usize = 42;

unsafe {
    vm.execute(&bytecode, field_ptrs, entity_index);
}
// pos_x is now updated in-place
```

### Reduction Mode

```rust
// Evaluate an expression and return the result without storing.
// Useful for filtering or aggregation.
let mut health: f32 = 30.0;
let field_ptrs: &[*mut u8] = &[
    &mut health as *mut f32 as *mut u8,  // field 0: health
];
let entity_index: usize = 7;

let result = unsafe {
    vm.execute_and_reduce(&bytecode, field_ptrs, entity_index)
};
// result contains the final stack value (e.g. a boolean comparison result as 0.0/1.0)
```

## Bytecode Example

Expression: `pos.x = pos.x + vel.x * dt`

Bytecode:
```
PushField(0)      // Push pos.x
PushField(1)      // Push vel.x
PushConst(0)      // Push dt from constant pool
Mul               // vel.x * dt
Add               // pos.x + (vel.x * dt)
StoreField(0)     // Store to pos.x
```

## Supported Operations

### Arithmetic
- Add, Sub, Mul, Div, Pow, Neg, Mod

### Trigonometric
- Sin, Cos, Tan, Asin, Acos, Atan

### Numeric
- Sqrt, Abs, Floor, Ceil, Round, Min, Max, Clamp
- Exp, Ln, Log10, Log2, Sign, Fract, Lerp

### Comparison
- Eq, Ne, Lt, Le, Gt, Ge

### Logical
- And, Or, Not

### Conditional
- Where (ternary operator: `where(condition, true_value, false_value)`)

### Random
- Random (deterministic per-entity [0.0, 1.0))
- RandomRange (deterministic per-entity [min, max))

## Optimizations

The compiler applies peephole optimizations including:

- **Constant folding**: `5.0 + 3.0` → `8.0` at compile time
- **Constant deduplication**: Shared constant pool with bit-exact matching

## Integration with PyBevy

This crate is used internally by PyBevy's View API:

```python
# Python code
view = View[tuple[Mut[Transform], Velocity]]
pos = view.column_mut(Transform)
vel = view.column(Velocity)

# Compiles to bytecode and executes via VM
pos.translation.x = pos.translation.x + vel.x * dt
```

## Safety

The VM uses `unsafe` for direct memory access to component fields. Callers must ensure:

- All field pointers are valid and properly aligned
- Field pointers remain valid for the duration of execution
- No concurrent mutations to the same memory

These safety requirements are guaranteed by Bevy's ECS query system.

## Testing

Run the test suite:

```bash
cargo test -p pybevy_bytecodevm
```

The crate includes comprehensive tests for:
- Arithmetic operations
- Trigonometric functions
- Comparison operators
- Logical operators
- Conditional selection
- Constant folding optimization
- Field storage and retrieval
