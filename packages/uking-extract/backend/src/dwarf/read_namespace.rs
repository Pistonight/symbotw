use std::collections::BTreeMap;

use error_stack::Result;
use gimli::{DW_TAG_compile_unit, DW_TAG_namespace, DW_TAG_subprogram, DW_TAG_variable};

use crate::parsed::{Namespace, NamespaceMap};

use super::unit::UnitCtx;
use super::{is_type_tag, Error, Node};

macro_rules! namespace_scope {
    ($namespace:ident, $name:expr, $block:block) => {{
        match $name {
            Some(name) => {
                $namespace.push(name);
                $block
                $namespace.pop();
            }
            None => {
                $block
            }
        }
    }};
}

pub fn read_namespace<'i>(units: &[UnitCtx<'_, 'i>]) -> Result<NamespaceMap<'i>, Error> {
    let mut namespace = Namespace::default();
    let mut offset_to_ns = BTreeMap::new();
    super::process_units!(units, "Register namespaces", unit, root, {
        read_namespace_recur(root, unit, &mut namespace, &mut offset_to_ns)?;
    });
    Ok(offset_to_ns.into())
}

fn read_namespace_recur<'i>(
    node: Node<'i, '_, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    namespace: &mut Namespace<'i>,
    offset_to_ns: &mut BTreeMap<usize, Namespace<'i>>,
) -> Result<(), Error> {
    let entry = node.entry();
    let tag = entry.tag();
    if is_type_tag(tag) || matches!(tag, DW_TAG_subprogram | DW_TAG_variable) {
        let offset = unit.to_global_offset(entry.offset());
        offset_to_ns.insert(offset, namespace.clone());
        // can have inner types, and may be anonymous
        let name = unit.get_entry_name_optional(entry)?;
        namespace_scope!(namespace, name, {
            unit.for_each_child(node, |child| {
                read_namespace_recur(child, unit, namespace, offset_to_ns)
            })?;
        });
        return Ok(());
    }
    match tag {
        DW_TAG_compile_unit => {
            // top-most case
            unit.for_each_child(node, |child| {
                read_namespace_recur(child, unit, namespace, offset_to_ns)
            })?;
        }
        DW_TAG_namespace => {
            // don't need to add
            let name = unit.get_entry_name_optional(entry)?;
            namespace_scope!(namespace, name, {
                unit.for_each_child(node, |child| {
                    read_namespace_recur(child, unit, namespace, offset_to_ns)
                })?;
            });
        }
        _ => {
            // ignore
        }
    }

    Ok(())
}
