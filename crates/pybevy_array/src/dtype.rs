//! Bounded-array dtypes and their metadata.

/// The bounded set of supported element dtypes.
///
/// Names and item sizes match NumPy 2 on supported (64-bit) platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrayDType {
    Float16,
    Float32,
    Float64,
    Int64,
    Int32,
    Uint32,
    Uint16,
    Uint8,
    Bool,
}

/// Interpreter-neutral DLPack dtype metadata.
///
/// The numeric codes are defined by DLPack: signed integer `0`, unsigned
/// integer `1`, floating point `2`, and boolean `6`. DLPack tensors exported
/// by PyBevy always use one lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DlpackDType {
    pub code: u8,
    pub bits: u8,
    pub lanes: u16,
}

impl ArrayDType {
    /// Return this array dtype's exact DLPack representation.
    pub const fn dlpack(self) -> DlpackDType {
        let (code, bits) = match self {
            ArrayDType::Float16 => (2, 16),
            ArrayDType::Float32 => (2, 32),
            ArrayDType::Float64 => (2, 64),
            ArrayDType::Int64 => (0, 64),
            ArrayDType::Int32 => (0, 32),
            ArrayDType::Uint32 => (1, 32),
            ArrayDType::Uint16 => (1, 16),
            ArrayDType::Uint8 => (1, 8),
            ArrayDType::Bool => (6, 8),
        };
        DlpackDType {
            code,
            bits,
            lanes: 1,
        }
    }

    /// Element size in bytes, matching NumPy.
    pub const fn itemsize(self) -> usize {
        match self {
            ArrayDType::Float16 => 2,
            ArrayDType::Float32 => 4,
            ArrayDType::Float64 => 8,
            ArrayDType::Int64 => 8,
            ArrayDType::Int32 => 4,
            ArrayDType::Uint32 => 4,
            ArrayDType::Uint16 => 2,
            ArrayDType::Uint8 => 1,
            ArrayDType::Bool => 1,
        }
    }

    /// The `str(dtype)` name NumPy reports (note: `Bool` -> `"bool"`).
    pub const fn name(self) -> &'static str {
        match self {
            ArrayDType::Float16 => "float16",
            ArrayDType::Float32 => "float32",
            ArrayDType::Float64 => "float64",
            ArrayDType::Int64 => "int64",
            ArrayDType::Int32 => "int32",
            ArrayDType::Uint32 => "uint32",
            ArrayDType::Uint16 => "uint16",
            ArrayDType::Uint8 => "uint8",
            ArrayDType::Bool => "bool",
        }
    }

    pub const fn is_float(self) -> bool {
        matches!(
            self,
            ArrayDType::Float16 | ArrayDType::Float32 | ArrayDType::Float64
        )
    }

    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            ArrayDType::Int64
                | ArrayDType::Int32
                | ArrayDType::Uint32
                | ArrayDType::Uint16
                | ArrayDType::Uint8
        )
    }

    pub const fn is_signed_integer(self) -> bool {
        matches!(self, ArrayDType::Int64 | ArrayDType::Int32)
    }

    pub const fn is_bool(self) -> bool {
        matches!(self, ArrayDType::Bool)
    }

    /// Resolve a dtype name. Accepts both `"bool"` and the Python constant
    /// spelling `"bool_"`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "float16" => ArrayDType::Float16,
            "float32" => ArrayDType::Float32,
            "float64" => ArrayDType::Float64,
            "int64" => ArrayDType::Int64,
            "int32" => ArrayDType::Int32,
            "uint32" => ArrayDType::Uint32,
            "uint16" => ArrayDType::Uint16,
            "uint8" => ArrayDType::Uint8,
            "bool" | "bool_" => ArrayDType::Bool,
            _ => return None,
        })
    }
}
