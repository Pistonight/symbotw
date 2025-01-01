use std::collections::BTreeMap;

use error_stack::{report, Result, ResultExt};

use super::{Offset, TypeComp, TypeError, TypeInfo, TypesStage2};

/// Merge types using function/data that references them
pub struct TypesStage1 {
    off2info: BTreeMap<Offset, TypeInfo>,
    off2bkt: BTreeMap<Offset, Offset>,
    buckets: BTreeMap<Offset, Vec<Offset>>,
}

impl TypesStage1 {
    pub fn new(
        off2info: BTreeMap<Offset, TypeInfo>,
        off2bkt: BTreeMap<Offset, Offset>,
        buckets: BTreeMap<Offset, Vec<Offset>>,
    ) -> Self {
        Self {
            off2info,
            off2bkt,
            buckets,
        }
    }
    // pub fn new2(off2info: BTreeMap<Offset, TypeInfo>, merges: Vec<(Offset, Offset)>) -> Self {
    //     let off2bkt = off2info.keys().map(|k| (*k, *k)).collect();
    //     let buckets = off2info.keys().map(|k| (*k, vec![*k])).collect();
    //
    //     let mut merger = Self {
    //         off2info,
    //         off2bkt,
    //         buckets,
    //     };
    //
    //     for (a, b) in merges {
    //         let a = a.into();
    //         let b = b.into();
    //         merger.merge(&a, &b);
    //     }
    //     merger.merge_typedefs();
    //
    //     merger
    // }
    //
    // fn merge_typedefs(&mut self) {
    //     let mut typedefs = Vec::new();
    //     for (offset, ty) in &self.off2info {
    //         if let TypeInfo::Typedef(_, ty_offset) = ty {
    //             typedefs.push((*ty_offset, *offset));
    //         }
    //     }
    //     for (a, b) in typedefs {
    //         self.merge(&a, &b);
    //     }
    // }

    /// Merge the bucket containing b into the bucket containing a
    pub fn merge(&mut self, off_a: &Offset, off_b: &Offset) {
        let bkt_a = *self.off2bkt.get(off_a).unwrap();
        let bkt_b = *self.off2bkt.get(off_b).unwrap();
        if bkt_a == bkt_b {
            return;
        }
        #[cfg(feature = "debug-merge")]
        {
            let bucket_a = self.buckets.get(&bkt_a).unwrap();
            let bucket_b = self.buckets.get(&bkt_b).unwrap();
            if bucket_a.contains(&super::DEBUG_MERGE_OFFSET)
                || bucket_b.contains(&super::DEBUG_MERGE_OFFSET)
            {
                println!("Merge Stage 1: {} {}", off_a, off_b);
            }
        }
        // offsets in b_bucket need to change their offset_to_bucket
        let bucket_b = self.buckets.remove(&bkt_b).unwrap();
        for off_b in &bucket_b {
            self.off2bkt.insert(*off_b, bkt_a);
        }
        self.buckets.get_mut(&bkt_a).unwrap().extend(bucket_b);
    }

    pub fn check_and_merge(&mut self, off_a: &Offset, off_b: &Offset) -> Result<(), TypeError> {
        let mut seen = Vec::new();
        let r = self.merge_recur(off_a, off_b, "Root invocation", &mut seen);
        if !seen.is_empty() {
            panic!("Seen stack not empty: {:?}", seen);
        }
        r
    }

    #[inline]
    fn merge_recur(
        &mut self,
        off_a: &Offset,
        off_b: &Offset,
        tag: &str,
        seen: &mut Vec<(Offset, Offset)>,
    ) -> Result<(), TypeError> {
        self.merge_recur_internal(off_a, off_b, seen)
            .change_context(TypeError::MergeFail)
            .attach_printable_lazy(|| format!("Merging {} and {}", off_a, off_b))
            .attach_printable_lazy(|| tag.to_string())
    }

