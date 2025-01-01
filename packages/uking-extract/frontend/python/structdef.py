from common import _assert
from memberdef import MemberDef, _make_member

class StructDef:
    size = 0
    align = 0
    members: list[MemberDef] = [] # MemberDef[]

    def __init__(self, size, align, members):
        self.size = size
        self.align = align
        self.members = members

def _make_vtable(vtable): # vtable: (name, tyyaml)[]
    size = len(vtable) * 8
    align = 1
    members = []
    for (i, (name, tyyaml)) in enumerate(vtable):
        member = _make_member(name, i * 8, tyyaml)
        members.append(member)
    return StructDef(size, align, members)

class StructImportVisitor:
    """Interface for visitor state when importing a StructDef"""

    # Order:
    # - alignment
    # - union_member for each member
    # - size
    # - finish

    def visit_alignment(self, align):
        """Set the align in bytes for the struct type"""
        _assert(False, "please implement visit_alignment")

    def visit_struct_member(self, offset_bytes: int, member_name: str, is_vtable: bool, is_base: bool, tinfo):
        """Set a member, the type info passed in is from frontend-specific TyyamlVisitor"""
        _assert(False, "please implement visit_struct_member")

    def visit_size(self, size):
        """Set the size in bytes for the struct type"""
        _assert(False, "please implement visit_size")

    def finish(self):
        """Finish the struct and set the type"""
        _assert(False, "please implement finish")