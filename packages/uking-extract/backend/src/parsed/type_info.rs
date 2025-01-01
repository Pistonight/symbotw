use std::collections::BTreeMap;

use super::{Offset, Subroutine, TypeComp, TypePrim};

/// Information of the type linked to the DWARF debug info
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeInfo {
    /// Primitive type
    Prim(TypePrim),
    /// Typedef <other> name; Other is offset in debug info
    Typedef(String, Offset),
    /// Struct or Class
    Struct(StructInfo),
    /// Enum
    Enum(EnumInfo),
    /// Union
    Union(UnionInfo),
    /// Compound type
    Comp(TypeComp<Offset>),
}

impl TypeInfo {
    pub fn pointer(ty: Offset) -> Self {
        Self::Comp(TypeComp::Ptr(ty))
    }
    pub fn subroutine(retty: Offset, params: Vec<Offset>) -> Self {
        Self::Comp(TypeComp::subroutine(retty, params))
    }
    #[allow(dead_code)]
    pub fn ptmf(this_ty: Offset, retty: Offset, params: Vec<Offset>) -> Self {
        Self::Comp(TypeComp::ptmf(this_ty, retty, params))
    }
    pub fn is_decl(&self) -> bool {
        match self {
            Self::Struct(s) => s.is_decl,
            Self::Enum(e) => e.is_decl,
            Self::Union(u) => u.is_decl,
            _ => false,
        }
    }

    pub fn slow_size(&self, off2info: &BTreeMap<Offset, Self>) -> Option<usize> {
        match self {
            Self::Prim(p) => p.size(),
            Self::Typedef(_, ty) => off2info.get(ty).unwrap().slow_size(off2info),
            Self::Struct(s) => Some(s.size),
            Self::Enum(e) => Some(e.size),
            Self::Union(u) => Some(u.size),
            Self::Comp(TypeComp::Ptr(_)) => Some(8),
            Self::Comp(TypeComp::Array(ty, len)) => {
                let ty_size = off2info.get(ty).unwrap().slow_size(off2info)?;
                Some(ty_size * len)
            }
            Self::Comp(TypeComp::Subroutine(_)) => None,
            Self::Comp(TypeComp::Ptmf(_, _)) => Some(16),
        }
    }
}

impl std::fmt::Display for TypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prim(p) => write!(f, "{}", p),
            Self::Typedef(name, ty) => write!(f, "typedef {ty} {name}"),
            Self::Struct(s) => write!(f, "struct {}", s.name.as_deref().unwrap_or("<unnamed>")),
            Self::Enum(e) => write!(f, "enum {}", e.name.as_deref().unwrap_or("<unnamed>")),
            Self::Union(u) => write!(f, "union {}", u.name.as_deref().unwrap_or("<unnamed>")),
            Self::Comp(c) => write!(f, "{}", c),
        }
    }
}

/// Struct information linked to the DWARF debug info
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructInfo {
    /// The name of the struct, or an empty string if it is unnamed/anonymous
    pub name: Option<String>,
    /// If the struct is only declared, not defined
    pub is_decl: bool,
    /// The vtable of the struct. It could be incomplete before type resolution
    pub vtable: VtableInfo,
    /// The size of the struct in bytes
    pub size: usize,
    /// The members of the struct
    pub members: Vec<MemberInfo>,
}

/// Information about a member of a struct
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberInfo {
    /// Offset of the member in the struct
    pub offset: usize,
    /// Name of the member, or None if it is unnamed
    pub name: Option<String>,
    /// If the member is a base type (declared with DW_TAG_inheritance)
    pub is_base: bool,
    /// The type of the member, linked to the DWARF debug info
    pub ty_offset: Offset,

    pub is_bitfield: bool,
    pub byte_size: usize,
}

impl MemberInfo {
    pub fn make_bitfield(&mut self, size: usize) {
        self.is_bitfield = true;
        self.byte_size = size;
        self.name = Some(format!("bitfield_{:x}", self.offset));
    }
}

/// Information about a vtable
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VtableInfo {
    inner: Vec<Option<VfptrInfo>>,
}

impl VtableInfo {
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Check if self and other are equivalent for type resolution
    ///
    /// 2 vtables are equivalent if:
    /// - their first `min(self.len(), other.len())` entries are equivalent
    ///
    /// 2 entries are equivalent if:
    /// - they are both destructors, or
    /// - they have the same name
    pub fn is_equiv_to(&self, other: &Self) -> bool {
        // since vtable could be incomplete, we need to check the names
        // instead of relying on the length
        for (a, b) in self.inner.iter().zip(other.inner.iter()) {
            match (a, b) {
                (Some(a), Some(b)) => {
                    if a.is_dtor() && b.is_dtor() {
                        continue;
                    }
                    if a.name != b.name {
                        return false;
                    }
                }
                (None, None) => {}
                _ => return false,
            }
        }
        true
    }

