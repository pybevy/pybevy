"""
Numba extension for ViewColumn opaque handles.

This module teaches Numba how to JIT-compile functions that operate on
ViewColumn handles with zero-copy access to Bevy ECS component storage.

Safety: The validity token is checked at the Numba call boundary (in unbox()),
preventing segfaults from use-after-free bugs.
"""

import operator

try:
    import numba  # type: ignore[import-untyped]
    from numba import types  # type: ignore[import-untyped]
    from numba.core import cgutils  # type: ignore[import-untyped]
    from numba.core.imputils import lower_builtin  # type: ignore[import-untyped]
    from numba.extending import (  # type: ignore[import-untyped]  # type: ignore[import-untyped]
        intrinsic,
        make_attribute_wrapper,
        models,
        overload,
        register_model,
        typeof_impl,
    )
except ImportError:
    raise ImportError(
        "Numba is required for ViewColumn zero-copy access.\n"
        "Install with: pip install numba"
    )

# Import the Rust type and Python wrappers
from . import ViewColumn
from .view_accessors import QuatViewColumn, Vec3ViewColumn

# ============================================================================
# Step 1: Define the Numba type
# ============================================================================


class ViewColumnType(types.Type):
    """
    Numba type for ViewColumn opaque handles.

    Dtype-parameterized for compile-time specialization:
    - ViewColumnType('f4') for float32 columns
    - ViewColumnType('f8') for float64 columns
    - ViewColumnType('i4') for int32 columns
    - ViewColumnType('i8') for int64 columns

    This eliminates runtime dtype branching in getitem/setitem.
    """

    def __init__(self, dtype: str = 'f4') -> None:
        self.dtype = dtype
        self.name = f"ViewColumn[{dtype}]"
        super(ViewColumnType, self).__init__(name=self.name)

    @property
    def key(self) -> tuple[str, str]:
        """Unique key for type caching - includes dtype."""
        return (self.name, self.dtype)


# Default types for common cases (cached singletons)
view_column_type_f32 = ViewColumnType('f4')
view_column_type_f64 = ViewColumnType('f8')
view_column_type_i32 = ViewColumnType('i4')
view_column_type_i64 = ViewColumnType('i8')
# Backwards compatibility alias
view_column_type = view_column_type_f32

# Dtype code constants (must match unbox logic)
_DTYPE_F32 = 0
_DTYPE_F64 = 1
_DTYPE_I32 = 2
_DTYPE_I64 = 3


# Tell Numba what type to use for ViewColumn objects
@typeof_impl.register(ViewColumn)
def typeof_view_column(val, c):
    """
    Infer the Numba type for ViewColumn objects.

    Returns dtype-specialized type for compile-time optimization.
    """
    match val.dtype:
        case 'f8':
            return view_column_type_f64
        case 'i4':
            return view_column_type_i32
        case 'i8':
            return view_column_type_i64
        case _:
            return view_column_type_f32


# ============================================================================
# Step 2: Define the native representation (C struct layout)
# ============================================================================


@register_model(ViewColumnType)
class ViewColumnModel(models.StructModel):
    """
    Native representation: struct { ptr, len, stride, dtype_code }

    This is the C struct that exists during JIT compilation.
    The Python ViewColumn object is "unboxed" into this struct.

    dtype_code: 0=f32, 1=f64, 2=i32, 3=i64
    """

    def __init__(self, dmm, fe_type) -> None:
        members = [
            ("ptr", types.uintp),  # Raw pointer
            ("len", types.intp),  # Length
            ("stride", types.intp),  # Stride in bytes
            ("is_f64", types.int8),  # Kept for backwards compat; now holds dtype_code
        ]
        models.StructModel.__init__(self, dmm, fe_type, members)


# Make fields accessible in JIT code (for advanced users)
make_attribute_wrapper(ViewColumnType, "ptr", "ptr")
make_attribute_wrapper(ViewColumnType, "len", "len")
make_attribute_wrapper(ViewColumnType, "stride", "stride")
make_attribute_wrapper(ViewColumnType, "is_f64", "is_f64")


