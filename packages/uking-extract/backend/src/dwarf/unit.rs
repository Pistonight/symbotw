use error_stack::{report, Result, ResultExt};
use gimli::{
    Abbreviations, AttributeValue, DW_AT_declaration, DW_AT_external, DwTag, UnitSectionOffset,
};

use crate::parsed::NamespaceMap;

use super::{Dwarf, Error, In, Node, Tree, Unit, UnitHeader, UnitOffset, DIE};

/// A Compile Unit in .debug_info
pub struct UnitCtx<'d, 'i> {
    pub offset: usize,
    pub unit: Unit<'i>,
    pub header: UnitHeader<'i>,
    pub abbrevs: Abbreviations,
    pub dwarf: &'d Dwarf<'i>,
    pub name: &'i str,
}

impl<'d, 'i> std::ops::Deref for UnitCtx<'d, 'i> {
    type Target = Unit<'i>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unit
    }
}

macro_rules! err_ctx {
    ($self:ident, $err:expr, $r:expr) => {{
        use error_stack::ResultExt;
        let r = { $r };
        r.change_context($err)
            .attach_printable_lazy(|| $self.format_context())
    }};
    ($self:ident, $offset:expr, $err:expr, $r:expr) => {{
        use error_stack::ResultExt;
        let r = { $r };
        r.change_context($err)
            .attach_printable_lazy(|| $self.format_offset($offset))
            .attach_printable_lazy(|| $self.format_context())
    }};
}
pub(crate) use err_ctx;
macro_rules! opt_ctx {
    ($self:ident, $err:expr, $opt:expr) => {{
        let opt = { $opt };
        opt.ok_or_else(|| error_stack::report!($err).attach_printable($self.format_context()))
    }};
    ($self:ident, $offset:expr, $err:expr, $opt:expr) => {{
        let opt = { $opt };
        opt.ok_or_else(|| {
            error_stack::report!($err)
                .attach_printable($self.format_offset($offset))
                .attach_printable($self.format_context())
        })
    }};
}
pub(crate) use opt_ctx;
macro_rules! bad {
    ($self:ident, $err:expr) => {{
        Err(error_stack::report!($err).attach_printable($self.format_context()))
    }};
    ($self:ident, $offset:expr, $err:expr) => {{
        Err(error_stack::report!($err)
            .attach_printable($self.format_offset($offset))
            .attach_printable($self.format_context()))
    }};
}
pub(crate) use bad;
// macro_rules! bad_global {
//     ($offset:expr, $err:expr) => {{
//         Err(error_stack::report!($err)
//             .attach_printable(format!("At .debug_info Offset: 0x{:08x}", $offset)))
//     }};
// }

impl<'d, 'i> UnitCtx<'d, 'i> {
    pub fn new(header: UnitHeader<'i>, dwarf: &'d Dwarf<'i>) -> Result<Self, Error> {
        let offset = match header.offset() {
            UnitSectionOffset::DebugInfoOffset(o) => o.0,
            UnitSectionOffset::DebugTypesOffset(_) => return Err(report!(Error::CreateUnitOffset)),
        };
        let unit = Unit::new(dwarf, header).change_context(Error::CreateUnit)?;
        let abbrevs = header
            .abbreviations(&dwarf.debug_abbrev)
            .change_context(Error::CreateUnitAbbrev)?;
        let mut s = Self {
            offset,
            unit,
            header,
            abbrevs,
            dwarf,
            name: "",
        };
        let mut tree = s.tree()?;
        let root = err_ctx!(s, Error::ReadRoot, tree.root())?;
        s.name = s.get_entry_name(root.entry())?;
        Ok(s)
    }
    /// Convert unit offset to global offset in debug_info
    pub fn to_global_offset(&self, offset: UnitOffset) -> usize {
        offset.0 + self.offset
    }

    /// Convert global offset in debug_info to unit offset
    pub fn to_unit_offset(&self, global_offset: usize) -> UnitOffset {
        gimli::UnitOffset(global_offset - self.offset)
    }

