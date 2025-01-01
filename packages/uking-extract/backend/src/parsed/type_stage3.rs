use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use common::ProgressPrinter;

use crate::worker::Pool;

use super::{
    Bucket, BucketType, NamespaceMap, Offset, TypeComp, TypeInfo, TypePrim, TypesStage4, VtableInfo,
};

pub struct TypesStage3 {
    off2info: BTreeMap<Offset, TypeInfo>,
    off2bkt: BTreeMap<Offset, Offset>,
    buckets: BTreeMap<Offset, Bucket>,
    iteration: usize,
    progress: ProgressPrinter,
    fabrication: usize,
}

impl TypesStage3 {
    pub fn new(
        off2info: BTreeMap<Offset, TypeInfo>,
        off2bkt: BTreeMap<Offset, Offset>,
        buckets: BTreeMap<Offset, Vec<Offset>>,
    ) -> Self {
        let buckets = buckets
            .into_iter()
            .map(|(k, v)| (k, Bucket::new(v)))
            .collect();
        let progress = ProgressPrinter::new(0, "");
        Self {
            off2info,
            off2bkt,
            buckets,
            iteration: 1,
            progress,
            fabrication: usize::MAX - 1,
        }
    }

    pub fn merge_into_stage4(self, namespaces: &NamespaceMap) -> TypesStage4 {
        let s = Arc::new(RwLock::new(self));
        Self::merge_buckets(&s, namespaces);
        let mut s = Arc::into_inner(s).unwrap().into_inner().unwrap();
        s.progress.done();
        s.resolve_vtables();
        TypesStage4::new(s.off2info, s.off2bkt, s.buckets)
    }

    fn make_progress_message(&self) -> String {
        let mut prim_count = 0;
        let mut enum_count = 0;
        let mut struct_count = 0;
        let mut union_count = 0;
        let mut other_count = 0;
        let total = self.buckets.len();
        for b in self.buckets.values() {
            match b.type_ {
                BucketType::Prim => prim_count += 1,
                BucketType::Enum => enum_count += 1,
                BucketType::Struct => struct_count += 1,
                BucketType::Union => union_count += 1,
                _ => other_count += 1,
            }
        }
        format!(
            "Merge types (Iter.{}: p:{} e:{} s:{} u:{} o:{}, {} total)",
            self.iteration, prim_count, enum_count, struct_count, union_count, other_count, total
        )
    }

    fn merge_buckets(s: &Arc<RwLock<Self>>, namespaces: &NamespaceMap) {
        Self::merge_buckets_by_filter(s, namespaces, |_, v| v.type_ == BucketType::Prim);
        Self::merge_buckets_by_filter(s, namespaces, |_, v| v.type_ == BucketType::Enum);
        Self::merge_buckets_by_filter(s, namespaces, |_, v| v.type_ == BucketType::Struct);
        Self::merge_buckets_by_filter(s, namespaces, |_, v| v.type_ == BucketType::Union);
        Self::merge_buckets_by_filter(s, namespaces, |_, v| v.type_ == BucketType::Comp);
        Self::merge_buckets_by_filter(s, namespaces, |_, _| true);
    }

