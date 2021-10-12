use std::fmt;

/// Enumeration of scalar value types
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Type {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    I64,
    U64,
    F64,
}

const TYPE_STR_BOOL: &str = "bool";
const TYPE_STR_I8: &str = "i8";
const TYPE_STR_U8: &str = "u8";
const TYPE_STR_I16: &str = "i16";
const TYPE_STR_U16: &str = "u16";
const TYPE_STR_I32: &str = "i32";
const TYPE_STR_U32: &str = "u32";
const TYPE_STR_F32: &str = "f32";
const TYPE_STR_I64: &str = "i64";
const TYPE_STR_U64: &str = "u64";
const TYPE_STR_F64: &str = "f64";

impl Type {
    pub const fn as_str(self) -> &'static str {
        use Type::*;
        match self {
            Bool => TYPE_STR_BOOL,
            I8 => TYPE_STR_I8,
            U8 => TYPE_STR_U8,
            I16 => TYPE_STR_I16,
            U16 => TYPE_STR_U16,
            I32 => TYPE_STR_I32,
            U32 => TYPE_STR_U32,
            F32 => TYPE_STR_F32,
            I64 => TYPE_STR_I64,
            U64 => TYPE_STR_U64,
            F64 => TYPE_STR_F64,
        }
    }

    pub fn try_from_str(s: &str) -> Option<Type> {
        // TODO: Declare as `const fn` when supported
        match s {
            TYPE_STR_BOOL => Some(Type::Bool),
            TYPE_STR_I8 => Some(Type::I8),
            TYPE_STR_U8 => Some(Type::U8),
            TYPE_STR_I16 => Some(Type::I16),
            TYPE_STR_U16 => Some(Type::U16),
            TYPE_STR_I32 => Some(Type::I32),
            TYPE_STR_U32 => Some(Type::U32),
            TYPE_STR_F32 => Some(Type::F32),
            TYPE_STR_I64 => Some(Type::I64),
            TYPE_STR_U64 => Some(Type::U64),
            TYPE_STR_F64 => Some(Type::F64),
            _ => None,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tagged union of scalar values
///
/// Numbers are always stored with 64-bit precision. Using
/// smaller sizes would have no advantages regarding the
/// size of this product type.
///
/// No conversions between signed/unsigned and integer/floating-point
/// numbers to prevent using wrong types unintentionally! The same
/// argument applies to boolean/integer conversions.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// Boolean
    Bool(bool),
    /// 8-bit signed integer
    I8(i8),
    /// 8-bit unsigned integer
    U8(u8),
    /// 16-bit signed integer
    I16(i16),
    /// 16-bit unsigned integer
    U16(u16),
    /// 32-bit signed integer
    I32(i32),
    /// 32-bit unsigned integer
    U32(u32),
    /// 32-bit floating-point number (double precision)
    F32(f32),
    /// 64-bit signed integer
    I64(i64),
    /// 64-bit unsigned integer
    U64(u64),
    /// 64-bit floating-point number (double precision)
    F64(f64),
}

impl Value {
    pub const fn from_bool(val: bool) -> Self {
        Self::Bool(val)
    }

    pub const fn from_i8(val: i8) -> Self {
        Self::I8(val)
    }

    pub const fn from_u8(val: u8) -> Self {
        Self::U8(val)
    }

    pub const fn from_i16(val: i16) -> Self {
        Self::I16(val)
    }

    pub const fn from_u16(val: u16) -> Self {
        Self::U16(val)
    }

    pub const fn from_i32(val: i32) -> Self {
        Self::I32(val)
    }

    pub const fn from_u32(val: u32) -> Self {
        Self::U32(val)
    }

    pub const fn from_i64(val: i64) -> Self {
        Self::I64(val)
    }

    pub const fn from_u64(val: u64) -> Self {
        Self::U64(val)
    }

    pub const fn from_f32(val: f32) -> Self {
        Self::F32(val)
    }

    pub const fn from_f64(val: f64) -> Self {
        Self::F64(val)
    }

    pub const fn to_bool(self) -> Option<bool> {
        match self {
            Self::Bool(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_i8(self) -> Option<i8> {
        match self {
            Self::I8(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_u8(self) -> Option<u8> {
        match self {
            Self::U8(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_i16(self) -> Option<i16> {
        match self {
            Self::I8(val) => Some(val.into()),
            Self::U8(val) => Some(val.into()),
            Self::I16(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_u16(self) -> Option<u16> {
        match self {
            Self::U8(val) => Some(val.into()),
            Self::U16(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_i32(self) -> Option<i32> {
        match self {
            Self::I8(val) => Some(val.into()),
            Self::U8(val) => Some(val.into()),
            Self::I16(val) => Some(val.into()),
            Self::U16(val) => Some(val.into()),
            Self::I32(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_u32(self) -> Option<u32> {
        match self {
            Self::U8(val) => Some(val.into()),
            Self::U16(val) => Some(val.into()),
            Self::U32(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_i64(self) -> Option<i64> {
        match self {
            Self::I8(val) => Some(val.into()),
            Self::U8(val) => Some(val.into()),
            Self::I16(val) => Some(val.into()),
            Self::U16(val) => Some(val.into()),
            Self::I32(val) => Some(val.into()),
            Self::U32(val) => Some(val.into()),
            Self::I64(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_u64(self) -> Option<u64> {
        match self {
            Self::U8(val) => Some(val.into()),
            Self::U16(val) => Some(val.into()),
            Self::U32(val) => Some(val.into()),
            Self::U64(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_f32(self) -> Option<f32> {
        match self {
            Self::I8(val) => Some(val.into()),
            Self::U8(val) => Some(val.into()),
            Self::I16(val) => Some(val.into()),
            Self::U16(val) => Some(val.into()),
            Self::F32(val) => Some(val),
            _ => None,
        }
    }

    pub fn to_f64(self) -> Option<f64> {
        match self {
            Self::I8(val) => Some(val.into()),
            Self::U8(val) => Some(val.into()),
            Self::I16(val) => Some(val.into()),
            Self::U16(val) => Some(val.into()),
            Self::I32(val) => Some(val.into()),
            Self::U32(val) => Some(val.into()),
            Self::F32(val) => Some(val.into()),
            Self::F64(val) => Some(val),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Value::*;
        match self {
            Bool(val) => write!(f, "{}", val),
            I8(val) => write!(f, "{}", val),
            U8(val) => write!(f, "{}", val),
            I16(val) => write!(f, "{}", val),
            U16(val) => write!(f, "{}", val),
            I32(val) => write!(f, "{}", val),
            U32(val) => write!(f, "{}", val),
            F32(val) => write!(f, "{}", val),
            I64(val) => write!(f, "{}", val),
            U64(val) => write!(f, "{}", val),
            F64(val) => write!(f, "{}", val),
        }
    }
}

impl Value {
    pub const fn to_type(self) -> Type {
        use self::Value::*;
        match self {
            Bool(_) => Type::Bool,
            I8(_) => Type::I8,
            U8(_) => Type::U8,
            I16(_) => Type::I16,
            U16(_) => Type::U16,
            I32(_) => Type::I32,
            U32(_) => Type::U32,
            F32(_) => Type::F32,
            I64(_) => Type::I64,
            U64(_) => Type::U64,
            F64(_) => Type::F64,
        }
    }
}

impl From<Value> for Type {
    fn from(from: Value) -> Self {
        from.to_type()
    }
}

impl From<bool> for Value {
    fn from(from: bool) -> Self {
        Self::from_bool(from)
    }
}

impl From<i8> for Value {
    fn from(from: i8) -> Self {
        Self::from_i8(from)
    }
}

impl From<u8> for Value {
    fn from(from: u8) -> Self {
        Self::from_u8(from)
    }
}

impl From<i16> for Value {
    fn from(from: i16) -> Self {
        Self::from_i16(from)
    }
}

impl From<u16> for Value {
    fn from(from: u16) -> Self {
        Self::from_u16(from)
    }
}

impl From<i32> for Value {
    fn from(from: i32) -> Self {
        Self::from_i32(from)
    }
}

impl From<u32> for Value {
    fn from(from: u32) -> Self {
        Self::from_u32(from)
    }
}

impl From<f32> for Value {
    fn from(from: f32) -> Self {
        Self::from_f32(from)
    }
}

impl From<i64> for Value {
    fn from(from: i64) -> Self {
        Self::from_i64(from)
    }
}

impl From<u64> for Value {
    fn from(from: u64) -> Self {
        Self::from_u64(from)
    }
}

impl From<f64> for Value {
    fn from(from: f64) -> Self {
        Self::from_f64(from)
    }
}

#[cfg(test)]
mod tests;
