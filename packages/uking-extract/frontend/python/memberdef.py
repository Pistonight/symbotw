class MemberDef:
    name = ""
    offset = 0
    tyyaml = []
    is_base = False

def _make_member(name, offset, tyyaml, is_base=False):
    """Make a member definition for struct, used by codegen"""
    member = MemberDef()
    member.name = name
    member.offset = offset
    member.tyyaml = tyyaml
    member.is_base = is_base
    return member

def _make_union_member(name, tyyaml):
    """Make a member definition for union, used by codegen"""
    member = MemberDef()
    member.name = name
    member.offset = 0
    member.tyyaml = tyyaml
    member.is_base = False
    return member

