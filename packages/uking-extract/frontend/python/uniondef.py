from common import _assert
from memberdef import MemberDef

class UnionDef:
    size = 0
    align = 0
    members: list[MemberDef] = [] # MemberDef[]

    def __init__(self, size, align, members):
        self.size = size
        self.align = align
        self.members = members

class UnionImportVisitor:
    """Interface for visitor state when importing a UnionDef"""

    # Order:
    # - alignment
    # - union_member for each member
    # - size
    # - finish

    def visit_alignment(self, align):
        """Set the align in bytes for the union type"""
        _assert(False, "please implement visit_alignment")

    def visit_union_member(self, member_name, tinfo):
        """Set a member, the type info passed in is from frontend-specific TyyamlVisitor"""
        _assert(False, "please implement visit_union_member")

    def visit_size(self, size):
        """Set the size in bytes for the union type"""
        _assert(False, "please implement visit_size")

    def finish(self):
        """Finish the union and set the type"""
        _assert(False, "please implement finish")