    fn merge_buckets_by_filter(
        s: &Arc<RwLock<Self>>,
        namespaces: &NamespaceMap,
        f: impl Fn(&Offset, &Bucket) -> bool,
    ) {
        let mut last_key_len = 0;
        loop {
            {
                let mut s = s.write().unwrap();
                s.reduce(namespaces);
            }
            let keys = {
                let mut s = s.write().unwrap();
                let prefix = s.make_progress_message();
                s.progress.set_prefix(&prefix);
                let keys = s
                    .buckets
                    .iter()
                    .filter_map(|(k, v)| if f(k, v) { Some(*k) } else { None })
                    .collect::<Vec<_>>();
                s.progress.set_total(keys.len());
                s.progress.reset_timer();
                s.progress.print(0, "");
                keys
            };
            if keys.len() == last_key_len {
                break;
            }
            last_key_len = keys.len();
            let s2 = Arc::clone(s);
            let input = InputIter::new(keys, 81920);
            let factor = input.factor();
            let input = input.map(move |x| (Arc::clone(&s2), x));
            let pool = Pool::run(input, move |(s, keys)| {
                let s = s.read().unwrap();
                let len = keys.len();
                (s.find_mergeable2(&keys), len)
            });
            let mut merge_sets = BTreeMap::new();
            let mut count = 0;
            for (m, l) in pool {
                count += l;
                {
                    let s = s.read().unwrap();
                    s.progress.print(count / factor, "");
                }
                for (k, v) in m {
                    let set = merge_sets.entry(k).or_insert_with(BTreeSet::new);
                    set.extend(v);
                }
            }
            {
                let mut s = s.write().unwrap();
                s.progress.set_total(merge_sets.len());
                s.progress.reset_timer();
                s.progress.print(0, "");
                for (i, (k, v)) in merge_sets.into_iter().enumerate() {
                    s.progress.print(i, "");
                    let v = v.into_iter().collect::<Vec<_>>();
                    s.merge_all(&k, &v);
                }
                s.iteration += 1;
            }
        }
    }

    fn find_mergeable2(&self, inputs: &Vec<(Offset, Offset)>) -> BTreeMap<Offset, Vec<Offset>> {
        let mut merge_sets = BTreeMap::<Offset, Vec<Offset>>::new();
        for (a, b) in inputs {
            let set = merge_sets.entry(*a).or_default();
            if self.can_merge_bucket(a, b) {
                set.push(*b);
            }
        }
        merge_sets
    }

