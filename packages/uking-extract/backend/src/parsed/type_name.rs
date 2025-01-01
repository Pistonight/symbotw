use error_stack::{Result, ResultExt};

use super::{NamespaceMap, TypeComp, TypeError, TypePrim, TypeYaml};

/// Information of resolved type name
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeName {
    /// Primitive
    Prim(TypePrim),
    /// Named type
    Name(String),
    /// Compound, indexing into offset_to_bucket
    Comp(Box<TypeComp<TypeName>>),
}

impl TypeName {
    pub fn anonymous_struct(
        offset: usize,
        ns: &NamespaceMap,
        hash: u64,
    ) -> Result<Self, TypeError> {
        Self::anonymous(offset, ns, "struct", hash)
    }
    pub fn anonymous_enum(offset: usize, ns: &NamespaceMap, hash: u64) -> Result<Self, TypeError> {
        Self::anonymous(offset, ns, "enum", hash)
    }
    pub fn anonymous_union(offset: usize, ns: &NamespaceMap, hash: u64) -> Result<Self, TypeError> {
        Self::anonymous(offset, ns, "union", hash)
    }
    /// Create an anonymous type name
    fn anonymous(
        offset: usize,
        ns: &NamespaceMap,
        tag: &str,
        hash: u64,
    ) -> Result<Self, TypeError> {
        let name = ns
            .get(offset, &format!("anonymous_{}x{:016x}", tag, hash))
            .attach_printable_lazy(|| format!("While creating anonymous {}", tag))?;
        Ok(Self::Name(name))
    }

    pub fn pointer(to: Self) -> Self {
        Self::Comp(Box::new(TypeComp::Ptr(to)))
    }
    pub fn array(to: Self, len: usize) -> Self {
        Self::Comp(Box::new(TypeComp::Array(to, len)))
    }
    pub fn referenced_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.add_referenced_names(&mut names);
        names
    }
    fn add_referenced_names(&self, names: &mut Vec<String>) {
        match self {
            Self::Prim(_) => (),
            Self::Name(name) => {
                names.push(name.to_string());
            }
            Self::Comp(c) => match c.as_ref() {
                TypeComp::Ptr(t) | TypeComp::Array(t, _) => t.add_referenced_names(names),
                TypeComp::Subroutine(sub) => {
                    for t in sub.iter() {
                        t.add_referenced_names(names);
                    }
                }
                TypeComp::Ptmf(this_ty, sub) => {
                    // ptmf also needs the _ptmf struct
                    if let Self::Name(name) = this_ty {
                        names.push(format!("{}_ptmf", name));
                    } else {
                        panic!("ptmf of non-name type: {:?}", this_ty);
                    }
                    this_ty.add_referenced_names(names);
                    for t in sub.iter() {
                        t.add_referenced_names(names);
                    }
                }
            },
        }
    }
}

impl TypeYaml for TypeName {
    fn yaml_string(&self) -> String {
        match self {
            TypeName::Prim(p) => p.yaml_string(),
            TypeName::Name(n) => format!("'\"{}\"'", n),
            TypeName::Comp(c) => c.yaml_string(),
        }
    }
}

impl std::fmt::Display for TypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeName::Prim(prim) => write!(f, "{}", prim),
            TypeName::Name(name) => write!(f, "\"{}\"", name),
            TypeName::Comp(c) => write!(f, "{}", c),
        }
    }
}

impl TypeName {
    /// Return if this type name is primitive, or composed of primitive types
    pub fn is_primitive(&self) -> bool {
        match self {
            Self::Prim(_) => true,
            Self::Comp(c) => match c.as_ref() {
                TypeComp::Ptr(t) => t.is_primitive(),
                TypeComp::Array(t, _) => t.is_primitive(),
                TypeComp::Subroutine(s) => s.iter().all(|t| t.is_primitive()),
                TypeComp::Ptmf(this_ty, s) => {
                    this_ty.is_primitive() && s.iter().all(|t| t.is_primitive())
                }
            },
            _ => false,
        }
    }

    /// Count the number of anonymous types in this type name
    pub fn count_anonymous(&self) -> usize {
        match self {
            Self::Prim(_) => 0,
            Self::Name(s) => {
                if s.contains("anonymous") {
                    1
                } else {
                    0
                }
            }
            Self::Comp(c) => match c.as_ref() {
                TypeComp::Ptr(t) => t.count_anonymous(),
                TypeComp::Array(t, _) => t.count_anonymous(),
                TypeComp::Subroutine(sub) => sub.iter().map(|t| t.count_anonymous()).sum::<usize>(),
                TypeComp::Ptmf(this_ty, sub) => {
                    this_ty.count_anonymous()
                        + sub.iter().map(|t| t.count_anonymous()).sum::<usize>()
                }
            },
        }
    }
}

