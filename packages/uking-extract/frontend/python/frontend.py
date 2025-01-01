from common import _assert
from enumdef import EnumDef, EnumImportVisitor
from uniondef import UnionDef, UnionImportVisitor
from structdef import StructDef, StructImportVisitor
from addrdef import AddrImportVisitor

import typing

class Frontend:
    """Interface implemented by each frontend"""
    # Look at frontend_ida.py for example for how to implement

    # ---------- HEURISTICS ----------
    # These define rules for naming stuff

    def get_member_heuristics(self):
        """Get heuristics for overiding member names"""
        _assert(False, "please implement get_member_heuristics")

    def member_fallback_heuristic(self, m1, m2):
        """Fallback heuristic for overiding member names, called if none of get_member_heuristics gave a definitive answer"""
        _assert(False, "please implement member_fallback_heuristic")

    def get_symbol_heuristics(self):
        """Get heuristics for overiding symbol names"""
        _assert(False, "please implement get_symbol_heuristics")

    def symbol_fallback_heuristic(self, m1, m2):
        """Fallback heuristic for overiding symbol names, called if none of get_symbol_heuristics gave a definitive answer"""
        _assert(False, "please implement symbol_fallback_heuristic")

    # ---------- TYPE INFO ----------
    # Binding for values that represent type information
    def get_tyyaml_visitor(self):
        """Return an implementation of TyyamlVisitor"""
        _assert(False, "please implement get_tyyaml_visitor")

    def get_vtable_struct_name(self, name):
        """Get the vtable struct name for a struct. Some tools like IDA allows automatically decompiling virtualized calls"""
        _assert(False, "please implement get_vtable_struct_name")

    def fill_existing_enum_def(self, name: str, enum_def: EnumDef):
        """If an enum with name already exists, fill in the enum definition, otherwise don't do anything"""
        _assert(False, "please implement fill_existing_enum_def")

    def make_enum_import_visitor(self, name: str, old_info: EnumDef, new_info: EnumDef) -> EnumImportVisitor:
        """Return an EnumImportVisitor for importing the enum type"""
        _assert(False, "please implement make_enum_import_visitor")

    def get_existing_union_member_names(self, name: str) -> list[str]:
        """If a union with name already exists, return the member names of that union. Otherwise return empty list"""
        _assert(False, "please implement get_existing_union_member_names")

    def make_union_import_visitor(self, name: str, new_info: UnionDef) -> UnionImportVisitor:
        """Return a UnionImportVisitor for importing the union type"""
        _assert(False, "please implement make_union_import_visitor")

    def get_existing_struct_offset_to_member_names(self, name: str) -> dict[int, str]:
        """If a struct with name already exists, return the offset_bytes -> member names of that struct. Otherwise return empty dict"""
        _assert(False, "please implement get_existing_struct_offset_to_member_names")
    
    def make_struct_import_visitor(self, name: str, new_info: StructDef) -> StructImportVisitor:
        """Return a StructImportVisitor for importing the struct type"""
        _assert(False, "please implement make_struct_import_visitor")

    def get_existing_function(self, addr: int) -> tuple[bool, list[str], typing.Any]:
        """
            Get the existing function at the address.
            Return True, arg_names, func_obj if there is an existing definition
            Return False, [], None if there is not

            The func_obj is passed to the visitor
        """

    def get_symbol_name_by_address(self, addr: int) -> str | None:
        """Return the name of the symbol at address. Return None if it doesn't exist"""
        _assert(False, "please implement get_symbol_name_by_address")

    def set_symbol_name_by_address(self, addr: int, name: str):
        """Set the symbol name at address"""
        _assert(False, "please implement set_symbol_name_by_address")

    def make_data_addr_import_visitor(self, addr: int, name: str) -> AddrImportVisitor:
        """Return an AddrImportVisitor for importing a data symbol"""
        _assert(False, "please implement make_data_addr_import_visitor")

    def make_func_addr_import_visitor(self, addr: int, name: str) -> AddrImportVisitor:
        """Return an AddrImportVisitor for importing a function symbol"""
        _assert(False, "please implement make_func_addr_import_visitor")