# ============================================================================
# Step 3: Unboxing (Python → Native) - THE SAFETY CHECK!
# ============================================================================


@numba.extending.unbox(ViewColumnType)
def unbox_view_column(typ, obj, c):
    """
    Convert Python ViewColumn object to native struct.

    THIS IS WHERE THE SAFETY CHECK HAPPENS!

    If the validity token is poisoned (system ended), this raises
    a RuntimeError instead of causing a segfault.
    """
    # Create native struct
    view_val = cgutils.create_struct_proxy(typ)(c.context, c.builder)

    # Extract is_valid property
    is_valid_obj = c.pyapi.object_getattr_string(obj, "is_valid")
    is_valid_int = c.pyapi.object_istrue(is_valid_obj)

    # Convert i32 result from object_istrue to i1 boolean for if_else
    is_valid = c.builder.icmp_signed('!=', is_valid_int, is_valid_int.type(0))

    # THE POISON PILL CHECK
    with c.builder.if_else(is_valid) as (valid, invalid):
        with valid:
            # Extract fields
            ptr_obj = c.pyapi.object_getattr_string(obj, "ptr")
            len_obj = c.pyapi.object_getattr_string(obj, "len")
            stride_obj = c.pyapi.object_getattr_string(obj, "stride")
            dtype_obj = c.pyapi.object_getattr_string(obj, "dtype")

            # Convert to native values
            view_val.ptr = c.pyapi.number_as_ssize_t(ptr_obj)
            view_val.len = c.pyapi.number_as_ssize_t(len_obj)
            view_val.stride = c.pyapi.number_as_ssize_t(stride_obj)

            # Determine dtype_code: 0=f32, 1=f64, 2=i32, 3=i64
            # Check dtype strings in order
            f8_str = c.pyapi.unserialize(c.pyapi.serialize_object("f8"))
            i4_str = c.pyapi.unserialize(c.pyapi.serialize_object("i4"))
            i8_str = c.pyapi.unserialize(c.pyapi.serialize_object("i8"))

            is_f8 = c.pyapi.object_richcompare(dtype_obj, f8_str, "==")
            is_f8_int = c.pyapi.object_istrue(is_f8)
            is_f8_bool = c.builder.icmp_signed('!=', is_f8_int, is_f8_int.type(0))

            is_i4 = c.pyapi.object_richcompare(dtype_obj, i4_str, "==")
            is_i4_int = c.pyapi.object_istrue(is_i4)
            is_i4_bool = c.builder.icmp_signed('!=', is_i4_int, is_i4_int.type(0))

            is_i8 = c.pyapi.object_richcompare(dtype_obj, i8_str, "==")
            is_i8_int = c.pyapi.object_istrue(is_i8)
            is_i8_bool = c.builder.icmp_signed('!=', is_i8_int, is_i8_int.type(0))

            # Build dtype_code: f8->1, i4->2, i8->3, else->0
            i8_type = c.context.get_value_type(types.int8)
            code = i8_type(0)  # default f32
            code = c.builder.select(is_f8_bool, i8_type(1), code)
            code = c.builder.select(is_i4_bool, i8_type(2), code)
            code = c.builder.select(is_i8_bool, i8_type(3), code)
            view_val.is_f64 = code

        with invalid:
            # Raise exception
            c.pyapi.err_set_string(
                "PyExc_RuntimeError",
                "CRITICAL: Accessing stale ViewColumn!\n"
                "This view is only valid within the system that created it.\n"
                "Do not store ViewColumn objects in global variables.",
            )

    # Check for Python errors and return NativeValue
    # The second argument indicates if there was an error during conversion
    err_ptr = c.pyapi.err_occurred()
    is_error = cgutils.is_not_null(c.builder, err_ptr)

    return numba.core.pythonapi.NativeValue(view_val._getvalue(), is_error=is_error)