    /// Create an entry tree for the unit
    pub fn tree(&self) -> Result<Tree<'i, '_, '_>, Error> {
        err_ctx!(
            self,
            Error::CreateTree,
            self.header.entries_tree(&self.abbrevs, None)
        )
    }

    /// Create entry tree from entry at offset
    pub fn tree_at(&self, offset: UnitOffset) -> Result<Tree<'i, '_, '_>, Error> {
        err_ctx!(
            self,
            self.to_global_offset(offset),
            Error::CreateTree,
            self.header.entries_tree(&self.abbrevs, Some(offset))
        )
    }

    /// Get a single entry at offset
    pub fn entry_at(&self, offset: UnitOffset) -> Result<DIE<'i, '_, '_>, Error> {
        err_ctx!(
            self,
            self.to_global_offset(offset),
            Error::ReadEntry,
            self.unit.entry(offset)
        )
    }

    /// Get the root of entry tree
    pub fn root_of<'t, 'a, 'u>(
        &self,
        offset: UnitOffset,
        tree: &'t mut Tree<'i, 'a, 'u>,
    ) -> Result<Node<'i, 'a, 'u, 't>, Error> {
        err_ctx!(
            self,
            self.to_global_offset(offset),
            Error::ReadRoot,
            tree.root()
        )
    }

    /// Get an attribute value as string
    pub fn get_string(&self, attr: AttributeValue<In<'i>>) -> Result<&'i str, Error> {
        let s = err_ctx!(
            self,
            Error::AttrString,
            self.dwarf.attr_string(&self.unit, attr)
        )?;
        let s = err_ctx!(self, Error::AttrString, s.to_string())?;
        Ok(s)
    }

    /// Check the entry has the tag
    pub fn check_tag(&self, entry: &DIE<'i, '_, '_>, tag: DwTag) -> Result<(), Error> {
        let offset = self.to_global_offset(entry.offset());
        let entry_tag = entry.tag();
        if entry_tag != tag {
            return bad!(self, offset, Error::UnexpectedTag(entry_tag))
                .attach_printable(format!("Expecting: {}", tag));
        }
        Ok(())
    }

    /// Get the DW_AT_external of a DIE
    pub fn get_entry_external(&self, entry: &DIE<'i, '_, '_>) -> Result<bool, Error> {
        let offset = self.to_global_offset(entry.offset());
        let value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_external),
            entry.attr_value(DW_AT_external)
        )?;
        match value {
            None => Ok(false),
            Some(AttributeValue::Flag(x)) => Ok(x),
            _ => bad!(
                self,
                offset,
                Error::BadEntryAttrType(DW_AT_external, "Flag")
            )
            .attach_printable(format!("Got: {:?}", value)),
        }
    }

    /// Get the DW_AT_declaration of a DIE
    pub fn get_entry_declaration(&self, entry: &DIE<'i, '_, '_>) -> Result<bool, Error> {
        let offset = self.to_global_offset(entry.offset());
        let value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_declaration),
            entry.attr_value(DW_AT_declaration)
        )?;
        match value {
            None => Ok(false),
            Some(AttributeValue::Flag(x)) => Ok(x),
            _ => bad!(
                self,
                offset,
                Error::BadEntryAttrType(DW_AT_declaration, "Flag")
            )
            .attach_printable(format!("Got: {:?}", value)),
        }
    }

    /// Get namespaced name of the entry at offset, using the namespace map
    pub fn get_namespaced_name(
        &self,
        entry: &DIE<'i, '_, '_>,
        map: &NamespaceMap<'i>,
    ) -> Result<String, Error> {
        let offset = self.to_global_offset(entry.offset());
        let name = self.get_entry_name(entry)?;
        let name = err_ctx!(self, offset, Error::Namespace, map.get(offset, name))?;
        Ok(name)
    }

    /// Get the namespaced name of entry at offset, allowing missing DW_AT_name
    pub fn get_namespaced_name_optional(
        &self,
        entry: &DIE<'i, '_, '_>,
        map: &NamespaceMap<'i>,
    ) -> Result<Option<String>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let name = self.get_entry_name_optional(entry)?;
        match name {
            Some(name) => {
                let name = err_ctx!(self, offset, Error::Namespace, map.get(offset, name))?;
                Ok(Some(name))
            }
            None => Ok(None),
        }
    }

    /// Execute f on each child from an entry
    pub fn for_each_child_entry<F>(&self, entry: &DIE<'i, '_, '_>, f: F) -> Result<(), Error>
    where
        F: for<'t> FnMut(Node<'i, '_, '_, 't>) -> Result<(), Error>,
    {
        let mut tree = self.tree_at(entry.offset())?;
        let node = self.root_of(entry.offset(), &mut tree)?;
        self.for_each_child(node, f)
    }

    /// Execute f on each child
    pub fn for_each_child<F>(&self, node: Node<'i, '_, '_, '_>, mut f: F) -> Result<(), Error>
    where
        F: for<'t> FnMut(Node<'i, '_, '_, 't>) -> Result<(), Error>,
    {
        let offset = self.to_global_offset(node.entry().offset());
        let mut children = node.children();
        while let Some(child) = err_ctx!(self, offset, Error::ReadChild, children.next())? {
            let c_offset = self.to_global_offset(child.entry().offset());
            err_ctx!(
                self,
                offset,
                Error::Ctx("Processing child entry", c_offset),
                f(child)
            )?;
        }
        Ok(())
    }

    pub fn format_context(&self) -> String {
        format!("In Compile Unit: {}", self.name)
    }

    pub fn format_offset(&self, offset: usize) -> String {
        format!("At .debug_info Offset: 0x{:08x}", offset)
    }
}
