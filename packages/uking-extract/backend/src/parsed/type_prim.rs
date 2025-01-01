/// Primitive types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TypePrim {
    #[default]
    /// Empty, void
    Void,
    /// Boolean
    Bool,
    /// Unsigned, 8-bit
    U8,
    /// Unsigned, 16-bit
    U16,
    /// Unsigned, 32-bit
    U32,
    /// Unsigned, 64-bit
    U64,
    /// Unsigned, 128-bit
    U128,
    /// Signed, 8-bit
    I8,
    /// Signed, 16-bit
    I16,
    /// Signed, 32-bit
    I32,
    /// Signed, 64-bit
    I64,
    /// Signed, 128-bit
    I128,
    /// Floating point, 32-bit
    F32,
    /// Floating point, 64-bit
    F64,
    /// Floating point, 128-bit
    F128,
}

impl TypePrim {
    /// Get the size of type in bytes. Return None if the type is void.
    pub fn size(&self) -> Option<usize> {
        Some(match self {
            Self::Void => return None,
            Self::Bool => 1,
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
            Self::U128 | Self::I128 | Self::F128 => 16,
        })
    }

    /// Get the name of the type in IDA
    #[allow(dead_code)]
    pub fn ida_type(&self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Bool => "bool",
            Self::U8 => "unsigned char",
            Self::U16 => "unsigned short",
            Self::U32 => "unsigned int",
            Self::U64 => "unsigned long",
            Self::U128 => "unsigned __int128",
            Self::I8 => "char",
            Self::I16 => "short",
            Self::I32 => "int",
            Self::I64 => "long",
            Self::I128 => "__int128",
            Self::F32 => "float",
            Self::F64 => "double",
            Self::F128 => "long double",
        }
    }
}

impl std::fmt::Display for TypePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TypePrim::Void => write!(f, "void"),
            TypePrim::Bool => write!(f, "bool"),
            TypePrim::U8 => write!(f, "u8"),
            TypePrim::U16 => write!(f, "u16"),
            TypePrim::U32 => write!(f, "u32"),
            TypePrim::U64 => write!(f, "u64"),
            TypePrim::U128 => write!(f, "u128"),
            TypePrim::I8 => write!(f, "i8"),
            TypePrim::I16 => write!(f, "i16"),
            TypePrim::I32 => write!(f, "i32"),
            TypePrim::I64 => write!(f, "i64"),
            TypePrim::I128 => write!(f, "i128"),
            TypePrim::F32 => write!(f, "f32"),
            TypePrim::F64 => write!(f, "f64"),
            TypePrim::F128 => write!(f, "f128"),
        }
    }
}

/// Composite or compound types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeComp<T> {
    /// T* or T&
    Ptr(T),
    /// T[elem_count]
    Array(T, usize),
    /// Function type with (ret_ty, param_ty)
    Subroutine(Subroutine<T>),
    /// Ptr-to-member-function (this_ty, ret_ty, param_ty)
    Ptmf(T, Subroutine<T>),
}

impl<T> TypeComp<T> {
    pub fn subroutine(retty: T, params: Vec<T>) -> Self {
        Self::Subroutine(Subroutine { retty, params })
    }

    pub fn ptmf(this_ty: T, retty: T, params: Vec<T>) -> Self {
        Self::Ptmf(this_ty, Subroutine { retty, params })
    }
}

impl<T: std::fmt::Display> std::fmt::Display for TypeComp<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let param_ty = match self {
            TypeComp::Ptr(t) => return write!(f, "{t}*"),
            TypeComp::Array(t, n) => return write!(f, "{t}[{n}]"),
            TypeComp::Subroutine(Subroutine { retty, params }) => {
                write!(f, "({retty})(")?;
                params
            }
            TypeComp::Ptmf(this_ty, Subroutine { retty, params }) => {
                write!(f, "({this_ty}::{retty})(")?;
                params
            }
        };
        let mut iter = param_ty.iter();
        if let Some(t) = iter.next() {
            write!(f, "{t}")?;
        }
        for t in iter {
            write!(f, ", {t}")?;
        }
        write!(f, ")")
    }
}

/// Subroutine type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subroutine<T> {
    /// Return type
    pub retty: T,
    /// Parameters
    pub params: Vec<T>,
}
impl<T> Subroutine<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.retty).chain(self.params.iter())
    }

    #[allow(dead_code)]
    pub fn into_iter(self) -> impl Iterator<Item = T> {
        std::iter::once(self.retty).chain(self.params)
    }
}

/// Trait to convert the type into a YAML string that can be parsed
/// by the IDAPython script for importing into IDA
///
/// The YAML string has the following format:
/// - It should be a comma separated list of values, so it's a valid list in YAML when put between
///   `[` and `]`
/// - Primitive types are string of the primitive representation:
///   - `void`
///   - `bool`
///   - `i`, `u`, `f` followed by the bit width
/// - Name types (struct, enum, union` are string of the name surrounded by `""`, such as
///   `"ksys::Foo"`
/// - Pointer types have 2 elements: the base type followed by `'*'`
/// - Array types also have 2 elements: the base type followed by `[n]` (a list with a single
///   element that's the element count
/// - Subroutine types have 3 elements: the return type, followed by the string `()`, and a list of parameters
///   - Each parameter is a type, represented as a list with type YAML inside it
/// - PTMF types have 4 elements: the return type, the `'(ptmf)'` string, the type for `this`, and a list of parameters
pub trait TypeYaml {
    fn yaml_string(&self) -> String;
}

impl TypeYaml for TypePrim {
    fn yaml_string(&self) -> String {
        self.to_string()
    }
}

impl<T: TypeYaml> TypeYaml for TypeComp<T> {
    fn yaml_string(&self) -> String {
        let (mut s, param_ty) = match self {
            TypeComp::Ptr(t) => return format!("{},'*'", t.yaml_string()),
            TypeComp::Array(t, n) => return format!("{},[{}]", t.yaml_string(), n),
            TypeComp::Subroutine(Subroutine { retty, params }) => {
                let s = format!("{},'()', [", retty.yaml_string());
                (s, params)
            }
            TypeComp::Ptmf(this_ty, Subroutine { retty, params }) => {
                let s = format!(
                    "{},'(ptmf)', [{}], [",
                    retty.yaml_string(),
                    this_ty.yaml_string()
                );
                (s, params)
            }
        };

        let mut iter = param_ty.iter();
        if let Some(t) = iter.next() {
            s.push_str(&format!("[{}]", t.yaml_string()));
        }
        for t in iter {
            s.push_str(&format!(", [{}]", t.yaml_string()));
        }
        s.push(']');
        s
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Offset(usize);
impl Offset {
    #[allow(dead_code)]
    pub const fn new_const(offset: usize) -> Self {
        Self(offset)
    }
}

impl std::fmt::Display for Offset {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "<0x{:08x}>", self.0)
    }
}

impl std::fmt::Debug for Offset {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Offset(0x{:08x})", self.0)
    }
}

impl From<usize> for Offset {
    fn from(offset: usize) -> Self {
        Self(offset)
    }
}

impl From<Offset> for usize {
    fn from(v: Offset) -> usize {
        v.0
    }
}
