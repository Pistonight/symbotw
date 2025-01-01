use std::collections::{BTreeMap, BTreeSet};

use error_stack::{report, Result, ResultExt};

use common::ProgressPrinter;

use super::{
    Bucket, BucketType, EnumDef, EnumInfo, MemberDef, Offset, StructDef, StructInfo, TypeComp,
    TypeDef, TypeError, TypeInfo, TypeName, TypePrim, TypeYaml, UnionDef, UnionInfo, VtableInfo,
};
pub struct TypesStage6 {
    off2info: BTreeMap<Offset, TypeInfo>,
    off2bkt: BTreeMap<Offset, Offset>,
    buckets: BTreeMap<Offset, Bucket>,
    bkt2name: BTreeMap<Offset, TypeName>,
    bkt2size: BTreeMap<Offset, Option<usize>>,
    referenced_names: NameRefMap,
}

impl TypesStage6 {
    pub fn new(
        off2info: BTreeMap<Offset, TypeInfo>,
        off2bkt: BTreeMap<Offset, Offset>,
        buckets: BTreeMap<Offset, Bucket>,
        names: BTreeMap<Offset, TypeName>,
        sizes: BTreeMap<Offset, Option<usize>>,
    ) -> Self {
        Self {
            off2info,
            off2bkt,
            buckets,
            bkt2name: names,
            bkt2size: sizes,
            referenced_names: NameRefMap::default(),
        }
    }

    pub fn get_name(&self, off: &Offset) -> &TypeName {
        let bkt = self.off2bkt.get(off).unwrap();
        self.bkt2name.get(bkt).unwrap()
    }

    pub fn create_defs(&mut self) -> Result<BTreeMap<String, TypeDef>, TypeError> {
        let progress = ProgressPrinter::new(self.buckets.len(), "Create type definitions");
        let mut referenced_names = NameRefMap::default();
        let mut name_to_def = BTreeMap::new();
        for (i, bkt) in self.buckets.keys().enumerate() {
            let name = self.bkt2name.get(bkt).unwrap();
            progress.print(i, name);
            self.create_def_for_bucket(bkt, &mut name_to_def, &mut referenced_names)
                .attach_printable_lazy(|| format!("While creating definition for type {}", name))?;
        }
        progress.done();
        // check if all referenced names have definitions
        for (x, referers) in &referenced_names.0 {
            if !name_to_def.contains_key(x) {
                return Err(report!(TypeError::BrokenTypeRef(x.clone())))
                    .attach_printable(format!("Referenced by {:?}", referers));
            }
        }
        self.referenced_names = referenced_names;
        Ok(name_to_def)
    }

    pub fn mark_referenced(&mut self, referr: &str, refed: &Offset) {
        let refed = self.get_name(refed).clone();
        self.referenced_names.add(referr, &refed);
    }

    pub fn check_reference(&self, types: &BTreeMap<String, TypeDef>) -> Result<(), TypeError> {
        // check if all referenced names have definitions
        for (x, referers) in &self.referenced_names.0 {
            if !types.contains_key(x) {
                return Err(report!(TypeError::BrokenTypeRef(x.clone())))
                    .attach_printable(format!("Referenced by {:?}", referers));
            }
        }
        Ok(())
    }

    pub fn check_and_gc_types(
        &mut self,
        types: &mut BTreeMap<String, TypeDef>,
    ) -> Result<(), TypeError> {
        for name in types.keys().cloned().collect::<Vec<_>>() {
            if !self.referenced_names.0.contains_key(&name) {
                types.remove(&name);
            }
        }
        self.check_reference(types)?;
        Ok(())
    }

