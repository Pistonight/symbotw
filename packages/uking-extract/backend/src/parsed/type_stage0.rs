use std::collections::BTreeMap;

use derivative::Derivative;

use super::{Offset, TypeInfo, TypesStage1};

/// Stage 0 type resolver
///
/// In this stage, initial types are merged based on information
/// during parsing types. Including equipvalent types discovered
/// purely based on type information, and typedefs
#[derive(Debug, Derivative)]
#[derivative(Default(bound = "", new = "true"))]
pub struct TypesStage0 {
    off2info: BTreeMap<Offset, TypeInfo>,
    merges: Vec<(Offset, Offset)>,
}

impl TypesStage0 {
    pub fn into_stage1(self) -> TypesStage1 {
        let mut typedefs = Vec::new();
        for (offset, ty) in &self.off2info {
            if let TypeInfo::Typedef(_, ty_offset) = ty {
                typedefs.push((*ty_offset, *offset));
            }
        }

        let off2bkt = self.off2info.keys().map(|k| (*k, *k)).collect();
        let buckets = self.off2info.keys().map(|k| (*k, vec![*k])).collect();

        let mut stage1 = TypesStage1::new(self.off2info, off2bkt, buckets);
        // initial merges
        for (a, b) in self.merges {
            #[cfg(feature = "debug-merge")]
            {
                if a == super::DEBUG_MERGE_OFFSET || b == super::DEBUG_MERGE_OFFSET {
                    println!("Merge Stage 0: {} {}", a, b);
                }
            }
            stage1.merge(&a, &b);
        }

        for (a, b) in typedefs {
            #[cfg(feature = "debug-merge")]
            {
                if a == super::DEBUG_MERGE_OFFSET || b == super::DEBUG_MERGE_OFFSET {
                    println!("Merge Stage 0: {} {}", a, b);
                }
            }
            stage1.merge(&a, &b);
        }

        stage1
    }

    #[inline]
    pub fn add_merge(&mut self, a: impl Into<Offset>, b: impl Into<Offset>) {
        self.merges.push((a.into(), b.into()));
    }

    #[inline]
    pub fn get(&self, offset: &Offset) -> Option<&TypeInfo> {
        self.off2info.get(offset)
    }

    #[inline]
    pub fn insert(&mut self, offset: impl Into<Offset>, info: TypeInfo) {
        self.off2info.insert(offset.into(), info);
    }
}