impl PartialOrd for TypeName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Ordering for TypeName, more preferred are larger
impl Ord for TypeName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // prefer primitive types over non-primitive types
        let self_prim = self.is_primitive();
        let other_prim = other.is_primitive();
        match (self_prim, other_prim) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => (),
        }

        // prefer less anonymous types
        let self_anonymous = self.count_anonymous();
        let other_anonymous = other.count_anonymous();
        match self_anonymous.cmp(&other_anonymous) {
            std::cmp::Ordering::Greater => return std::cmp::Ordering::Less,
            std::cmp::Ordering::Less => return std::cmp::Ordering::Greater,
            _ => (),
        }

        let self_name = match self {
            Self::Prim(_) => {
                // self is primitive and other is primitive too, they are probably the same type
                return std::cmp::Ordering::Equal;
            }
            Self::Comp(c) => {
                let other_c = match other {
                    Self::Comp(x) => x,
                    // prefer compound types
                    _ => return std::cmp::Ordering::Greater,
                };
                match c.as_ref() {
                    TypeComp::Ptr(t) | TypeComp::Array(t, _) => {
                        match other_c.as_ref() {
                            TypeComp::Ptr(t_c) | TypeComp::Array(t_c, _) => {
                                return t.cmp(t_c);
                            }
                            // prefer more compounded types
                            _ => return std::cmp::Ordering::Less,
                        }
                    }
                    TypeComp::Subroutine(sub) => {
                        match other_c.as_ref() {
                            TypeComp::Ptr(_) | TypeComp::Array(_, _) => {
                                // prefer more compounded types
                                return std::cmp::Ordering::Greater;
                            }
                            TypeComp::Subroutine(sub_c) => {
                                let mut less_count = 0;
                                let mut more_count = 0;

                                for (t, t_c) in sub.iter().zip(sub_c.iter()) {
                                    match t.cmp(t_c) {
                                        std::cmp::Ordering::Less => {
                                            if more_count > 0 {
                                                return std::cmp::Ordering::Equal;
                                            }
                                            less_count += 1;
                                        }
                                        std::cmp::Ordering::Greater => {
                                            if less_count > 0 {
                                                return std::cmp::Ordering::Equal;
                                            }
                                            more_count += 1;
                                        }
                                        _ => (),
                                    }
                                }

                                if more_count > 0 {
                                    return std::cmp::Ordering::Greater;
                                }
                                if less_count > 0 {
                                    return std::cmp::Ordering::Less;
                                }
                                return std::cmp::Ordering::Equal;
                            }
                            // prefer more compounded types
                            _ => return std::cmp::Ordering::Less,
                        }
                    }
                    TypeComp::Ptmf(this_ty, sub) => {
                        match other_c.as_ref() {
                            TypeComp::Ptmf(this_ty_c, sub_c) => {
                                let mut less_count = 0;
                                let mut more_count = 0;

                                let ty_iter = std::iter::once(this_ty)
                                    .chain(sub.iter())
                                    .zip(std::iter::once(this_ty_c).chain(sub_c.iter()));
                                for (t, t_c) in ty_iter {
                                    match t.cmp(t_c) {
                                        std::cmp::Ordering::Less => {
                                            if more_count > 0 {
                                                return std::cmp::Ordering::Equal;
                                            }
                                            less_count += 1;
                                        }
                                        std::cmp::Ordering::Greater => {
                                            if less_count > 0 {
                                                return std::cmp::Ordering::Equal;
                                            }
                                            more_count += 1;
                                        }
                                        _ => (),
                                    }
                                }

                                if more_count > 0 {
                                    return std::cmp::Ordering::Greater;
                                }
                                if less_count > 0 {
                                    return std::cmp::Ordering::Less;
                                }
                                return std::cmp::Ordering::Equal;
                            }
                            // prefer more compounded types
                            _ => return std::cmp::Ordering::Less,
                        }
                    }
                }
            }
            Self::Name(x) => x,
        };
        let other_name = match other {
            Self::Prim(_) => {
                // prefer primitive over named
                return std::cmp::Ordering::Less;
            }
            Self::Comp(_) => {
                // prefer compound over named
                return std::cmp::Ordering::Less;
            }
            Self::Name(x) => x,
        };

        // prefer less whitespace
        let self_spaces = self_name.chars().filter(|c| c.is_whitespace()).count();
        let other_spaces = other_name.chars().filter(|c| c.is_whitespace()).count();
        match self_spaces.cmp(&other_spaces) {
            std::cmp::Ordering::Greater => return std::cmp::Ordering::Less,
            std::cmp::Ordering::Less => return std::cmp::Ordering::Greater,
            _ => (),
        }

        // prefer less < >
        let self_angle = self_name.chars().filter(|c| *c == '<' || *c == '>').count();
        let other_angle = other_name
            .chars()
            .filter(|c| *c == '<' || *c == '>')
            .count();
        match self_angle.cmp(&other_angle) {
            std::cmp::Ordering::Greater => return std::cmp::Ordering::Less,
            std::cmp::Ordering::Less => return std::cmp::Ordering::Greater,
            _ => (),
        }

        // prefer fewer underscores
        let self_underscores = self_name.chars().filter(|c| *c == '_').count();
        let other_underscores = other_name.chars().filter(|c| *c == '_').count();
        match self_underscores.cmp(&other_underscores) {
            std::cmp::Ordering::Greater => return std::cmp::Ordering::Less,
            std::cmp::Ordering::Less => return std::cmp::Ordering::Greater,
            _ => (),
        }

        // prefer sead
        let self_sead = self_name.starts_with("sead");
        let other_sead = other_name.starts_with("sead");
        match (self_sead, other_sead) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => (),
        }

        // prefer nn
        let self_nn = self_name.starts_with("nn");
        let other_nn = other_name.starts_with("nn");
        match (self_nn, other_nn) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => (),
        }

        // prefer std
        let self_std = self_name.starts_with("std");
        let other_std = other_name.starts_with("std");
        match (self_std, other_std) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => (),
        }

        // prefer ksys
        let self_ksys = self_name.starts_with("ksys");
        let other_ksys = other_name.starts_with("ksys");
        match (self_ksys, other_ksys) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => (),
        }

        // prefer uking
        let self_uking = self_name.starts_with("uking");
        let other_uking = other_name.starts_with("uking");
        match (self_uking, other_uking) {
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            _ => (),
        }

        // prefer shorter
        match other_name.len().cmp(&self_name.len()) {
            std::cmp::Ordering::Greater => return std::cmp::Ordering::Less,
            std::cmp::Ordering::Less => return std::cmp::Ordering::Greater,
            _ => (),
        }

        // for consistent ordering
        self_name.cmp(other_name)
    }
}