    fn reduce(&mut self, namespaces: &NamespaceMap) {
        // reduce it once first to get primitive buckets reduced
        for (bucket_key, bucket) in &mut self.buckets {
            bucket.reduce(&self.off2info, namespaces);
            bucket.check(bucket_key, &self.off2info);
        }
        let mut merges = Vec::new();
        let mut modifications = Vec::new();
        let mut new_types = Vec::new();
        // simplify unions
        for (off, info) in &self.off2info {
            let mut info = match info {
                TypeInfo::Union(x) => x.clone(),
                _ => continue,
            };
            if info.is_decl {
                continue;
            }
            // has any anonymous member?
            if info.members.iter().any(|m| m.0.is_none()) {
                // remove non-anonymous members
                info.members.retain(|m| m.0.is_none());
            }
            ///// This optimization is unstable and somehow causes unions of different sizes to be
            ///// merged
            // // small pritimive simplification -
            // // simplify small unions with a primitive member to a primitive
            // // unsigned integer
            // if info.members.iter().any(|m| {
            //     let bkt = self.off2bkt.get(&m.1).unwrap();
            //     let bucket = self.buckets.get(bkt).unwrap();
            //     bucket.type_ == BucketType::Prim
            // }) {
            //     match info.size {
            //         1 => {
            //             let prim_info = TypeInfo::Prim(TypePrim::U8);
            //             let prim_offset = self.fabricate_offset();
            //             new_types.push(prim_offset);
            //             modifications.push((prim_offset, prim_info));
            //             merges.push((*off, prim_offset));
            //             #[cfg(feature = "debug-union-opt")]
            //             {
            //                 println!("Simplified union to U8: {:?}", off);
            //             }
            //             continue
            //         }
            //         2 => {
            //             let prim_info = TypeInfo::Prim(TypePrim::U16);
            //             let prim_offset = self.fabricate_offset();
            //             new_types.push(prim_offset);
            //             modifications.push((prim_offset, prim_info));
            //             merges.push((*off, prim_offset));
            //             #[cfg(feature = "debug-union-opt")]
            //             {
            //                 println!("Simplified union to U16: {:?}", off);
            //             }
            //             continue
            //         }
            //         4 => {
            //             let prim_info = TypeInfo::Prim(TypePrim::U32);
            //             let prim_offset = self.fabricate_offset();
            //             new_types.push(prim_offset);
            //             modifications.push((prim_offset, prim_info));
            //             merges.push((*off, prim_offset));
            //             #[cfg(feature = "debug-union-opt")]
            //             {
            //                 println!("Simplified union to U32: {:?}", off);
            //             }
            //             continue
            //         }
            //         8 => {
            //             let prim_info = TypeInfo::Prim(TypePrim::U64);
            //             let prim_offset = self.fabricate_offset();
            //             new_types.push(prim_offset);
            //             modifications.push((prim_offset, prim_info));
            //             merges.push((*off, prim_offset));
            //             #[cfg(feature = "debug-union-opt")]
            //             {
            //                 println!("Simplified union to U64: {:?}", off);
            //             }
            //             continue
            //         }
            //         _=>{}
            //     }
            // }
            // did we remove the largest member?
            if info.members.iter().all(|m| {
                let m_info = self.off2info.get(&m.1).unwrap();
                let size = m_info.slow_size(&self.off2info).unwrap_or(0);
                size != info.size
            }) {
                // reduce union to byte array
                let byte_info = TypeInfo::Prim(TypePrim::U8);
                let byte_offset = self.fabricate_offset();
                new_types.push(byte_offset);
                modifications.push((byte_offset, byte_info));
                let array_info = TypeInfo::Comp(TypeComp::Array(byte_offset, info.size));
                let array_offset = self.fabricate_offset();
                new_types.push(array_offset);
                modifications.push((array_offset, array_info));

                merges.push((*off, array_offset));
                continue;
            }
            // are all members the same type?
            let tbkt = info
                .members
                .iter()
                .map(|m| *self.off2bkt.get(&m.1).unwrap())
                .collect::<BTreeSet<_>>();
            if tbkt.len() == 1 {
                // merge into that type
                merges.push((*off, *tbkt.iter().next().unwrap()));
                continue;
            }
            modifications.push((*off, TypeInfo::Union(info)));
        }

        // apply new types
        for off in new_types {
            self.off2bkt.insert(off, off);
            self.buckets.insert(off, Bucket::new(vec![off]));
        }
        // apply modifications
        for (off, info) in modifications {
            self.off2info.insert(off, info);
        }
        // apply merges
        for (a, b) in merges {
            self.merge_all(&a, &[b]);
        }
        self.reset_fabrication();
        // reduce again to process the new merges
        for (bucket_key, bucket) in &mut self.buckets {
            bucket.reduce(&self.off2info, namespaces);
            bucket.check(bucket_key, &self.off2info);
        }
    }

    fn fabricate_offset(&self) -> Offset {
        let mut off = self.fabrication;
        while self.off2info.contains_key(&off.into()) {
            off -= 1;
        }
        off.into()
    }

    fn reset_fabrication(&mut self) {
        self.fabrication = self.fabricate_offset().into();
    }