# ============================================================================
# Step 4: Implement operations (getitem, setitem, len)
# ============================================================================


@overload(len)
def view_column_len(view_col):
    """
    Implement len(view) -> view.len

    This compiles to native code that reads the len field.
    """
    if isinstance(view_col, ViewColumnType):
        def len_impl(view_col):
            return view_col.len
        return len_impl


@intrinsic
def _view_column_getitem_impl(typingctx, view_col_t, idx_t):
    """
    Intrinsic implementation of ViewColumn[i] with compile-time dtype specialization.

    The dtype is checked at JIT compile time (not runtime), eliminating
    per-access branching overhead.
    """
    # Always return float64 to handle all column types uniformly
    sig = types.float64(view_col_t, idx_t)

    # Compile-time dtype check - determines which code path to generate
    dtype = view_col_t.dtype

    def codegen(context, builder, sig, args):
        view_val, idx_val = args
        view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

        # Calculate offset: i * stride
        offset = builder.mul(idx_val, view.stride)

        # Calculate address: ptr + offset
        addr = builder.add(view.ptr, offset)

        if dtype == 'f8':
            # f64 path: load 8 bytes as f64
            f64_ptr_t = context.get_value_type(types.float64).as_pointer()
            ptr64 = builder.inttoptr(addr, f64_ptr_t)
            return builder.load(ptr64)
        if dtype == 'i4':
            # i32 path: load 4 bytes as i32, convert to f64
            i32_ptr_t = context.get_value_type(types.int32).as_pointer()
            ptr32 = builder.inttoptr(addr, i32_ptr_t)
            val32 = builder.load(ptr32)
            return builder.sitofp(val32, context.get_value_type(types.float64))
        if dtype == 'i8':
            # i64 path: load 8 bytes as i64, convert to f64
            i64_ptr_t = context.get_value_type(types.int64).as_pointer()
            ptr64 = builder.inttoptr(addr, i64_ptr_t)
            val64 = builder.load(ptr64)
            return builder.sitofp(val64, context.get_value_type(types.float64))
        # f32 path: load 4 bytes as f32, extend to f64
        f32_ptr_t = context.get_value_type(types.float32).as_pointer()
        ptr32 = builder.inttoptr(addr, f32_ptr_t)
        val32 = builder.load(ptr32)
        return builder.fpext(val32, context.get_value_type(types.float64))

    return sig, codegen


@overload(operator.getitem)
def view_column_getitem_overload(view_col, idx):
    """High-level typing for view[i] - returns float64"""
    if isinstance(view_col, ViewColumnType) and isinstance(idx, types.Integer):
        def getitem_impl(view_col, idx):
            return _view_column_getitem_impl(view_col, idx)
        return getitem_impl


