#[derive(Debug, thiserror::Error)]
pub enum TypeError {
    // stage 1
    #[error("Fail to merge types")]
    MergeFail,
    #[error("Primitive type mismatch")]
    PrimitiveMismatch,
    #[error("Size mismatch")]
    SizeMismatch,
    #[error("Member length mismatch")]
    MemberLengthMismatch,
    #[error("Vtable mismatch")]
    VtableMismatch,
    #[error("Member {0} name mismatch")]
    MemberNameMismatch(usize),
    #[error("Member {0} is_base mismatch")]
    MemberIsBaseMismatch(usize),
    #[error("Member {0} offset mismatch")]
    MemberOffsetMismatch(usize),
    #[error("Enumerator {0} mismatch")]
    EnumeratorMismatch(usize),
    #[error("Array length mismatch")]
    ArrayLengthMismatch,
    #[error("Subroutine mismatch")]
    SubroutineMismatch,
    #[error("PTMF this type mismatch")]
    ThisTypeMismatch,
    #[error("Types are unrelated")]
    Unrelated,

    // stage 3
    #[error("Namespace not found: 0x{0:08x}")]
    UnlinkedNamespace(usize),

    // stage 4 errors
    #[error("No base name available")]
    NoBaseName,
    #[error("There are unresolved names")]
    UnresolvedNames,
    #[error("There are duplicated names")]
    DuplicatedNames,

    // stage 5 errors
    #[error("Types of different sizes were grouped")]
    InconsistentSize,
    #[error("Type is not sized")]
    NotSized,

    // stage 6 errors
    #[error("Type `{0}` is referenced but not actually generated")]
    BrokenTypeRef(String),
    #[error("The base type for a PTMF must be a named type")]
    UnexpectedUnnamedPtmfThis,
    #[error("Types with the same name have different definitions")]
    ConflictingType,
    #[error("Invalid member layout for struct")]
    InvalidLayout,
}