    fn can_merge_bucket(&self, bkt_a: &Offset, bkt_b: &Offset) -> bool {
        self.can_merge_bucket_internal(bkt_a, bkt_b, &mut Vec::new())
    }
    fn can_merge_bucket_internal(
        &self,
        bkt_a: &Offset,
        bkt_b: &Offset,
        seen: &mut Vec<(Offset, Offset)>,
    ) -> bool {
        let bucket_a = self.buckets.get(bkt_a).unwrap();
        let bucket_b = self.buckets.get(bkt_b).unwrap();
        match (&bucket_a.type_, &bucket_b.type_) {
            (BucketType::Prim, BucketType::Prim) => {
                let a = bucket_a.candidates[0];
                let a = self.off2info.get(&a).unwrap();
                let b = bucket_b.candidates[0];
                let b = self.off2info.get(&b).unwrap();
                match (a, b) {
                    (TypeInfo::Prim(a), TypeInfo::Prim(b)) => a == b,
                    _ => false,
                }
            }
            (BucketType::Enum, BucketType::Enum) => {
                for a in &bucket_a.candidates {
                    let info_a = self.off2info.get(a).unwrap();
                    for b in &bucket_b.candidates {
                        let info_b = self.off2info.get(b).unwrap();
                        match (info_a, info_b) {
                            (TypeInfo::Enum(a), TypeInfo::Enum(b)) => {
                                if a.is_decl || b.is_decl {
                                    match (&a.name, &b.name) {
                                        (Some(a), Some(b)) => {
                                            if a == b {
                                                return true;
                                            }
                                        }
                                        _ => continue,
                                    }
                                }
                                if a.name != b.name {
                                    continue;
                                }
                                if a.size != b.size {
                                    continue;
                                }
                                if a.enumerators.len() != b.enumerators.len() {
                                    continue;
                                }
                                if a.enumerators.is_empty() {
                                    if a.name != b.name {
                                        continue;
                                    }
                                }
                                return true;
                            }
                            _ => continue,
                        }
                    }
                }
                false
            }
            (BucketType::Struct, BucketType::Struct) => {
                for a in &bucket_a.candidates {
                    let info_a = self.off2info.get(a).unwrap();
                    for b in &bucket_b.candidates {
                        let info_b = self.off2info.get(b).unwrap();
                        match (info_a, info_b) {
                            (TypeInfo::Struct(a), TypeInfo::Struct(b)) => {
                                if a.is_decl || b.is_decl {
                                    match (&a.name, &b.name) {
                                        (Some(a), Some(b)) => {
                                            if a == b {
                                                return true;
                                            }
                                        }
                                        _ => continue,
                                    }
                                }
                                if a.name != b.name {
                                    continue;
                                }
                                if a.size != b.size {
                                    continue;
                                }
                                if a.members.len() != b.members.len() {
                                    continue;
                                }
                                if a.members.is_empty() {
                                    if a.name != b.name {
                                        continue;
                                    }
                                }
                                if !a.vtable.is_equiv_to(&b.vtable) {
                                    continue;
                                }
                                for (m_a, m_b) in a.members.iter().zip(b.members.iter()) {
                                    if m_a.name != m_b.name {
                                        continue;
                                    }
                                    if m_a.is_base != m_b.is_base {
                                        continue;
                                    }
                                    if m_a.offset != m_b.offset {
                                        continue;
                                    }
                                }
                                let already_recursing = seen.iter().any(|(a, b)| {
                                    (*a == *bkt_a && *b == *bkt_b) || (*a == *bkt_b && *b == *bkt_a)
                                });
                                if !already_recursing {
                                    seen.push((*bkt_a, *bkt_b));
                                    let mut r = true;
                                    for (m_a, m_b) in a.members.iter().zip(b.members.iter()) {
                                        let bkt_a = self.off2bkt.get(&m_a.ty_offset).unwrap();
                                        let bkt_b = self.off2bkt.get(&m_b.ty_offset).unwrap();
                                        if !self.can_merge_bucket_internal(bkt_a, bkt_b, seen) {
                                            r = false;
                                            break;
                                        }
                                    }
                                    seen.pop();
                                    if !r {
                                        continue;
                                    }
                                }
                                return true;
                            }
                            _ => continue,
                        }
                    }
                }
                false
            }
            (BucketType::Union, BucketType::Union) => {
                // let mut is_a_all_decl = true;
                // let mut has_name_match = false;
                for a in &bucket_a.candidates {
                    let info_a = self.off2info.get(a).unwrap();
                    for b in &bucket_b.candidates {
                        let info_b = self.off2info.get(b).unwrap();
                        match (info_a, info_b) {
                            (TypeInfo::Union(a), TypeInfo::Union(b)) => {
                                if a.is_decl || b.is_decl {
                                    match (&a.name, &b.name) {
                                        (Some(a), Some(b)) => {
                                            if a == b {
                                                return true;
                                            }
                                        }
                                        _ => continue,
                                    }
                                }
                                if a.name != b.name {
                                    continue;
                                }
                                if a.size != b.size {
                                    continue;
                                }
                                if a.members.len() != b.members.len() {
                                    continue;
                                }
                                if a.members.is_empty() {
                                    if a.name != b.name {
                                        continue;
                                    }
                                }
                                for (m_a, m_b) in a.members.iter().zip(b.members.iter()) {
                                    if m_a.0 != m_b.0 {
                                        continue;
                                    }
                                }
                                let already_recursing = seen.iter().any(|(a, b)| {
                                    (*a == *bkt_a && *b == *bkt_b) || (*a == *bkt_b && *b == *bkt_a)
                                });
                                if !already_recursing {
                                    seen.push((*bkt_a, *bkt_b));
                                    let mut r = true;
                                    for (m_a, m_b) in a.members.iter().zip(b.members.iter()) {
                                        let bkt_a = self.off2bkt.get(&m_a.1).unwrap();
                                        let bkt_b = self.off2bkt.get(&m_b.1).unwrap();
                                        if !self.can_merge_bucket_internal(bkt_a, bkt_b, seen) {
                                            r = false;
                                            break;
                                        }
                                    }
                                    seen.pop();
                                    if !r {
                                        continue;
                                    }
                                }
                                return true;
                            }
                            _ => continue,
                        }
                    }
                }
                false
            }
            (BucketType::Comp, BucketType::Comp) => {
                for a in &bucket_a.candidates {
                    let info_a = self.off2info.get(a).unwrap();
                    for b in &bucket_b.candidates {
                        let info_b = self.off2info.get(b).unwrap();
                        match (info_a, info_b) {
                            (
                                TypeInfo::Comp(TypeComp::Ptr(a)),
                                TypeInfo::Comp(TypeComp::Ptr(b)),
                            ) => {
                                seen.push((*bkt_a, *bkt_b));
                                let bkt_a = self.off2bkt.get(a).unwrap();
                                let bkt_b = self.off2bkt.get(b).unwrap();
                                let r = self.can_merge_bucket_internal(bkt_a, bkt_b, seen);
                                seen.pop();
                                if r {
                                    return true;
                                }
                            }
                            (
                                TypeInfo::Comp(TypeComp::Array(a, len_a)),
                                TypeInfo::Comp(TypeComp::Array(b, len_b)),
                            ) => {
                                if len_a != len_b {
                                    continue;
                                }
                                seen.push((*bkt_a, *bkt_b));
                                let bkt_a = self.off2bkt.get(a).unwrap();
                                let bkt_b = self.off2bkt.get(b).unwrap();
                                let r = self.can_merge_bucket_internal(bkt_a, bkt_b, seen);
                                seen.pop();
                                if r {
                                    return true;
                                }
                            }
                            (
                                TypeInfo::Comp(TypeComp::Subroutine(a)),
                                TypeInfo::Comp(TypeComp::Subroutine(b)),
                            ) => {
                                let a = a.iter().copied().collect::<Vec<_>>();
                                let b = b.iter().copied().collect::<Vec<_>>();
                                if a.len() != b.len() {
                                    continue;
                                }
                                seen.push((*bkt_a, *bkt_b));
                                let mut r = true;
                                for (a, b) in a.iter().zip(b.iter()) {
                                    let bkt_a = self.off2bkt.get(a).unwrap();
                                    let bkt_b = self.off2bkt.get(b).unwrap();
                                    if !self.can_merge_bucket_internal(bkt_a, bkt_b, seen) {
                                        r = false;
                                        break;
                                    }
                                }
                                seen.pop();
                                if r {
                                    return true;
                                }
                            }
                            (
                                TypeInfo::Comp(TypeComp::Ptmf(this_a, a)),
                                TypeInfo::Comp(TypeComp::Ptmf(this_b, b)),
                            ) => {
                                seen.push((*bkt_a, *bkt_b));
                                let bkt_a = self.off2bkt.get(this_a).unwrap();
                                let bkt_b = self.off2bkt.get(this_b).unwrap();
                                let r = self.can_merge_bucket_internal(bkt_a, bkt_b, seen);
                                if !r {
                                    seen.pop();
                                    continue;
                                }
                                let a = a.iter().copied().collect::<Vec<_>>();
                                let b = b.iter().copied().collect::<Vec<_>>();
                                if a.len() != b.len() {
                                    seen.pop();
                                    continue;
                                }
                                let mut r = true;
                                for (a, b) in a.iter().zip(b.iter()) {
                                    let bkt_a = self.off2bkt.get(a).unwrap();
                                    let bkt_b = self.off2bkt.get(b).unwrap();
                                    if !self.can_merge_bucket_internal(bkt_a, bkt_b, seen) {
                                        r = false;
                                        break;
                                    }
                                }
                                seen.pop();
                                if r {
                                    return true;
                                }
                            }
                            _ => continue,
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Merge the all buckets containing b into the bucket containing a
    fn merge_all(&mut self, off_a: &Offset, off_b: &[Offset]) {
        let bkt_a = *self.off2bkt.get(off_a).unwrap();
        let mut to_change = Vec::new();
        for b in off_b {
            let bkt_b = self.off2bkt.get(b).unwrap();
            if *bkt_b == bkt_a {
                // shouldn't happen, just safe guard
                continue;
            }
            if let Some(bucket_b) = self.buckets.remove(bkt_b) {
                to_change.extend(bucket_b.original_candidates);
            }
        }
        for off_b in &to_change {
            self.off2bkt.insert(*off_b, bkt_a);
        }
        let bkt_a = self.buckets.get_mut(&bkt_a).unwrap();
        bkt_a.candidates.extend(to_change.clone());
        bkt_a.original_candidates.extend(to_change);
    }

    fn resolve_vtables(&mut self) {
        let progress = ProgressPrinter::new(self.buckets.len(), "Resolve vtables");
        for (i, bucket) in self.buckets.values().enumerate() {
            progress.print(i, "");
            let mut vtable = VtableInfo::default();
            for off in &bucket.original_candidates {
                let info = self.off2info.get(off).unwrap();
                if let TypeInfo::Struct(s) = info {
                    vtable.merge(&s.vtable);
                }
            }
            for off in &bucket.original_candidates {
                let info = self.off2info.get_mut(off).unwrap();
                if let TypeInfo::Struct(s) = info {
                    s.vtable = vtable.clone();
                }
            }
        }
        progress.done();
    }
}

/// Input iterator for merging buckets
struct InputIter {
    keys: Vec<Offset>,
    a: usize,
    b: usize,
    chunk_size: usize,
}
impl InputIter {
    fn new(keys: Vec<Offset>, chunk_size: usize) -> Self {
        Self {
            keys,
            a: 0,
            b: 1,
            chunk_size,
        }
    }
    /// Factor when multiplied by to get the total number of combinations
    fn factor(&self) -> usize {
        match self.keys.len() {
            0 => 1,
            n => (n - 1) / 2,
        }
    }
    fn next_single(&mut self) -> Option<(Offset, Offset)> {
        let len = self.keys.len();
        self.b += 1;
        if self.b >= len {
            self.a += 1;
            self.b = self.a + 1;
        }
        if self.a >= len || self.b >= len {
            return None;
        }
        Some((self.keys[self.a], self.keys[self.b]))
    }
}
impl Iterator for InputIter {
    type Item = Vec<(Offset, Offset)>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut v = Vec::with_capacity(self.chunk_size);
        for _ in 0..self.chunk_size {
            match self.next_single() {
                Some(x) => v.push(x),
                None => break,
            }
        }
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    }
}