@intrinsic
def _view_column_setitem_impl(typingctx, view_col_t, idx_t, val_t):
    """
    Intrinsic implementation of ViewColumn[i] = value with compile-time dtype specialization.

    The dtype is checked at JIT compile time (not runtime), eliminating
    per-access branching overhead.
    """
    sig = types.void(view_col_t, idx_t, val_t)

    # Compile-time dtype check - determines which code path to generate
    dtype = view_col_t.dtype

    def codegen(context, builder, sig, args):
        view_val, idx_val, value_val = args
        view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

        # Calculate offset
        offset = builder.mul(idx_val, view.stride)
        addr = builder.add(view.ptr, offset)

        # Convert input to float64 first (handles int, float32, float64)
        if isinstance(sig.args[2], types.Integer):
            f64_val = builder.sitofp(value_val, context.get_value_type(types.float64))
        elif sig.args[2] == types.float32:
            f64_val = builder.fpext(value_val, context.get_value_type(types.float64))
        else:
            f64_val = value_val

        if dtype == 'f8':
            # f64 path: store 8 bytes as f64
            f64_ptr_t = context.get_value_type(types.float64).as_pointer()
            ptr64 = builder.inttoptr(addr, f64_ptr_t)
            store_inst = builder.store(f64_val, ptr64)
            store_inst.volatile = True
        elif dtype == 'i4':
            # i32 path: convert f64 to i32 and store 4 bytes
            i32_val = builder.fptosi(f64_val, context.get_value_type(types.int32))
            i32_ptr_t = context.get_value_type(types.int32).as_pointer()
            ptr32 = builder.inttoptr(addr, i32_ptr_t)
            store_inst = builder.store(i32_val, ptr32)
            store_inst.volatile = True
        elif dtype == 'i8':
            # i64 path: convert f64 to i64 and store 8 bytes
            i64_val = builder.fptosi(f64_val, context.get_value_type(types.int64))
            i64_ptr_t = context.get_value_type(types.int64).as_pointer()
            ptr64 = builder.inttoptr(addr, i64_ptr_t)
            store_inst = builder.store(i64_val, ptr64)
            store_inst.volatile = True
        else:
            # f32 path: truncate to f32 and store 4 bytes
            f32_val = builder.fptrunc(f64_val, context.get_value_type(types.float32))
            f32_ptr_t = context.get_value_type(types.float32).as_pointer()
            ptr32 = builder.inttoptr(addr, f32_ptr_t)
            store_inst = builder.store(f32_val, ptr32)
            store_inst.volatile = True

        return context.get_dummy_value()

    return sig, codegen


@overload(operator.setitem)
def view_column_setitem_overload(view_col, idx, val):
    """High-level typing for view[i] = value - uses intrinsic implementation"""
    if isinstance(view_col, ViewColumnType) and isinstance(idx, types.Integer):
        if isinstance(val, (types.Float, types.Integer)):
            def setitem_impl(view_col, idx, val) -> None:
                _view_column_setitem_impl(view_col, idx, val)
            return setitem_impl


