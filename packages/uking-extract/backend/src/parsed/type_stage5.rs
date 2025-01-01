use std::collections::BTreeMap;

use error_stack::Result;

use super::{Bucket, BucketSize, Offset, TypeError, TypeInfo, TypeName, TypesStage6};

/// Stage 5 - Resolve the size for each type bucket
pub struct TypesStage5 {
    off2info: BTreeMap<Offset, TypeInfo>,
    off2bkt: BTreeMap<Offset, Offset>,
    buckets: BTreeMap<Offset, Bucket>,
    bkt2name: BTreeMap<Offset, TypeName>,
}

impl TypesStage5 {
    pub fn new(
        off2info: BTreeMap<Offset, TypeInfo>,
        off2bkt: BTreeMap<Offset, Offset>,
        buckets: BTreeMap<Offset, Bucket>,
        names: BTreeMap<Offset, TypeName>,
    ) -> Self {
        Self {
            off2info,
            off2bkt,
            buckets,
            bkt2name: names,
        }
    }
    pub fn resolve_into_stage6(self) -> Result<TypesStage6, TypeError> {
        let sizes = self.resolve_sizes()?;
        Ok(TypesStage6::new(
            self.off2info,
            self.off2bkt,
            self.buckets,
            self.bkt2name,
            sizes,
        ))
    }
    fn resolve_sizes(&self) -> Result<BTreeMap<Offset, Option<usize>>, TypeError> {
        let mut bkt2size = BTreeMap::new();
        #[cfg(feature = "debug-resolve-size")]
        {
            println!("Resolving sizes");
        }
        loop {
            let last_len = bkt2size.len();
            for (bkt, bucket) in &self.buckets {
                if bkt2size.contains_key(bkt) {
                    continue;
                }
                let size =
                    bucket.get_size(&self.off2info, &self.off2bkt, &self.bkt2name, &bkt2size)?;
                if let BucketSize::Size(size) = size {
                    #[cfg(feature = "debug-resolve-size")]
                    {
                        println!("  {}: {:?}", bkt, size);
                    }
                    bkt2size.insert(*bkt, size);
                }
            }
            if last_len == bkt2size.len() {
                break;
            }
        }
        for bkt in self.buckets.keys() {
            if !bkt2size.contains_key(bkt) {
                bkt2size.insert(*bkt, None);
            }
        }
        #[cfg(feature = "debug-resolve-size")]
        {
            if bkt2size.len() != self.buckets.len() {
                println!("Unresolved sizes:");
                for (bkt, bucket) in &self.buckets {
                    if !bkt2size.contains_key(bkt) {
                        println!("  {}: {:?}", bkt, bucket.names);
                    }
                }
            }
        }
        Ok(bkt2size)
    }
}
