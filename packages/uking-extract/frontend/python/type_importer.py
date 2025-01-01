from printutil import infoln, verboseln, veryverboseln, PrintScope
from heuristics import Heuristics
from enumdef import EnumDef
from uniondef import UnionDef
from structdef import StructDef, _make_vtable
from frontend import Frontend
from tyyaml import TyyamlParser
from common import _assert

class TypeImporter:
    """Importer for importing type information"""
    frontend: Frontend
    heuristics: Heuristics
    tyyaml: TyyamlParser

    skipping: bool = False

    # names that are already imported
    imported = set()

    name2struct: dict[str, StructDef] = {}
    name2vtable_struct: dict[str, StructDef] = {}
    name2enum: dict[str, EnumDef] = {}
    name2union: dict[str, UnionDef] = {}

    def __init__(self, frontend: Frontend):
        self.frontend = frontend
        self.heuristics = Heuristics(frontend)
        tyyaml_visitor = frontend.get_tyyaml_visitor()
        self.tyyaml = TyyamlParser(tyyaml_visitor, lambda name: self._import_named(name))

    def skip(self):
        self.skipping = True

    def add_struct(self, name, struct, vtable):
        """Add a struct definition"""
        self.name2struct[name] = struct
        if vtable:
            self.name2vtable_struct[name] = _make_vtable(vtable)

    def add_enum(self, name, enum):
        """Add an enum definition"""
        self.name2enum[name] = enum

    def add_union(self, name, union):
        """Add a union definition"""
        self.name2union[name] = union

    def run_import(self, substring_pattern):
        """Import all types whose name contains the given substring"""
        struct_names = [name for name in self.name2struct if not substring_pattern or substring_pattern in name]
        enum_names = [name for name in self.name2enum if not substring_pattern or substring_pattern in name]
        union_names = [name for name in self.name2union if not substring_pattern or substring_pattern in name]
        struct_total = len(struct_names)
        for (i, name) in enumerate(struct_names):
            infoln(f"struct {i}/{struct_total}")
            self._import_named(name)
        enum_total = len(enum_names)
        for (i, name) in enumerate(enum_names):
            infoln(f"enum {i}/{enum_total}")
            self._import_named(name)
        union_total = len(union_names)
        for (i, name) in enumerate(union_names):
            infoln(f"union {i}/{union_total}")
            self._import_named(name)

    def _import_named(self, name):
        if self.skipping:
            return
        if name in self.imported:
            return
        infoln(f"Importing {name}")
        self.imported.add(name)
        with PrintScope():
            if name in self.name2enum:
                try:
                    self._import_enum(name)
                except:
                    infoln(f"Failed to import enum {name}")
                    raise
            elif name in self.name2struct:
                try:
                    self._import_struct(name)
                except:
                    infoln(f"Failed to import struct {name}")
                    raise
            elif name in self.name2union:
                try:
                    self._import_union(name)
                except:
                    infoln(f"Failed to import union {name}")
                    raise
            else:
                raise RuntimeError(f"Unknown type: {name}")

    def _import_enum(self, name):
        verboseln(f"Enum {name}")
        new_info: EnumDef = self.name2enum[name]
        old_info = EnumDef(1, [("UNKNOWN", 0)])
        self.frontend.fill_existing_enum_def(name, old_info)
        _assert(1<=new_info.size<=8, f"Invalid new enum size: {new_info.size}")
        _assert(1<=old_info.size<=8, f"Invalid old enum size: {old_info.size}")
        if new_info == old_info:
            veryverboseln(f"skipped (existing info matches)")
            return
        old_value2name = {}
        for (name, value) in old_info.enumerators:
            old_value2name[value] = name

        enum_visitor = self.frontend.make_enum_import_visitor(name, old_info, new_info)
        enum_visitor.visit_size(new_info.size)
        for (new_name, value) in new_info.enumerators:
            try:
                value = int(value)
                name = new_name
                if value in old_value2name:
                    old_name = old_value2name[value]
                    if new_name != old_name and self.heuristics.can_ovrd_member(new_name, old_name):
                        veryverboseln(f"Rename enumerator: {old_name} -> {new_name}")
                    else:
                        veryverboseln(f"Keep enumerator name: {old_name}")
                        name = old_name
                else:
                    veryverboseln(f"new enumerator: {new_name}")
                enum_visitor.visit_enumerator(name, value)
            except:
                infoln(f"Failed to add enum member {new_name} to {name}: value = {value}")
                raise
        verboseln(f"Setting enum type: {name}")
        enum_visitor.finish()

    def _import_union(self, name):
        verboseln(f"Union {name}")
        new_info: UnionDef = self.name2union[name]
        old_membernames = self.frontend.get_existing_union_member_names(name)
        # only reuse name if member count matches
        reuse_name = len(old_membernames) == len(new_info.members)
        if not reuse_name:
            verboseln("Not reusing names because member count changed")

        union_visitor = self.frontend.make_union_import_visitor(name, new_info)
        union_visitor.visit_alignment(new_info.align)

        for (i, m) in enumerate(new_info.members):
            _assert(m.offset == 0, f"Union member {m.name} has non-zero offset: {m.offset}")
            new_name = m.name
            name = new_name

            if reuse_name:
                old_name = old_membernames[i]
                if new_name != old_name and self.heuristics.can_ovrd_member(new_name, old_name):
                    veryverboseln(f"Rename union member: {old_name} -> {new_name}")
                else:
                    veryverboseln(f"Reuse union member name: {old_name}")
                    name = old_name
            else:
                veryverboseln(2, f"Add union member: {new_name}")

            member_type = self.tyyaml.parse_tyyaml(m.tyyaml)
            union_visitor.visit_union_member(name, member_type)
        union_visitor.visit_size(new_info.size)
        union_visitor.finish()

    def _import_struct(self, name):
        new_info = self.name2struct[name]
        self._import_struct_with_info(name, new_info)

    def _import_struct_with_info(self, name: str, new_info: StructDef):
        verboseln(f"Struct {name}")
        old_off2membernames = self.frontend.get_existing_struct_offset_to_member_names(name)

        struct_visitor = self.frontend.make_struct_import_visitor(name, new_info)
        struct_visitor.visit_alignment(new_info.align)

        if name in self.name2vtable_struct:
            # if the struct has a vtable, also import it
            vtable_info = self.name2vtable_struct[name]
            vtable_struct_name = self.frontend.get_vtable_struct_name(name)
            if vtable_struct_name in self.imported:
                return
            infoln(f"Importing vtable for {name}")
            self.imported.add(vtable_struct_name)
            self._import_struct_with_info(vtable_struct_name, vtable_info)

        for m in new_info.members:
            new_name = m.name
            name = new_name
            if m.offset in old_off2membernames:
                old_name = old_off2membernames[m.offset]
                if new_name != old_name and self.heuristics.can_ovrd_member(new_name, old_name):
                    veryverboseln(f"Rename struct member: {old_name} -> {new_name}")
                else:
                    veryverboseln(f"Reuse struct member name: {old_name}")
                    name= old_name
            else:
                veryverboseln(f"Add struct member: {m.name}")
            is_vtable = name == "__vtable"
            member_type = self.tyyaml.parse_tyyaml(m.tyyaml)

            struct_visitor.visit_struct_member(
                m.offset,
                name,
                is_vtable,
                m.is_base,
                member_type
            )
        struct_visitor.visit_size(new_info.size)
        struct_visitor.finish()
