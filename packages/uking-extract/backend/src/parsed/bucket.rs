use std::collections::{BTreeMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use error_stack::{report, Result, ResultExt};

use super::{NamespaceMap, Offset, TypeComp, TypeError, TypeInfo, TypeName};

/// A bucket of types, used in type resolution
pub struct Bucket {
    /// The type of bucket
    pub type_: BucketType,
    /// Names of the types in the bucket
    pub names: HashSet<TypeName>,
    /// Remaining candidates
    pub candidates: Vec<Offset>,
    /// Original candidates
    pub original_candidates: Vec<Offset>,
}

pub enum BucketSize {
    Size(Option<usize>),
    NotResolved,
}

impl Bucket {
    pub fn new(off: Vec<Offset>) -> Self {
        Self {
            type_: BucketType::Unknown,
            names: Default::default(),
            candidates: off.clone(),
            original_candidates: off,
        }
    }

    pub fn get_size(
        &self,
        off2info: &BTreeMap<Offset, TypeInfo>,
        off2bkt: &BTreeMap<Offset, Offset>,
        bkt2name: &BTreeMap<Offset, TypeName>,
        bkt2size: &BTreeMap<Offset, Option<usize>>,
    ) -> Result<BucketSize, TypeError> {
        let mut size = BucketSize::NotResolved;
        let mut size_name = HashSet::<TypeName>::new();
        for off in &self.original_candidates {
            let info = off2info.get(off).unwrap();
            let new_size = match info {
                TypeInfo::Prim(p) => p.size(),
                TypeInfo::Typedef(_, _) => continue,
                TypeInfo::Enum(x) => {
                    if x.is_decl && x.size == 0 {
                        continue;
                    }
                    Some(x.size)
                }
                TypeInfo::Struct(x) => {
                    if x.is_decl && x.size == 0 {
                        continue;
                    }
                    Some(x.size)
                }
                TypeInfo::Union(x) => {
                    if x.is_decl && x.size == 0 {
                        continue;
                    }
                    Some(x.size)
                }
                TypeInfo::Comp(c) => match c {
                    TypeComp::Ptr(_) => Some(8),
                    TypeComp::Array(t, l) => {
                        let bkt = off2bkt.get(t).unwrap();
                        if let Some(s) = bkt2size.get(bkt) {
                            match s {
                                None => {
                                    let r = Err(report!(TypeError::NotSized))
                                        .attach_printable(format!("For offset {}", off))
                                        .attach_printable("Array of unsized type");
                                    return r;
                                }
                                Some(s) => Some(s * l),
                            }
                        } else {
                            return Ok(BucketSize::NotResolved);
                        }
                    }
                    TypeComp::Subroutine(_) => None,
                    TypeComp::Ptmf(_, _) => Some(16),
                },
            };
            if let BucketSize::Size(old_size) = size {
                if old_size != new_size {
                    let r = Err(report!(TypeError::InconsistentSize))
                        .attach_printable(format!("For offset {}", off))
                        .attach_printable(format!(
                            "Old size: {:?} != New size: {:?}",
                            old_size, new_size
                        ))
                        .attach_printable(format!(
                            "Names: {}",
                            size_name
                                .iter()
                                .map(|x| x.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    return r;
                }
            } else {
                size = BucketSize::Size(new_size);
                let bkt = off2bkt.get(off).unwrap();
                let name = bkt2name.get(bkt).unwrap();
                size_name.insert(name.clone());
            }
        }
        Ok(size)
    }

    pub fn check(&self, id: &Offset, off2info: &BTreeMap<Offset, TypeInfo>) {
        if self.candidates.is_empty() {
            panic!("Empty bucket {}: {:?}", id, self.names);
        }
        if self.type_ == BucketType::Unknown {
            panic!("Unknown type in bucket {}", id);
        }
        for c in &self.candidates {
            let info = off2info.get(c).unwrap();
            if !matches!(info, TypeInfo::Typedef(_, _)) {
                return;
            }
        }
        panic!("All candidates are typedefs in bucket {}", id);
    }

    pub fn reduce(&mut self, off2info: &BTreeMap<Offset, TypeInfo>, namespaces: &NamespaceMap) {
        if self.type_ == BucketType::Prim {
            // if self is already reduced to primitive, no need to do anything
            return;
        }
        for candidate in std::mem::take(&mut self.candidates) {
            let info = off2info.get(&candidate).unwrap();
            match info {
                TypeInfo::Prim(p) => {
                    // if bucket contains primitive, reduce the whole bucket to that primitive
                    self.type_ = BucketType::Prim;
                    self.names.clear();
                    self.names.insert(TypeName::Prim(*p));
                    self.candidates.clear();
                    self.candidates.push(candidate);
                    return;
                }
                TypeInfo::Typedef(name, _) => {
                    // for typedef, absorb it into the names
                    self.names.insert(TypeName::Name(name.clone()));
                }
                TypeInfo::Enum(x) => {
                    // take the name
                    if let Some(name) = &x.name {
                        self.names.insert(TypeName::Name(name.clone()));
                    } else {
                        let hash = hash_members(x.enumerators.iter().map(|x| x.0.as_str()));
                        let name =
                            TypeName::anonymous_enum(candidate.into(), namespaces, hash).unwrap();
                        self.names.insert(name);
                    }
                    if BucketType::Enum.is_preferred_over(&self.type_) {
                        self.type_ = BucketType::Enum;
                        self.candidates.clear();
                    }
                    self.candidates.push(candidate);
                }
                TypeInfo::Comp(_) => {
                    if BucketType::Comp.is_preferred_over(&self.type_) {
                        // change self type to comp
                        self.type_ = BucketType::Comp;
                        self.candidates.clear();
                    }
                    // self is already comp, add the candidate
                    self.candidates.push(candidate);
                }
                TypeInfo::Struct(x) => {
                    // take the name
                    if let Some(name) = &x.name {
                        self.names.insert(TypeName::Name(name.clone()));
                    } else {
                        let hash = hash_members(
                            x.members
                                .iter()
                                .map(|x| x.name.as_deref().unwrap_or("anonymous")),
                        );
                        let name =
                            TypeName::anonymous_struct(candidate.into(), namespaces, hash).unwrap();
                        self.names.insert(name);
                    }
                    if BucketType::Struct.is_preferred_over(&self.type_) {
                        self.type_ = BucketType::Struct;
                        self.candidates.clear();
                    }
                    self.candidates.push(candidate);
                }
                TypeInfo::Union(x) => {
                    // take the name
                    if let Some(name) = &x.name {
                        self.names.insert(TypeName::Name(name.clone()));
                    } else {
                        let hash = hash_members(
                            x.members
                                .iter()
                                .map(|x| x.0.as_deref().unwrap_or("anonymous")),
                        );
                        let name =
                            TypeName::anonymous_union(candidate.into(), namespaces, hash).unwrap();
                        self.names.insert(name);
                    }
                    if BucketType::Union.is_preferred_over(&self.type_) {
                        self.type_ = BucketType::Union;
                        self.candidates.clear();
                    }
                    self.candidates.push(candidate);
                }
            }
        }
        // remove declarations if there are any non-declarations
        if self.candidates.len() > 1 {
            let mut has_decl = false;
            let mut has_non_decl = false;
            for candidates in &self.candidates {
                let info = off2info.get(candidates).unwrap();
                if info.is_decl() {
                    has_decl = true;
                } else {
                    has_non_decl = true;
                }
            }
            if has_decl && has_non_decl {
                self.candidates
                    .retain(|candidate| !off2info.get(candidate).unwrap().is_decl())
            }
        }

        // clear names for composite type, because we will use a composite name
        if self.type_ == BucketType::Comp {
            self.names.clear();
        }
    }

    pub fn reduce_comp_name(
        &mut self,
        off2info: &BTreeMap<Offset, TypeInfo>,
        off2bkt: &BTreeMap<Offset, Offset>,
        bkt2name: &BTreeMap<Offset, TypeName>,
    ) {
        'out: for candidate in std::mem::take(&mut self.candidates) {
            let info = off2info.get(&candidate).unwrap();
            let c = match info {
                TypeInfo::Comp(c) => c,
                _ => continue,
            };
            match c {
                TypeComp::Ptr(t) => {
                    let bkt_t = off2bkt.get(t).unwrap();
                    if let Some(name) = bkt2name.get(bkt_t) {
                        self.names.insert(TypeName::pointer(name.clone()));
                    } else {
                        self.candidates.push(candidate);
                    }
                }
                TypeComp::Array(t, l) => {
                    let bkt_t = off2bkt.get(t).unwrap();
                    if let Some(name) = bkt2name.get(bkt_t) {
                        self.names.insert(TypeName::array(name.clone(), *l));
                    } else {
                        self.candidates.push(candidate);
                    }
                }
                TypeComp::Subroutine(sub) => {
                    let bkt_ret = off2bkt.get(&sub.retty).unwrap();
                    let name_ret = match bkt2name.get(bkt_ret) {
                        Some(name) => name.clone(),
                        None => {
                            self.candidates.push(candidate);
                            continue;
                        }
                    };
                    let mut params = Vec::new();
                    for t in &sub.params {
                        let bkt_t = off2bkt.get(t).unwrap();
                        if let Some(name) = bkt2name.get(bkt_t) {
                            params.push(name.clone());
                        } else {
                            self.candidates.push(candidate);
                            continue 'out;
                        }
                    }
                    let sub = TypeComp::subroutine(name_ret, params);
                    self.names.insert(TypeName::Comp(Box::new(sub)));
                }
                TypeComp::Ptmf(this, sub) => {
                    let bkt_this = off2bkt.get(this).unwrap();
                    let name_this = match bkt2name.get(bkt_this) {
                        Some(name) => name.clone(),
                        None => {
                            self.candidates.push(candidate);
                            continue;
                        }
                    };
                    let bkt_ret = off2bkt.get(&sub.retty).unwrap();
                    let name_ret = match bkt2name.get(bkt_ret) {
                        Some(name) => name.clone(),
                        None => {
                            self.candidates.push(candidate);
                            continue;
                        }
                    };
                    let mut params = Vec::new();
                    for t in &sub.params {
                        let bkt_t = off2bkt.get(t).unwrap();
                        if let Some(name) = bkt2name.get(bkt_t) {
                            params.push(name.clone());
                        } else {
                            self.candidates.push(candidate);
                            continue 'out;
                        }
                    }
                    let sub = TypeComp::ptmf(name_this, name_ret, params);
                    self.names.insert(TypeName::Comp(Box::new(sub)));
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BucketType {
    Prim,
    Enum,
    Comp,
    Struct,
    Union,
    Unknown,
}

impl BucketType {
    fn is_preferred_over(&self, other: &BucketType) -> bool {
        match (self, other) {
            (Self::Prim, _) => true,

            (Self::Enum, Self::Prim) => false,
            (Self::Enum, _) => true,

            (Self::Comp, Self::Prim | Self::Enum) => false,
            (Self::Comp, _) => true,

            (Self::Struct, Self::Prim | Self::Enum | Self::Comp) => false,
            (Self::Struct, _) => true,

            (Self::Union, Self::Prim | Self::Enum | Self::Comp | Self::Struct) => false,
            (Self::Union, _) => true,

            (Self::Unknown, _) => false,
        }
    }
}

/// hash members for anonymous types so they don't conflict
fn hash_members<'a>(members: impl Iterator<Item = &'a str>) -> u64 {
    let mut string_for_hash = String::new();
    for m in members {
        string_for_hash.push_str(&format!("{}{}", m.len(), m));
    }
    let mut hasher = DefaultHasher::new();
    string_for_hash.hash(&mut hasher);
    hasher.finish()
}
