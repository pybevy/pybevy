# pybevy_bytecodevm

The expression engine behind PyBevy's lazy `View` expressions and `pybevy.array`
element-wise kernels. It compiles expression trees to a small stack bytecode and
runs that bytecode over ECS table columns, flat buffers, or dense arrays.

## Modules

| Module | Role |
|---|---|
| `expr` | `RustExpr`, the expression tree, and its conversion from Python `Expr` objects |
| `bytecode` | `Compiler`, `CompiledBytecode`, the `Op` set, and the per-row stack `VM` |
| `tiled` | Tile-at-a-time executors for float and homogeneous integer programs |
| `columns` | Typed column capabilities that feed the tiled executors |
| `dense` | Validated programs over dense `f32`/`f64` input slices |
| `buffer_lens` | Validated in-place assignments over strided buffers |
| `view_engine` | Table-row gathering and batch execution for View filters |
| `view_runtime` | The validated View capability layer used by the backends |

Nothing is re-exported at the crate root; import from these modules.

## Types and semantics

Fields may be `f32`, `f64`, `i32`, `i64`, `u8`, `u32`, `u64`, or `bool`.
Composite `Vec2`/`Vec3`/`Vec4` fields are expanded to `f32` lanes before
compilation. Integer values stay exact in their own width and wrap on overflow;
floating-point fields evaluate in f64, or in f32 when a dense program's inputs
are all `f32`. Literals follow NumPy 2's rules for Python scalars: an `int`
literal adopts the other operand's type and a `float` literal promotes an
integer operand to f64. `%`, `//`, `round`, `sign`, `min`, `max`, and `clamp`
follow NumPy's element-wise conventions.

Random draws are deterministic per row. The View engine seeds each row from its
`Entity`, so a value does not depend on the execution path or on the table row
the entity occupies.

## Compiling and running

```rust
use pybevy_bytecodevm::{
    bytecode::{Compiler, FieldId, FieldType, Op, VM},
    expr::RustExpr,
};

// x = x + v * 0.016 over a component with two f32 fields.
let component_id = bevy_ecs::component::ComponentId::new(0);
let field = |offset| RustExpr::Field {
    component_id,
    offset,
    field_type: FieldType::F32,
};
let expression = RustExpr::Add(
    Box::new(field(0)),
    Box::new(RustExpr::Mul(
        Box::new(field(4)),
        Box::new(RustExpr::FloatConst(0.016)),
    )),
);

let mut compiler = Compiler::new();
expression.compile(&mut compiler);
let x = compiler.add_field(FieldId {
    component_id,
    offset: 0,
    field_type: FieldType::F32,
});
compiler.emit(Op::StoreField(x));
compiler.optimize();
let bytecode = compiler.finalize();

let mut row = [1.0_f32, 2.0];
let base = row.as_mut_ptr().cast::<u8>();
let pointers = [base, unsafe { base.add(4) }];
let mut vm = VM::new();
// SAFETY: one pointer per field, each valid for its field type.
unsafe { vm.execute(&bytecode, &pointers, 0) };
assert_eq!(row[0], 1.032);
```

`VM::execute_and_reduce` evaluates a store-free program and returns its value.
`VM::execute_batch_multi` and the `tiled` executors run the same bytecode over
strided runs of rows.

## Safety

The execution functions take raw pointers and are `unsafe`: each field pointer
must be valid for its `FieldType` for the whole call, and no other code may
access the stored memory concurrently. PyBevy's backends do not call them
directly. They go through `view_runtime`, which validates every program against
the resolved View (declared components, real field offsets and types, write
effects, stack shape, and integer literal ranges) before any pointer is formed,
and checks run-scoped validity before each access.

## Benchmarks and tests

```bash
cargo bench -p pybevy_bytecodevm --bench view_assignment
cargo test -p pybevy_bytecodevm --features pyo3/auto-initialize --lib
```

The unit tests start an embedded Python interpreter, hence the
`pyo3/auto-initialize` feature.