    fn create_def_for_bucket<'a>(
        &self,
        bucket: &Offset,
        name2def: &'a mut BTreeMap<String, TypeDef>,
        referenced_names: &mut NameRefMap,
    ) -> Result<Option<&'a TypeDef>, TypeError> {
        let info_def = self.get_info_for_def(bucket)?;
        let (name, info) = match info_def {
            Some(x) => x,
            None => return Ok(None),
        };
        #[cfg(feature = "debug-create-def")]
        {
            println!("Creating def for {}", name);
        }
        let key = name.clone();
        let def = match info {
            TypeInfo::Enum(x) => {
                if x.is_decl {
                    return Ok(None);
                }
                TypeDef::Enum(EnumDef {
                    name,
                    size: x.size,
                    enumerators: x.enumerators.clone(),
                })
            }
            TypeInfo::Struct(x) => {
                if x.is_decl {
                    return Ok(None);
                }
                TypeDef::Struct(self.create_struct_def(x, &name, name2def, referenced_names)?)
            }
            TypeInfo::Union(x) => {
                if x.is_decl {
                    return Ok(None);
                }
                let mut size = 0;
                let mut members: Vec<(String, String)> = Vec::with_capacity(x.members.len());
                for (i, (name, offset)) in x.members.into_iter().enumerate() {
                    let mut name = name.unwrap_or_else(|| format!("_{}", i));
                    while members.iter().any(|x| x.0 == name) {
                        name.push('_');
                    }
                    let bkt = self.off2bkt.get(&offset).unwrap();
                    let member_size = self.bkt2size.get(bkt).unwrap()
                        .ok_or(report!(TypeError::InvalidLayout))
                        .attach_printable_lazy(|| {
                            format!("While resolving union member `{name}`, cannot find size for type {}", offset)
                        })?;
                    size = size.max(member_size);
                    let ty_name = self.bkt2name.get(bkt).unwrap();
                    referenced_names.add(&key, ty_name);
                    let ty_yaml = ty_name.yaml_string();
                    members.push((name, ty_yaml))
                }
                if x.size < size {
                    return Err(report!(TypeError::InvalidLayout).attach_printable(format!(
                        "Union `{name}` has size 0x{:x} smaller than its largest member 0x{:x}",
                        x.size, size
                    )));
                }
                let tail_padding = x.size - size;
                let alignment = if tail_padding == 0 {
                    1
                } else {
                    2usize.pow(tail_padding.ilog2() + 1)
                };
                TypeDef::Union(UnionDef {
                    name,
                    size: x.size,
                    alignment,
                    members,
                })
            }
            TypeInfo::Comp(c) => {
                if let TypeComp::Ptmf(_, _) = c {
                    // PTMF actually doesn't need the original type
                    TypeDef::ptmf(name)
                } else {
                    unreachable!()
                }
            }
            _ => unreachable!(),
        };
        if let Some(old) = name2def.get(&key) {
            if old != &def {
                return Err(report!(TypeError::ConflictingType))
                    .attach_printable(format!("Old: {:?}", old))
                    .attach_printable(format!("New: {:?}", def));
            }
        } else {
            name2def.insert(key.clone(), def);
        }
        return Ok(name2def.get(&key));
    }

    /// Get the type info for creating def for the bucket
    ///
    /// Return value:
    /// - None - don't create a def for this bucket
    /// - Some(name, info) - create a def with the given name
    fn get_info_for_def(&self, bkt: &Offset) -> Result<Option<(String, TypeInfo)>, TypeError> {
        let name = self.bkt2name.get(bkt).unwrap();
        let name = match name {
            // don't create definitions for primitive types
            TypeName::Prim(_) => return Ok(None),
            TypeName::Name(n) => n.clone(),
            TypeName::Comp(c) => {
                // need to create definition to PTMFs as structs
                if let TypeComp::Ptmf(this, _) = c.as_ref() {
                    if let TypeName::Name(n) = this {
                        format!("{}_ptmf", n)
                    } else {
                        return Err(report!(TypeError::UnexpectedUnnamedPtmfThis))
                            .attach_printable(format!("PTMF has base type: {}", this))
                            .attach_printable(format!("PTMF: {:?}", c))
                            .attach_printable(format!("Bucket: {}", bkt))
                            .attach_printable(format!("Name: {}", name));
                    }
                } else {
                    // don't create definitions for pointers, arrays, subroutines
                    return Ok(None);
                }
            }
        };
        let bucket = self.buckets.get(bkt).unwrap();
        if bucket.type_ == BucketType::Prim {
            return Ok(None);
        }
        for off in &bucket.original_candidates {
            let info = self.off2info.get(off).unwrap();
            match bucket.type_ {
                BucketType::Enum => {
                    if let TypeInfo::Enum(x) = info {
                        if !x.is_decl {
                            return Ok(Some((name, info.clone())));
                        }
                    }
                }
                BucketType::Struct => {
                    if let TypeInfo::Struct(x) = info {
                        if !x.is_decl {
                            return Ok(Some((name, info.clone())));
                        }
                    }
                }
                BucketType::Union => {
                    if let TypeInfo::Union(x) = info {
                        if !x.is_decl {
                            return Ok(Some((name, info.clone())));
                        }
                    }
                }
                BucketType::Comp => {
                    if let TypeInfo::Comp(TypeComp::Ptmf(_, _)) = info {
                        return Ok(Some((name, info.clone())));
                    }
                }
                _ => continue,
            }
        }

        // if a bucket doesn't have a definition, create a dummy one
        let info = match bucket.type_ {
            BucketType::Enum => TypeInfo::Enum(EnumInfo {
                name: None,
                size: 0,
                is_decl: false,
                enumerators: Vec::new(),
            }),
            BucketType::Struct => TypeInfo::Struct(StructInfo {
                name: None,
                size: 0,
                is_decl: false,
                members: Vec::new(),
                vtable: VtableInfo::default(),
            }),
            BucketType::Union => TypeInfo::Union(UnionInfo {
                name: None,
                size: 0,
                is_decl: false,
                members: Vec::new(),
            }),
            _ => unreachable!(),
        };
        Ok(Some((name, info)))
    }

    /// Create a struct layout after tail padding and empty base optimization,
    /// and ensure member name uniqueness
    fn create_struct_def(
        &self,
        info: StructInfo,
        name: &str,
        name2def: &mut BTreeMap<String, TypeDef>,
        referenced_names: &mut NameRefMap,
    ) -> Result<StructDef, TypeError> {
        // if there is no vtable only one member and it's a struct, inline that struct's member
        if info.vtable.is_empty() && info.members.len() == 1 {
            let member = &info.members[0];
            let member_bkt = self.off2bkt.get(&member.ty_offset).unwrap();
            let member_bucket = self.buckets.get(member_bkt).unwrap();
            if member_bucket.type_ == BucketType::Struct {
                if let Some(TypeDef::Struct(def)) =
                    self.create_def_for_bucket(member_bkt, name2def, referenced_names)?
                {
                    return Ok(StructDef {
                        name: name.to_string(),
                        vtable: Vec::new(),
                        size: info.size,
                        alignment: def.alignment,
                        members: def.members.clone(),
                    });
                }
            }
        }

        // size of the members in this struct's layout, calculated from offsets
        let mut member_sizes: Vec<usize> = Vec::with_capacity(info.members.len());
        for i in 0..info.members.len() {
            let offset = info.members[i].offset;
            let next_offset = if i < info.members.len() - 1 {
                info.members[i + 1].offset
            } else {
                info.size
            };
            member_sizes.push(next_offset - offset);
        }

        // calculate tail padding
        let alignment = if let Some(member) = info.members.last() {
            let bkt = self.off2bkt.get(&member.ty_offset).unwrap();
            let size = self
                .bkt2size
                .get(bkt)
                .unwrap()
                .ok_or(report!(TypeError::InvalidLayout))
                .attach_printable_lazy(|| {
                    format!(
                        "While resolving tail padding for `{name}`, cannot find size for type {}",
                        member.ty_offset
                    )
                })?;
            let space = *member_sizes.last().unwrap();
            if space < size {
                let r = Err(report!(TypeError::InvalidLayout).attach_printable(format!(
                    "Tail padding for member `{:?}` is negative: space=0x{:x}, size=0x{:x}",
                    member.name, space, size
                )));
                return r;
            }
            let tail_padding = space - size;
            if tail_padding == 0 {
                1
            } else {
                2usize.pow(tail_padding.ilog2() + 1)
            }
        } else {
            1
        };

        // MEMBERS ====
        let mut members: Vec<MemberDef> = Vec::new();
        for (mut m, m_size) in info.members.into_iter().zip(member_sizes.into_iter()) {
            let mut m_name = m.name.unwrap_or_else(|| format!("field_{:x}", m.offset));
            // replace vfptr with IDA standard
            let is_vfptr = if m_name.starts_with("_vptr$") {
                m_name = "__vftable".to_string();
                true
            } else {
                false
            };
            while members.iter().any(|x| x.name == m_name) {
                m_name.push('_');
            }
            let bkt = self.off2bkt.get(&m.ty_offset).unwrap();

            if m.is_base {
                if m_size == 0 {
                    #[cfg(feature = "debug-layout")]
                    {
                        println!("struct {}, base member `{}`, empty base", name, m_name);
                    }
                    // empty base optimization - base is removed
                    continue;
                }
                let base_size = self.bkt2size.get(bkt).unwrap()
                    .ok_or(report!(TypeError::InvalidLayout))
                    .attach_printable_lazy(|| {
                        format!("While resolving base member `{m_name}`, cannot find size for base type {}", m.ty_offset)
                    })?;
                if base_size > m_size {
                    #[cfg(feature = "debug-layout")]
                    {
                        println!("struct {}, base member `{}`, tail-padding", name, m_name);
                    }
                    // tail padding optimization
                    // expand the base members into the struct
                    let base_def = self.create_def_for_bucket(bkt, name2def, referenced_names)?;
                    let base_def = base_def
                        .ok_or(report!(TypeError::InvalidLayout))
                        .attach_printable(format!("While resolving tail padding optimization for base member, cannot find base definition for member `{m_name}"))?;
                    let base_def = match base_def {
                        TypeDef::Struct(x) => x,
                        _ => {
                            let r = report!(TypeError::InvalidLayout)
                            .attach_printable(format!("Base Member `{m_name}` is not a struct. Currently unions are not supported as base type"));
                            return Err(r);
                        }
                    };
                    for base_member in &base_def.members {
                        // it's probably fine to use base member's name
                        // as it's unlikely that a derived member would have the same name
                        let mut base_m_name = base_member.name.clone();
                        while members.iter().any(|x| x.name == base_m_name) {
                            base_m_name.push('_');
                        }
                        let m = MemberDef {
                            offset: m.offset + base_member.offset,
                            name: base_m_name,
                            is_base: base_member.is_base,
                            ty_yaml: base_member.ty_yaml.clone(),
                        };
                        members.push(m);
                    }
                    continue;
                }
            } else if m_size == 0 {
                let r = report!(TypeError::InvalidLayout).attach_printable(format!(
                    "Non-base Member `{}` has zero size in the struct layout",
                    m_name
                ));
                return Err(r);
            }

            let ty_yaml = if is_vfptr {
                TypeName::pointer(TypeName::Name(format!("{name}_vtbl"))).yaml_string()
            } else if m.is_bitfield {
                let ty_name = match m.byte_size {
                    1 => TypeName::Prim(TypePrim::U8),
                    2 => TypeName::Prim(TypePrim::U16),
                    4 => TypeName::Prim(TypePrim::U32),
                    8 => TypeName::Prim(TypePrim::U64),
                    16 => TypeName::Prim(TypePrim::U128),
                    _ => {
                        let r = report!(TypeError::InvalidLayout).attach_printable(format!(
                            "Bitfield member `{}` has invalid size: {}",
                            m_name, m.byte_size
                        ));
                        return Err(r);
                    }
                };
                ty_name.yaml_string()
            } else {
                // if the member is a struct with only one member and no vtable,
                // inline it
                let member_bucket = self.buckets.get(bkt).unwrap();
                let ty_name = self.bkt2name.get(bkt).unwrap();
                let mut ty_yaml = ty_name.yaml_string();
                let mut inlined = false;
                if (info.vtable.is_empty() || !m.is_base)
                    && member_bucket.type_ == BucketType::Struct
                {
                    if let Some(TypeDef::Struct(def)) =
                        self.create_def_for_bucket(bkt, name2def, referenced_names)?
                    {
                        // need to make sure if base type only contains vtable, it's not inlined
                        if def.vtable.is_empty() && def.members.len() == 1 {
                            ty_yaml = def.members[0].ty_yaml.clone();
                            inlined = true;
                            // if inlined, the type could be a non-struct, in which case IDA will break
                            // if it's set as base
                            if m.is_base {
                                m.is_base = false;
                                // we will take the member's name as well to be more descriptive
                                m_name = def.members[0].name.clone();
                            }
                        }
                    }
                }
                if !inlined {
                    referenced_names.add(name, ty_name);
                }
                ty_yaml
            };
            let m = MemberDef {
                offset: m.offset,
                name: m_name,
                is_base: m.is_base,
                ty_yaml,
            };
            members.push(m);
        }

        // VTABLE    ====
        let mut vtable: Vec<(String, String)> = Vec::with_capacity(info.vtable.len());
        for vfptr in info.vtable.into_iter() {
            let mut v_name = vfptr.name;
            while vtable.iter().any(|x| x.0 == v_name) {
                v_name.push('_');
            }
            let retty_name = self
                .bkt2name
                .get(self.off2bkt.get(&vfptr.function.retty).unwrap())
                .unwrap();
            let argty_names = vfptr
                .function
                .params
                .iter()
                .map(|&offset| {
                    self.bkt2name
                        .get(self.off2bkt.get(&offset).unwrap())
                        .unwrap()
                        .clone()
                })
                .collect::<Vec<_>>();
            let ty_name = TypeName::Comp(Box::new(TypeComp::subroutine(
                retty_name.clone(),
                argty_names,
            )));
            let ty_name = TypeName::pointer(ty_name);
            referenced_names.add(name, &ty_name);

            vtable.push((v_name, ty_name.yaml_string()))
        }

        let def = StructDef {
            name: name.to_string(),
            vtable,
            size: info.size,
            alignment,
            members,
        };

        Ok(def)
    }
}

/// Map of referenced type -> referrers
#[derive(Default)]
struct NameRefMap(BTreeMap<String, BTreeSet<String>>);
impl NameRefMap {
    fn add(&mut self, referrer: &str, refed: &TypeName) {
        for r in refed.referenced_names() {
            self.0.entry(r).or_default().insert(referrer.to_string());
        }
    }
}
