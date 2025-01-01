use std::collections::{BTreeMap, BTreeSet};

use super::{Offset, TypeComp, TypeInfo, TypesStage3};

/// Garbage collector for types.
pub struct TypesStage2 {
    off2info: BTreeMap<Offset, TypeInfo>,
    buckets: BTreeMap<Offset, Vec<Offset>>,
    marked_offs: BTreeSet<Offset>,
}
impl TypesStage2 {
    pub fn new(
        off2info: BTreeMap<Offset, TypeInfo>,
        buckets: BTreeMap<Offset, Vec<Offset>>,
    ) -> Self {
        Self {
            off2info,
            buckets,
            marked_offs: BTreeSet::new(),
        }
    }
    pub fn mark(&mut self, offset: &Offset) {
        self.mark_recur(offset)
    }

    fn mark_recur(&mut self, off: &Offset) {
        if !self.marked_offs.insert(*off) {
            return;
        }
        self.mark_recur_internal(off);
    }

    fn mark_recur_internal(&mut self, offset: &Offset) {
        let ty = self.off2info.get(offset).unwrap();
        match ty {
            TypeInfo::Prim(_) => {}
            TypeInfo::Enum(_) => {}
            TypeInfo::Typedef(_, t) => {
                let t = *t;
                self.mark_recur(&t)
            }
            TypeInfo::Struct(s) => {
                let refed = s
                    .members
                    .iter()
                    .map(|m| m.ty_offset)
                    .chain(s.vtable.iter().flat_map(|x| x.function.iter().copied()))
                    .collect::<Vec<_>>();
                for t in refed {
                    self.mark_recur(&t)
                }
            }
            TypeInfo::Union(u) => {
                for t in u.members.iter().map(|m| m.1).collect::<Vec<_>>() {
                    self.mark_recur(&t)
                }
            }
            TypeInfo::Comp(TypeComp::Ptr(t)) => {
                let t = *t;
                self.mark_recur(&t)
            }
            TypeInfo::Comp(TypeComp::Array(t, _)) => {
                let t = *t;
                self.mark_recur(&t)
            }
            TypeInfo::Comp(TypeComp::Subroutine(sub)) => {
                for t in sub.iter().copied().collect::<Vec<_>>() {
                    self.mark_recur(&t)
                }
            }
            TypeInfo::Comp(TypeComp::Ptmf(this_ty, sub)) => {
                let this_ty = *this_ty;
                for t in sub.iter().copied().collect::<Vec<_>>() {
                    self.mark_recur(&t)
                }
                self.mark_recur(&this_ty);
            }
        }
    }

    /// Remove types not marked and create stage 3 resolver
    pub fn sweep_into_stage3(mut self) -> TypesStage3 {
        let marked_offs = self.marked_offs;
        self.off2info.retain(|k, _| marked_offs.contains(k));
        self.buckets.retain(|_, v| {
            v.retain(|x| marked_offs.contains(x));
            !v.is_empty()
        });
        let buckets: BTreeMap<Offset, Vec<Offset>> =
            self.buckets.into_values().map(|x| (x[0], x)).collect();
        let mut off2bkt = BTreeMap::<Offset, Offset>::new();
        for (bkt, offs) in &buckets {
            for off in offs {
                off2bkt.insert(*off, *bkt);
            }
        }

        let off2info_keys = self.off2info.keys().copied().collect::<BTreeSet<_>>();
        let off2bkt_keys = off2bkt.keys().copied().collect::<BTreeSet<_>>();
        if off2info_keys != off2bkt_keys {
            panic!("off2info and off2bkt keys are not equal");
        }

        TypesStage3::new(self.off2info, off2bkt, buckets)
    }
}