    pub fn inherit_from_base(&mut self, base_vtable: &Self) {
        for (i, vfptr) in base_vtable.inner.iter().enumerate() {
            let entry = self.ensure(i);
            if entry.is_none() {
                if let Some(vfptr) = vfptr {
                    let mut cloned = vfptr.clone();
                    cloned.is_from_base = true;
                    *entry = Some(cloned);
                } else {
                    *entry = None;
                }
            }
        }
    }

    pub fn ensure(&mut self, idx: usize) -> &mut Option<VfptrInfo> {
        if idx >= self.inner.len() {
            self.inner.resize(idx + 1, None);
        }
        &mut self.inner[idx]
    }

    /// Set the vfptr at the given index. Return false if there's already
    /// an info set at that index, unless the old info is from base
    /// and the new info is not from base
    pub fn set(&mut self, idx: usize, vfptr: VfptrInfo) -> bool {
        let entry = self.ensure(idx);
        match entry {
            Some(old) => {
                if old.is_from_base && !vfptr.is_from_base {
                    *old = vfptr;
                    true
                } else {
                    false
                }
            }
            None => {
                entry.replace(vfptr);
                true
            }
        }
    }

    /// Place virtual destructors
    ///
    /// Looks for the first 2 adjacent entries that each is either empty or a destructor,
    /// replace them with the given destructor info, and rename the first
    /// one D1 and the second D0
    pub fn place_dtor(&mut self, mut vdtorptr: VfptrInfo) {
        let mut placement = self.inner.len();
        for i in 0..self.inner.len() {
            let a_can_place = self.inner[i].as_ref().map(|x| x.is_dtor()).unwrap_or(true);
            let b_can_place = if i < self.inner.len() - 1 {
                self.inner[i + 1]
                    .as_ref()
                    .map(|x| x.is_dtor())
                    .unwrap_or(true)
            } else {
                true
            };
            if a_can_place && b_can_place {
                placement = i;
                break;
            }
        }
        let i = placement;
        let mut d0 = vdtorptr.clone();
        d0.name.push_str("D0");
        self.ensure(i + 1).replace(d0);
        vdtorptr.name.push_str("D1");
        self.ensure(i).replace(vdtorptr);
    }

    /// Check the vtable has any vacent entries, including at the end
    ///
    /// Returns the first vacant index
    pub fn has_vacant(&self) -> Option<usize> {
        self.inner.iter().position(|x| x.is_none())
    }

    pub fn merge(&mut self, other: &Self) {
        if other.len() > self.len() {
            for i in self.len()..other.len() {
                self.inner.push(other.inner[i].clone());
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &VfptrInfo> {
        self.inner.iter().filter_map(|x| x.as_ref())
    }

    pub fn into_iter(self) -> impl Iterator<Item = VfptrInfo> {
        if self.has_vacant().is_some() {
            panic!("Vtables after resolving should not have None entries");
        }
        self.inner.into_iter().map(|x| x.unwrap())
    }
}

/// Information about a virtual function pointer (one entry in the vtable)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfptrInfo {
    /// Name of the function (i.e. name of the field of the vtbl struct)
    pub name: String,
    /// If this entry is from a base class
    pub is_from_base: bool,
    /// Function type
    pub function: Subroutine<Offset>,
}
impl VfptrInfo {
    pub fn is_dtor(&self) -> bool {
        self.name.starts_with('~')
    }
}

/// Information about an enum
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumInfo {
    /// The name of the enum, or an empty string if it is unnamed/anonymous
    pub name: Option<String>,
    /// The size of the enum in bytes
    pub size: usize,
    /// If the enum is only declared, not defined
    pub is_decl: bool,
    /// The enumerators of the enum (name, value)
    pub enumerators: Vec<(String, i128)>,
}

/// Information about a union
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionInfo {
    /// The name of the union, or an empty string if it is unnamed/anonymous
    pub name: Option<String>,
    /// The size of the union in bytes
    pub size: usize,
    /// If the union is only declared, not defined
    pub is_decl: bool,
    /// The members of the union (name, type_offset)
    pub members: Vec<(Option<String>, Offset)>,
}

impl UnionInfo {
    pub fn decl(name: Option<String>, size: usize) -> Self {
        Self {
            name,
            size,
            is_decl: true,
            members: Vec::new(),
        }
    }
}