# Lower-level implementations for direct LLVM codegen (f32 columns)
@lower_builtin("setitem", view_column_type_f32, types.Integer, types.Float)
def view_column_setitem_float(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = float_value (f32 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    float_ptr_t = context.get_value_type(types.float32).as_pointer()
    ptr = builder.inttoptr(addr, float_ptr_t)
    store_inst = builder.store(value_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


@lower_builtin("setitem", view_column_type_f32, types.Integer, types.Integer)
def view_column_setitem_int(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = int_value (f32 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    float_val = builder.sitofp(value_val, context.get_value_type(types.float32))
    float_ptr_t = context.get_value_type(types.float32).as_pointer()
    ptr = builder.inttoptr(addr, float_ptr_t)
    store_inst = builder.store(float_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


@lower_builtin("setitem", view_column_type_f32, types.Integer, types.float64)
def view_column_setitem_float64(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = float64_value (f32 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    float_val = builder.fptrunc(value_val, context.get_value_type(types.float32))
    float_ptr_t = context.get_value_type(types.float32).as_pointer()
    ptr = builder.inttoptr(addr, float_ptr_t)
    store_inst = builder.store(float_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


# Lower-level implementations for i32 columns
@lower_builtin("setitem", view_column_type_i32, types.Integer, types.Float)
def view_column_i32_setitem_float(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = float_value (i32 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    i32_val = builder.fptosi(value_val, context.get_value_type(types.int32))
    i32_ptr_t = context.get_value_type(types.int32).as_pointer()
    ptr = builder.inttoptr(addr, i32_ptr_t)
    store_inst = builder.store(i32_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


@lower_builtin("setitem", view_column_type_i32, types.Integer, types.Integer)
def view_column_i32_setitem_int(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = int_value (i32 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    # Truncate to i32 if needed
    i32_val = builder.trunc(value_val, context.get_value_type(types.int32))
    i32_ptr_t = context.get_value_type(types.int32).as_pointer()
    ptr = builder.inttoptr(addr, i32_ptr_t)
    store_inst = builder.store(i32_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


@lower_builtin("setitem", view_column_type_i32, types.Integer, types.float64)
def view_column_i32_setitem_float64(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = float64_value (i32 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    i32_val = builder.fptosi(value_val, context.get_value_type(types.int32))
    i32_ptr_t = context.get_value_type(types.int32).as_pointer()
    ptr = builder.inttoptr(addr, i32_ptr_t)
    store_inst = builder.store(i32_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


# Lower-level implementations for i64 columns
@lower_builtin("setitem", view_column_type_i64, types.Integer, types.Float)
def view_column_i64_setitem_float(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = float_value (i64 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    i64_val = builder.fptosi(value_val, context.get_value_type(types.int64))
    i64_ptr_t = context.get_value_type(types.int64).as_pointer()
    ptr = builder.inttoptr(addr, i64_ptr_t)
    store_inst = builder.store(i64_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


@lower_builtin("setitem", view_column_type_i64, types.Integer, types.Integer)
def view_column_i64_setitem_int(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = int_value (i64 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    # Sign-extend to i64 if needed
    i64_val = builder.sext(value_val, context.get_value_type(types.int64))
    i64_ptr_t = context.get_value_type(types.int64).as_pointer()
    ptr = builder.inttoptr(addr, i64_ptr_t)
    store_inst = builder.store(i64_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


@lower_builtin("setitem", view_column_type_i64, types.Integer, types.float64)
def view_column_i64_setitem_float64(context, builder, sig, args):
    """Direct LLVM implementation for view[i] = float64_value (i64 column)"""
    view_val, idx_val, value_val = args
    view = cgutils.create_struct_proxy(sig.args[0])(context, builder, value=view_val)

    offset = builder.mul(idx_val, view.stride)
    addr = builder.add(view.ptr, offset)

    i64_val = builder.fptosi(value_val, context.get_value_type(types.int64))
    i64_ptr_t = context.get_value_type(types.int64).as_pointer()
    ptr = builder.inttoptr(addr, i64_ptr_t)
    store_inst = builder.store(i64_val, ptr)
    store_inst.volatile = True

    return context.get_dummy_value()


# ============================================================================
# Step 5: Helper for range iteration
# ============================================================================


@overload(range)
def view_column_range(view):
    """
    Allow: for i in range(len(view))

    This enables natural iteration syntax in Numba kernels.
    """
    if isinstance(view, ViewColumnType):

        def impl(view):
            return range(len(view))

        return impl


# ============================================================================
# Vec3ViewColumn Numba Extension
# ============================================================================


class Vec3ViewColumnType(types.Type):
    """Numba type for Vec3ViewColumn composite wrappers."""

    def __init__(self) -> None:
        self.name = "Vec3ViewColumn"
        super(Vec3ViewColumnType, self).__init__(name=self.name)


vec3_view_column_type = Vec3ViewColumnType()


@typeof_impl.register(Vec3ViewColumn)
def typeof_vec3_view_column(val, c):
    """Infer the Numba type for Vec3ViewColumn objects."""
    return vec3_view_column_type


@register_model(Vec3ViewColumnType)
class Vec3ViewColumnModel(models.StructModel):
    """
    Native representation: struct { x, y, z }

    Each field is a ViewColumn (which itself is struct { ptr, len, stride }).
    """

    def __init__(self, dmm, fe_type) -> None:
        members = [
            ("x", view_column_type),
            ("y", view_column_type),
            ("z", view_column_type),
        ]
        models.StructModel.__init__(self, dmm, fe_type, members)


# Make fields accessible in JIT code
make_attribute_wrapper(Vec3ViewColumnType, "x", "x")
make_attribute_wrapper(Vec3ViewColumnType, "y", "y")
make_attribute_wrapper(Vec3ViewColumnType, "z", "z")


@numba.extending.unbox(Vec3ViewColumnType)
def unbox_vec3_view_column(typ, obj, c):
    """
    Convert Python Vec3ViewColumn object to native struct.

    Extracts .x, .y, .z properties and unboxes each as a ViewColumn.
    """
    # Create native struct
    vec3_val = cgutils.create_struct_proxy(typ)(c.context, c.builder)

    # Extract x, y, z properties
    x_obj = c.pyapi.object_getattr_string(obj, "x")
    y_obj = c.pyapi.object_getattr_string(obj, "y")
    z_obj = c.pyapi.object_getattr_string(obj, "z")

    # Unbox each ViewColumn
    x_native = c.unbox(view_column_type, x_obj)
    y_native = c.unbox(view_column_type, y_obj)
    z_native = c.unbox(view_column_type, z_obj)

    # Check for errors during unboxing
    err = cgutils.is_not_null(c.builder, c.pyapi.err_occurred())

    # Store in struct
    vec3_val.x = x_native.value
    vec3_val.y = y_native.value
    vec3_val.z = z_native.value

    return numba.core.pythonapi.NativeValue(vec3_val._getvalue(), is_error=err)


# ============================================================================
# QuatViewColumn Numba Extension
# ============================================================================


class QuatViewColumnType(types.Type):
    """Numba type for QuatViewColumn composite wrappers."""

    def __init__(self) -> None:
        self.name = "QuatViewColumn"
        super(QuatViewColumnType, self).__init__(name=self.name)


quat_view_column_type = QuatViewColumnType()


@typeof_impl.register(QuatViewColumn)
def typeof_quat_view_column(val, c):
    """Infer the Numba type for QuatViewColumn objects."""
    return quat_view_column_type


@register_model(QuatViewColumnType)
class QuatViewColumnModel(models.StructModel):
    """
    Native representation: struct { x, y, z, w }

    Each field is a ViewColumn (which itself is struct { ptr, len, stride }).
    """

    def __init__(self, dmm, fe_type) -> None:
        members = [
            ("x", view_column_type),
            ("y", view_column_type),
            ("z", view_column_type),
            ("w", view_column_type),
        ]
        models.StructModel.__init__(self, dmm, fe_type, members)


# Make fields accessible in JIT code
make_attribute_wrapper(QuatViewColumnType, "x", "x")
make_attribute_wrapper(QuatViewColumnType, "y", "y")
make_attribute_wrapper(QuatViewColumnType, "z", "z")
make_attribute_wrapper(QuatViewColumnType, "w", "w")


@numba.extending.unbox(QuatViewColumnType)
def unbox_quat_view_column(typ, obj, c):
    """
    Convert Python QuatViewColumn object to native struct.

    Extracts .x, .y, .z, .w properties and unboxes each as a ViewColumn.
    """
    # Create native struct
    quat_val = cgutils.create_struct_proxy(typ)(c.context, c.builder)

    # Extract x, y, z, w properties
    x_obj = c.pyapi.object_getattr_string(obj, "x")
    y_obj = c.pyapi.object_getattr_string(obj, "y")
    z_obj = c.pyapi.object_getattr_string(obj, "z")
    w_obj = c.pyapi.object_getattr_string(obj, "w")

    # Unbox each ViewColumn
    x_native = c.unbox(view_column_type, x_obj)
    y_native = c.unbox(view_column_type, y_obj)
    z_native = c.unbox(view_column_type, z_obj)
    w_native = c.unbox(view_column_type, w_obj)

    # Check for errors during unboxing
    err = cgutils.is_not_null(c.builder, c.pyapi.err_occurred())

    # Store in struct
    quat_val.x = x_native.value
    quat_val.y = y_native.value
    quat_val.z = z_native.value
    quat_val.w = w_native.value

    return numba.core.pythonapi.NativeValue(quat_val._getvalue(), is_error=err)


# ============================================================================
# Export the type for type hints
# ============================================================================

__all__ = [
    "QuatViewColumn",
    "QuatViewColumnType",
    "Vec3ViewColumn",
    "Vec3ViewColumnType",
    "ViewColumn",
    "ViewColumnType",
    "quat_view_column_type",
    "vec3_view_column_type",
    "view_column_type",
    "view_column_type_i32",
    "view_column_type_i64",
]
