"""
Frontend implementation for IDA

In IDA, choose File > Script file... or press Alt+F7 to run this script

Please make sure to run the script AFTER auto-analysis is complete in a fresh database.

Note that IDA 7.7 Only supports Python <=3.11
"""

"""

This implementation is largely adopted from classgen: https://github.com/leoetlino/classgen

MIT License

Copyright (c) 2021 leoetlino

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""

from printutil import verboseln, veryverboseln
from enumdef import EnumDef, EnumImportVisitor
from uniondef import UnionDef, UnionImportVisitor
from structdef import StructDef, StructImportVisitor
from addrdef import AddrImportVisitor
from common import _assert
from frontend import Frontend
from tyyaml import TyyamlVisitor

import math
import ida_typeinf
import ida_nalt
import ida_name

class IDAFrontend(Frontend):

    ## HEURISTICS

    def get_member_heuristics(self):
        return [
            # prefers not __placeholder
            lambda m: m != "__placeholder",
            # prefers __vtable
            lambda m: m == "__vtable",
            # prefers not starting with _
            lambda m: not m.startswith("_"),
            # prefers not starting with field_
            lambda m: not m.startswith("field_"),
            # prefers not empty
            lambda m: bool(m),
            # prefers not ending with a hex number (offset)
            lambda m: not m in "0123456789abcdefABCDEF",
        ]
    
    def member_fallback_heuristic(self, _m1, _m2):
        # if none of the above is hit, prefers the new name
        return True
    
    def get_symbol_heuristics(self):
        return [   # prefers non-empty
            lambda m: bool(m),
            # prefers not starting with sub_, nullsub_ or j_
            lambda m: not m.startswith("sub_") and not m.startswith("nullsub_") and not m.startswith("j_"),
            # prefers mangled
            lambda m: m.startswith("_Z"),
        ]

    def symbol_fallback_heuristic(self, old, _new):
        # if old is mangled, it can probably always override
        return old.startswith("_Z")
    
    def get_tyyaml_visitor(self):
        return IDATyyamlVisitor()
    
    def get_vtable_struct_name(self, name):
        return name + "_vtbl"
    
    def fill_existing_enum_def(self, name, out: EnumDef):
        existing = ida_typeinf.tinfo_t()
        _assert(existing is not None, f"failed to create tinfo_t when getting existing enum: {name}")
        if not existing.get_named_type(None, name):
            return out
        
        existing_data = ida_typeinf.enum_type_data_t()
        _assert(existing_data is not None, f"failed to create enum_type_data_t when getting existing enum: {name}")
        if not existing.get_enum_details(existing_data):
            return out
        value2enumeratorname = {}
        
        for member in existing_data:
            enumerator_name: str = member.name
            if enumerator_name.startswith(name + "::"):
                enumerator_name = enumerator_name[len(name) + 2]
            value2enumeratorname[int(member.value)] = enumerator_name

        out.enumerators = []
        for value in sorted(value2enumeratorname):
            enumerator = (value2enumeratorname[value], value)
            out.enumerators.append(enumerator)

        return out
    
    def make_enum_import_visitor(self, name, old_info, new_info):
        data = ida_typeinf.enum_type_data_t()
        _assert(data is not None, f"Failed to create enum data for: {name}")
        return IDAEnumImportVisitor(name, data)
    
    def get_existing_union_member_names(self, name) -> list[str]:
        existing = ida_typeinf.tinfo_t()
        _assert(existing is not None, f"failed to create tinfo_t when getting existing union: {name}")
        if not existing.get_named_type(None, name):
            return []
        existing_data = ida_typeinf.udt_type_data_t()
        _assert(existing_data is not None, f"failed to create ude_type_data_t when getting existing union: {name}")
        if not existing.get_udt_details(existing_data):
            return []
        if not existing_data.is_union:
            return []
        membernames = []
        for member in existing_data:
            membernames.append(member.name)
        return membernames
    
    def make_union_import_visitor(self, name, new_info: UnionDef):
        _create_placeholder(name, new_info.size, new_info.align)
        udt = ida_typeinf.udt_type_data_t()
        _assert(udt is not None, f"failed to create udt_type_data_t for: {name}")
        udt.taudt_bits |= ida_typeinf.TAUDT_CPPOBJ
        udt.is_union = True
        return IDAUnionImportVisitor(name, udt)
    
    def get_existing_struct_offset_to_member_names(self, name) -> dict[int, str]:
        existing = ida_typeinf.tinfo_t()
        _assert(existing is not None, f"failed to create tinfo_t when getting existing union: {name}")
        if not existing.get_named_type(None, name):
            return {}
        existing_data = ida_typeinf.udt_type_data_t()
        _assert(existing_data is not None, f"failed to create ude_type_data_t when getting existing union: {name}")
        if not existing.get_udt_details(existing_data):
            return {}
        if existing_data.is_union:
            return {}
        off2membername = {}
        for member in existing_data:
            off2membername[member.offset // 8] = member.name # convert to byts
        return off2membername

    def make_struct_import_visitor(self, name, new_info: StructDef):
        _create_placeholder(name, new_info.size, new_info.align)
        udt = ida_typeinf.udt_type_data_t()
        _assert(udt is not None, f"failed to create udt_type_data_t for: {name}")
        udt.taudt_bits |= ida_typeinf.TAUDT_CPPOBJ
        udt.is_union = False
        return IDAStructImportVisitor(name, udt)
    
    def get_existing_function(self, addr: int):
        existing_func = ida_typeinf.func_type_data_t()
        _assert(existing_func is not None, f"failed to create func_type_data_t when getting existing function at: 0x{addr:08x}")
        existing_tinfo = ida_typeinf.tinfo_t()
        _assert(existing_tinfo is not None, f"failed to create tinfo_t when getting existing function at: 0x{addr:08x}")
        if not ida_nalt.get_tinfo(existing_tinfo, addr):
            return False, [], None
        if not existing_tinfo.get_func_details(existing_func):
            return False, [], None
        existing_names = []
        for arg in existing_func:
            existing_names.append(arg.name)
        return True, existing_names, existing_func
    
    def get_symbol_name_by_address(self, addr: int) -> str | None:
        name = ida_name.get_name(addr)
        if not name:
            return None
        return name
    
    def set_symbol_name_by_address(self, addr: int, name: str):
        ida_name.set_name(addr, name)

    def make_data_addr_import_visitor(self, addr, name):
        return IDAAddrImportVisitor(addr, name, None)
    
    def make_func_addr_import_visitor(self, addr, name):
        func = ida_typeinf.func_type_data_t()
        _assert(func is not None, f"failed to create func_type_data_t for function at: 0x{addr:08x}, {name}")
        return IDAAddrImportVisitor(addr, name, func)
    
    
class IDATyyamlVisitor(TyyamlVisitor):
    def visit_pointer(self, base):
        tinfo = ida_typeinf.tinfo_t()
        if tinfo is None:
            return None
        if not tinfo.create_ptr(base):
            return None
        return tinfo
    
    def visit_array(self, base, length):
        tinfo = ida_typeinf.tinfo_t()
        if tinfo is None:
            return None
        if not tinfo.create_array(base, length):
            return None
        return tinfo

    def visit_subroutine_start(self, rettype):
        func = ida_typeinf.func_type_data_t()
        if func is None:
            return None
        func.cc = ida_typeinf.CM_CC_FASTCALL
        func.rettype = rettype
        return func
    
    def visit_function_arg(self, _subroutine, argtype):
        funcarg = ida_typeinf.funcarg_t()
        if funcarg is None:
            return None
        funcarg.type = argtype
        return funcarg
    
    def visit_subroutine_args(self, subroutine, args):
        for arg in args:
            subroutine.push_back(arg)
    
    def visit_subroutine_end(self, subroutine):
        tinfo = ida_typeinf.tinfo_t()
        if tinfo is None:
            return None
        if not tinfo.create_func(subroutine):
            return None
        return tinfo
    
    def visit_name(self, name):
        # IDA dislikes names starting with (
        if name.startswith("("):
            return None
        return name
    
    def visit_name_ptmf(self, name):
        # IDA dislikes names starting with (
        if name.startswith("("):
            return None
        return name + "_ptmf"
    
    def visit_named(self, name):
        # IDA dislikes names starting with (
        if name.startswith("("):
            return None
        tinfo = ida_typeinf.tinfo_t()
        if tinfo is None:
            return None
        if not tinfo.get_named_type(None, name):
            return None
        return tinfo
    
    def visit_void(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_VOID)
    def visit_bool(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_BOOL)
    def visit_u8(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_UCHAR)
    def visit_u16(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_UINT16)
    def visit_u32(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_UINT32)
    def visit_u64(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_UINT64)
    def visit_u128(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_UINT128)
    def visit_i8(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_INT8)
    def visit_i16(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_INT16)
    def visit_i32(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_INT32)
    def visit_i64(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_INT64)
    def visit_i128(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_INT128)
    def visit_f32(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_FLOAT)
    def visit_f64(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_DOUBLE)
    def visit_f128(self):
        return ida_typeinf.tinfo_t(ida_typeinf.BTF_LDOUBLE)

class IDAEnumImportVisitor(EnumImportVisitor):
    def __init__(self, name, data):
        self.name = name
        self.data = data

    def visit_size(self, size):
        self.data.bte |= int(math.log2(size)) + 1 # set byte size
        _assert(self.data.calc_nbytes() == size, f"Enum size mismatch: Actual: {self.data.calc_nbytes()} != Expected: {size}")
    
    def visit_enumerator(self, enumerator_name, value):
        member = ida_typeinf.enum_member_t()
        _assert(member is not None, f"Failed to create enum_member_t for enumerator: {enumerator_name}")
        member.value = value
        member.name = self.name + "::" + enumerator_name
        self.data.push_back(member)

    def finish(self):
        tinfo = ida_typeinf.tinfo_t()
        _assert(tinfo.create_enum(self.data), f"Failed to create enum type: {self.name}")
        _set_tinfo(self.name, tinfo)

class IDAUnionImportVisitor(UnionImportVisitor):
    def __init__(self, name, udt):
        self.name = name
        self.udt = udt
        self.tinfo = None

    def visit_alignment(self, align):
        _set_udt_align(self.udt, align)

    def visit_union_member(self, member_name, tinfo):
        member_d = ida_typeinf.udt_member_t()
        _assert(member_d is not None, f"Failed to create udt_member_t for union: {member_name}")
        member_d.offset = 0
        member_d.name = member_name
        member_d.type = tinfo
        _assert(member_d.type is not None, f"Failed to set union member type: {self.name}")
        member_size = member_d.type.get_size()
        _assert(member_size != ida_typeinf.BADSIZE, f"Failed to get union member size: {self.name}")
        member_d.size = member_size * 8 # bits
        self.udt.push_back(member_d)

    def visit_size(self, size):
        tinfo = ida_typeinf.tinfo_t()
        _assert(tinfo is not None, f"Failed to create tinfo_t for union: {self.name}")
        _assert(tinfo.create_udt(self.udt, ida_typeinf.BTF_UNION), f"Failed to create union type: {self.name}")
        _assert(tinfo.get_size() == size, f"Union size mismatch: Actual: {tinfo.get_size()} != Expected: {size}")
        self.tinfo = tinfo

    def finish(self):
        _assert(self.tinfo is not None, "Did not create tinfo yet!")
        _set_tinfo(self.name, self.tinfo)

class IDAStructImportVisitor(StructImportVisitor):
    def __init__(self, name, udt):
        self.name = name
        self.udt = udt
        self.tinfo = None

    def visit_alignment(self, align):
        _set_udt_align(self.udt, align)

    def visit_struct_member(self, offset_bytes, member_name, is_vtable, is_base, tinfo):
        member_d = ida_typeinf.udt_member_t()
        _assert(member_d is not None, f"Failed to create udt_member_t for struct: {member_name}")
        member_d.offset = offset_bytes * 8 # bits
        member_d.name = member_name
        member_d.type = tinfo
        if is_vtable:
            member_d.set_vftable()
        if is_base:
            member_d.set_baseclass()
        _assert(member_d.type is not None, f"Failed to set struct member type: {self.name}")
        member_size = member_d.type.get_size()
        _assert(member_size != ida_typeinf.BADSIZE, f"Failed to get struct member size: {self.name}")
        member_d.size = member_size * 8 # bits
        self.udt.push_back(member_d)

    def visit_size(self, size):
        tinfo = ida_typeinf.tinfo_t()
        _assert(tinfo is not None, f"Failed to create tinfo_t for struct: {self.name}")
        _assert(tinfo.create_udt(self.udt, ida_typeinf.BTF_STRUCT), f"Failed to create struct type: {self.name}")
        if tinfo.get_size() != size:
            # If size mismatch, try explicit tail padding
            verboseln(f"Struct size mismatch, trying explicit tail padding")
            _explicit_tail_padding(self.udt, size)
            tinfo = ida_typeinf.tinfo_t()
            _assert(tinfo is not None, f"Failed to create tinfo_t for struct: {self.name}")
            _assert(tinfo.create_udt(self.udt, ida_typeinf.BTF_STRUCT), f"Failed to create struct type: {self.name}")
            _assert(tinfo.get_size() != size, f"Struct size mismatch after tail padding: Actual: {tinfo.get_size()} != Expected: {size}")
            verboseln(f"Struct size OK with explicit tail padding added")
        self.tinfo = tinfo

    def finish(self):
        _assert(self.tinfo is not None, "Did not create tinfo yet!")
        _set_tinfo(self.name, self.tinfo)

class IDAAddrImportVisitor(AddrImportVisitor):
    def __init__(self, addr, name, func):
        self.addr = addr
        self.name = name
        self.func = func
        self.tinfo = None

    def visit_data_type(self, tinfo):
        self.tinfo = tinfo

    def visit_rettype(self, tinfo):
        self.func.rettype = tinfo
        _assert(self.func.rettype is not None, f"Failed to set function return type")

    def visit_old_rettype(self, func_obj):
        t = ida_typeinf.tinfo_t(func_obj.rettype)
        _assert(t, f"failed to create tinfo_t for old rettype")
        self.visit_rettype(t)

    def visit_dummy_rettype(self):
        self.visit_rettype(ida_typeinf.tinfo_t(ida_typeinf.BTF_INT64))

    def visit_argument(self, name: str, tinfo):
        funcarg = ida_typeinf.funcarg_t()
        _assert(funcarg is not None, f"Failed to create funcarg_t for arg: {name}")
        funcarg.name = name
        funcarg.type = tinfo
        _assert(funcarg.type is not None, f"Failed to set func arg type: {name}")
        self.func.push_back(funcarg)

    def visit_old_argument(self, name, i, func_obj):
        t = ida_typeinf.tinfo_t(func_obj[i].type)
        _assert(t is not None, f"failed to create tinfo_t for old arg type")
        self.visit_argument(name, t)

    def visit_dummy_argument(self, name: str):
        self.visit_argument(name, ida_typeinf.tinfo_t(ida_typeinf.BTF_INT64))

    def finish(self):
        """Set the type and done"""
        if self.func is not None:
            tinfo = ida_typeinf.tinfo_t()
            _assert(tinfo is not None, f"Failed to create tinfo_t for function: {self.name}")
            _assert(tinfo.create_func(self.func), f"Failed to create function type")
            _set_tinfo_by_address(self.addr, tinfo)
        elif self.tinfo is not None:
            _set_tinfo_by_address(self.addr, self.tinfo)
        else:
            raise RuntimeError("addr is not a function or data. This should not happen")


def _set_tinfo(name, tinfo):
    """Set a tinfo_t by name in IDA"""
    _assert(not name.startswith("("), f"Invalid name: {name}")
    ret = tinfo.set_named_type(None, name, ida_typeinf.NTF_REPLACE)
    _assert(ret == ida_typeinf.TERR_OK, f"Failed to import type: {name}")

def _set_tinfo_by_address(addr, tinfo):
    _assert(ida_nalt.set_tinfo(addr, tinfo), f"Failed to set type for address: {hex(addr)}")

def _create_placeholder(name: str, size: int, align: int):
    """Create a placeholder tinfo_t by name, so recursive pointer ref works"""
    existing = ida_typeinf.tinfo_t()
    if existing.get_named_type(None, name):
        existing_data = ida_typeinf.udt_type_data_t()
        if existing.get_udt_details(existing_data):
            ok = True
            existing_size = existing.get_size()
            if existing_size != size:
                veryverboseln(f"Size mismatch: {name}, existing_size={existing_size}, size={size}")
                ok = False
            if ok:
                existing_sda = existing_data.sda
                expected_sda = _align2sda(align)
                if existing_sda != expected_sda:
                    veryverboseln(f"SDA mismatch: {name}, expected={expected_sda}, actual={existing_sda}")
                    ok = False
            if ok:
                verboseln(f"Existing type: {name}, size={size}, align={align}")
                return

    verboseln(f"Creating placeholder type: {name}, size={size}, align={align}")
    storage_tinfo = ida_typeinf.tinfo_t(ida_typeinf.BTF_CHAR)
    _assert(storage_tinfo.create_array(storage_tinfo, size), f"Failed to create placeholder type: {name}")
    member = ida_typeinf.udt_member_t()
    member.name = "__placeholder"
    member.type = storage_tinfo
    member.offset = 0
    member.size = size * 8 # bits

    udt = ida_typeinf.udt_type_data_t()
    udt.taudt_bits |= ida_typeinf.TAUDT_CPPOBJ
    _set_udt_align(udt, align)
    udt.push_back(member)

    tinfo = ida_typeinf.tinfo_t()
    _assert(tinfo is not None, f"Failed to create tinfo_t for placeholder type: {name}")
    _assert(tinfo.create_udt(udt, ida_typeinf.BTF_STRUCT), f"Failed to create placeholder type: {name}")
    _set_tinfo(name, tinfo)

def _align2sda(align):
    """
    Convert alignment to Declared Structure Alignment value
    See https://hex-rays.com/products/ida/support/sdkdoc/structudt__type__data__t.html 
    """
    return int(math.log2(align)) + 1

def _set_udt_align(udt, align):
    udt.sda = _align2sda(align)
    # udt.effalign = align

def _explicit_tail_padding(udt, size):
    """Explicitly add tail padding to a struct"""
    if udt.empty():
        return
    last_field = udt.back()
    # bits
    gap_offset = last_field.offset + last_field.size
    gap_size = size - gap_offset
    if gap_size <= 0:
        return
    gap_member = ida_typeinf.udt_member_t()
    gap_member.name = f"__tail_{gap_offset // 8:x}"
    gap_member.size = gap_size
    gap_member.offset = gap_offset
    c = ida_typeinf.tinfo_t(ida_typeinf.BTF_CHAR)
    gap_type = ida_typeinf.tinfo_t()
    _assert(gap_type.create_array(c, gap_size // 8), f"Failed to create tail padding type")
    gap_member.type = gap_type
    udt.push_back(gap_member)