    /// Check if off_a and off_b, which are expected to be the same type,
    /// can be the same type. If so, merge them into the same bucket
    fn merge_recur_internal(
        &mut self,
        off_a: &Offset,
        off_b: &Offset,
        seen: &mut Vec<(Offset, Offset)>,
    ) -> Result<(), TypeError> {
        // are they already merged?
        let bkt_a = self.off2bkt.get(off_a).unwrap();
        let bkt_b = self.off2bkt.get(off_b).unwrap();
        if bkt_a == bkt_b {
            return Ok(());
        }
        // if we have already seen this combo, don't recurse, only use name
        let already_recursing = seen
            .iter()
            .any(|(a, b)| (*a == *off_a && *b == *off_b) || (*a == *off_b && *b == *off_a));

        let info_a = self.off2info.get(off_a).unwrap().clone();
        let info_b = self.off2info.get(off_b).unwrap().clone();

        match (&info_a, &info_b) {
            (TypeInfo::Prim(a), TypeInfo::Prim(b)) => {
                if a != b {
                    return Err(TypeError::PrimitiveMismatch.into());
                }
                self.merge(off_a, off_b);
                Ok(())
            }
            (TypeInfo::Typedef(_, off_a2), _) => {
                seen.push((*off_a, *off_b));
                let result = self.merge_recur(off_a2, off_b, "Typedef left", seen);
                seen.pop();
                result?;
                self.merge(off_a, off_b);
                Ok(())
            }
            (_, TypeInfo::Typedef(_, off_b2)) => {
                seen.push((*off_a, *off_b));
                let result = self.merge_recur(off_a, off_b2, "Typedef right", seen);
                seen.pop();
                result?;
                self.merge(off_a, off_b);
                Ok(())
            }
            (TypeInfo::Struct(a), TypeInfo::Struct(b)) => {
                // for declaration, name and only should match
                if a.is_decl || b.is_decl {
                    self.merge(off_a, off_b);
                    return Ok(());
                }
                // size, members, vtable must match
                if a.size != b.size {
                    return Err(TypeError::SizeMismatch.into());
                }
                if a.members.len() != b.members.len() {
                    return Err(TypeError::MemberLengthMismatch.into());
                }
                if !a.vtable.is_equiv_to(&b.vtable) {
                    return Err(TypeError::VtableMismatch.into());
                }
                // each member must match
                for (i, (m_a, m_b)) in a.members.iter().zip(b.members.iter()).enumerate() {
                    if m_a.name != m_b.name {
                        return Err(TypeError::MemberNameMismatch(i).into());
                    }
                    if m_a.is_base != m_b.is_base {
                        return Err(TypeError::MemberIsBaseMismatch(i).into());
                    }
                    if m_a.offset != m_b.offset {
                        return Err(TypeError::MemberOffsetMismatch(i).into());
                    }
                }
                if !already_recursing {
                    seen.push((*off_a, *off_b));
                    let mut r = Ok(());
                    for (i, (m_a, m_b)) in a.members.iter().zip(b.members.iter()).enumerate() {
                        r = self.merge_recur(
                            &m_a.ty_offset,
                            &m_b.ty_offset,
                            &format!("Matching type of struct member {i}"),
                            seen,
                        );
                        if r.is_err() {
                            break;
                        }
                    }
                    seen.pop();
                    r?
                }
                self.merge(off_a, off_b);
                Ok(())
            }
            (TypeInfo::Union(a), TypeInfo::Union(b)) => {
                if a.is_decl || b.is_decl {
                    self.merge(off_a, off_b);
                    return Ok(());
                }
                if a.size != b.size {
                    return Err(TypeError::SizeMismatch.into());
                }
                if a.members.len() != b.members.len() {
                    return Err(TypeError::MemberLengthMismatch.into());
                }
                for (i, (m_a, m_b)) in a.members.iter().zip(b.members.iter()).enumerate() {
                    // name
                    if m_a.0 != m_b.0 {
                        return Err(TypeError::MemberNameMismatch(i).into());
                    }
                }
                if !already_recursing {
                    seen.push((*off_a, *off_b));
                    let mut r = Ok(());
                    for (i, (m_a, m_b)) in a.members.iter().zip(b.members.iter()).enumerate() {
                        r = self.merge_recur(
                            &m_a.1,
                            &m_b.1,
                            &format!("Match type of union member {i}"),
                            seen,
                        );
                        if r.is_err() {
                            break;
                        }
                    }
                    seen.pop();
                    r?
                }
                self.merge(off_a, off_b);
                Ok(())
            }
            (TypeInfo::Enum(a), TypeInfo::Enum(b)) => {
                if a.is_decl || b.is_decl {
                    self.merge(off_a, off_b);
                    return Ok(());
                }
                if a.size != b.size {
                    return Err(TypeError::SizeMismatch.into());
                }
                if a.enumerators.len() != b.enumerators.len() {
                    return Err(TypeError::MemberLengthMismatch.into());
                }
                for (i, (e_a, e_b)) in a.enumerators.iter().zip(b.enumerators.iter()).enumerate() {
                    if e_a != e_b {
                        return Err(TypeError::EnumeratorMismatch(i).into());
                    }
                }
                self.merge(off_a, off_b);
                Ok(())
            }
            (TypeInfo::Comp(TypeComp::Ptr(a)), TypeInfo::Comp(TypeComp::Ptr(b))) => {
                seen.push((*off_a, *off_b));
                let result = self.merge_recur(a, b, "Merging pointer inner type", seen);
                seen.pop();
                result?;
                self.merge(off_a, off_b);
                Ok(())
            }
            (
                TypeInfo::Comp(TypeComp::Array(a, a_len)),
                TypeInfo::Comp(TypeComp::Array(b, b_len)),
            ) => {
                if a_len != b_len {
                    return Err(report!(TypeError::ArrayLengthMismatch))
                        .attach_printable(format!("{} != {}", a_len, b_len));
                }
                seen.push((*off_a, *off_b));
                let result = self.merge_recur(a, b, "Merging array inner type", seen);
                seen.pop();
                if result.is_ok() {
                    self.merge(off_a, off_b);
                }
                result
            }
            (
                TypeInfo::Comp(TypeComp::Subroutine(s_a)),
                TypeInfo::Comp(TypeComp::Subroutine(s_b)),
            ) => {
                seen.push((*off_a, *off_b));
                let a = s_a.iter().collect::<Vec<_>>();
                let b = s_b.iter().collect::<Vec<_>>();
                if a.len() != b.len() {
                    return Err(report!(TypeError::SubroutineMismatch)).attach_printable(format!(
                        "Type lengths: {} != {}",
                        a.len(),
                        b.len()
                    ));
                }
                let mut r = Ok(());
                for (i, (a, b)) in a.iter().zip(b.iter()).enumerate() {
                    r = self.merge_recur(a, b, &format!("Merging subroutine type at {i}"), seen);
                    if r.is_err() {
                        break;
                    }
                }
                if r.is_ok() {
                    self.merge(off_a, off_b);
                }
                seen.pop();
                r
            }
            (
                TypeInfo::Comp(TypeComp::Ptmf(a_this, a_sub)),
                TypeInfo::Comp(TypeComp::Ptmf(b_this, b_sub)),
            ) => {
                seen.push((*off_a, *off_b));
                if self
                    .merge_recur(a_this, b_this, "Merging ptmf this type", seen)
                    .is_err()
                {
                    seen.pop();
                    return Err(TypeError::ThisTypeMismatch.into());
                }
                let a = a_sub.iter().collect::<Vec<_>>();
                let b = b_sub.iter().collect::<Vec<_>>();
                if a.len() != b.len() {
                    seen.pop();
                    return Err(report!(TypeError::SubroutineMismatch)).attach_printable(format!(
                        "Type lengths: {} != {}",
                        a.len(),
                        b.len()
                    ));
                }
                let mut r = Ok(());
                for (i, (a, b)) in a.iter().zip(b.iter()).enumerate() {
                    r = self.merge_recur(
                        a,
                        b,
                        &format!("Merging ptmf subroutine type at {i}"),
                        seen,
                    );
                    if r.is_err() {
                        break;
                    }
                }
                if r.is_ok() {
                    self.merge(off_a, off_b);
                }
                seen.pop();
                r
            }
            _ => Err(TypeError::Unrelated.into()),
        }
    }

    pub fn into_stage2(self) -> TypesStage2 {
        TypesStage2::new(self.off2info, self.buckets)
    }
}
