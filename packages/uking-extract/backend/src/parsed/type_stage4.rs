use std::collections::{BTreeMap, HashSet};

use error_stack::{report, Result, ResultExt};

use super::{Bucket, BucketType, Offset, TypeError, TypeInfo, TypeName, TypesStage5};

/// Stage 4 - Assign a unique name to each type bucket
pub struct TypesStage4 {
    off2info: BTreeMap<Offset, TypeInfo>,
    off2bkt: BTreeMap<Offset, Offset>,
    buckets: BTreeMap<Offset, Bucket>,
}

impl TypesStage4 {
    pub fn new(
        off2info: BTreeMap<Offset, TypeInfo>,
        off2bkt: BTreeMap<Offset, Offset>,
        buckets: BTreeMap<Offset, Bucket>,
    ) -> Self {
        Self {
            off2info,
            off2bkt,
            buckets,
        }
    }

    pub fn resolve_into_stage5(mut self) -> Result<TypesStage5, TypeError> {
        let names = self.resolve_names()?;
        Ok(TypesStage5::new(
            self.off2info,
            self.off2bkt,
            self.buckets,
            names,
        ))
    }

    fn resolve_names(&mut self) -> Result<BTreeMap<Offset, TypeName>, TypeError> {
        let mut base_names = self
            .buckets
            .iter()
            .filter_map(|(bkt, bucket)| {
                let mut names = match bucket.type_ {
                    BucketType::Comp => return None,
                    _ => bucket.names.iter().cloned().collect::<Vec<_>>(),
                };
                names.sort();
                Some((*bkt, names))
            })
            .collect::<BTreeMap<_, _>>();
        #[cfg(feature = "debug-resolve-name")]
        {
            println!("Initial names:");
            for (bkt, names) in &base_names {
                println!("{}:", bkt);
                for name in names {
                    println!("  {}", name);
                }
            }
        }
        loop {
            let mut seen_names = HashSet::new();
            let mut duplicate_names = HashSet::new();
            for (bkt, names) in &base_names {
                let best_name = match names.last() {
                    Some(name) => name.clone(),
                    None => {
                        return Err(report!(TypeError::NoBaseName))
                            .attach_printable_lazy(|| format!("For bucket {}", bkt));
                    }
                };
                if !seen_names.insert(best_name.clone()) {
                    duplicate_names.insert(best_name);
                }
            }
            if duplicate_names.is_empty() {
                break;
            }
            #[cfg(feature = "debug-resolve-name")]
            {
                println!(">>>>>>>>>>>>>>>");
                println!("Removing Duplicate names:");
                for name in &duplicate_names {
                    println!("  {}", name);
                }
            }
            for (bkt, names) in &mut base_names {
                let best_name = match names.last() {
                    Some(name) => name,
                    None => {
                        return Err(report!(TypeError::NoBaseName))
                            .attach_printable_lazy(|| format!("For bucket {}", bkt));
                    }
                };
                if duplicate_names.contains(best_name) {
                    names.pop();
                }
            }
        }
        let mut bkt2name = base_names
            .into_iter()
            .map(|(bkt, mut names)| {
                let name = names.pop().unwrap();
                (bkt, name)
            })
            .collect::<BTreeMap<_, _>>();
        #[cfg(feature = "debug-resolve-name")]
        {
            println!("Resolved base names:");
            for (bkt, name) in &bkt2name {
                println!("{}: {}", bkt, name);
            }
        }
        loop {
            let mut to_insert = Vec::new();
            for (bkt, bucket) in self
                .buckets
                .iter_mut()
                .filter(|(k, _)| !bkt2name.contains_key(k))
            {
                bucket.reduce_comp_name(&self.off2info, &self.off2bkt, &bkt2name);
                if !bucket.names.is_empty() {
                    let mut names = bucket.names.iter().cloned().collect::<Vec<_>>();
                    names.sort();
                    let best_name = names.pop().unwrap();
                    to_insert.push((*bkt, best_name));
                }
            }
            if to_insert.is_empty() {
                break;
            }
            for (bkt, name) in to_insert {
                bkt2name.insert(bkt, name);
            }
        }
        if bkt2name.len() != self.buckets.len() {
            #[cfg(feature = "debug-resolve-name")]
            {
                println!("Unresolved names:");
                for (bkt, bucket) in &self.buckets {
                    if !bkt2name.contains_key(bkt) {
                        println!("{}: {:?}", bkt, bucket.names);
                    }
                }
            }
            return Err(report!(TypeError::UnresolvedNames));
        }
        let seen_names = bkt2name.values().cloned().collect::<HashSet<_>>();
        if seen_names.len() != bkt2name.len() {
            #[cfg(feature = "debug-resolve-name")]
            {
                let mut seen_names = HashSet::new();
                println!("Duplicated names:");
                for (bkt, name) in &bkt2name {
                    if !seen_names.insert(name) {
                        println!("{}: {}", bkt, name);
                    }
                }
            }
            return Err(report!(TypeError::DuplicatedNames));
        }
        #[cfg(feature = "debug-resolve-name")]
        {
            println!("All Resolved names:");
            for (bkt, name) in &bkt2name {
                println!("{}: {}", bkt, name);
            }
        }
        Ok(bkt2name)
    }
}
