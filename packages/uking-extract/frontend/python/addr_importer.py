from type_importer import TypeImporter
from addrdef import FunctionDef, NameAndType
from printutil import infoln, verboseln, veryverboseln
from frontend import Frontend

class AddrImporter:
    """importer for importing address symbols"""
    frontend: Frontend
    ti: TypeImporter
    upper = 0
    name_only = False

    def __init__(self, ti):
        self.ti = ti

    addr2func: dict[int, FunctionDef] = {}
    addr2data: dict[int, NameAndType] = {}

    def set_upper(self, upper):
        """Set the upper 32 address bits"""
        self.upper = upper

    def add_func(self, addr, func):
        """Add function to import"""
        self.addr2func[self._fix_addr(addr)] = func

    def add_data(self, addr, data):
        """Add data symbol to import"""
        self.addr2data[self._fix_addr(addr)] = data

    def run_import(self, name_only, name_pattern):
        self.name_only = name_only
        data_addrs = [addr for addr in self.addr2data if not name_pattern or name_pattern in self.addr2data[addr].name]
        func_addrs = [addr for addr in self.addr2func if not name_pattern or name_pattern in self.addr2func[addr].name]
        for addr in data_addrs:
            self._import_data(addr)
        for addr in func_addrs:
            self._import_func(addr)

    def _fix_addr(self, addr):
        return self.upper << 32 | (addr & 0xFFFFFFFF)

    def _import_data(self, addr):
        info: NameAndType = self.addr2data[addr]
        infoln(f"Importing Data {hex(addr)}: {info.name}")
        if info.name:
            self._set_name(addr, info.name)
        if self.name_only:
            return
        if info.tyyaml:
            visitor = self.frontend.make_data_addr_import_visitor(addr, info.name)
            tinfo = self.ti.tyyaml.parse_tyyaml(info.tyyaml)
            visitor.visit_data_type(tinfo)
            visitor.finish()

    def _import_func(self, addr):
        info: FunctionDef = self.addr2func[addr]
        infoln(f"Importing Function {hex(addr)}: {info.name}")
        if info.name:
            self._set_name(addr, info.name)
        if self.name_only:
            return
        
        has_existing, old_argnames, old_func = self.frontend.get_existing_function(addr)

        reuse_names = has_existing and len(old_argnames) == len(info.args)
        if not reuse_names:
            verboseln(f"Not reusing names because arg count changed")

        visitor = self.frontend.make_func_addr_import_visitor(addr, info.name)

        if info.tyyaml:
            ret_tinfo = self.ti.tyyaml.parse_tyyaml(info.tyyaml)
            verboseln(f"Using new return type")
            visitor.visit_rettype(ret_tinfo)
        elif has_existing and old_func:
            verboseln(f"Keep existing return type")
            visitor.visit_old_rettype(old_func)
        else:
            verboseln(f"Use dummy return type")
            visitor.visit_dummy_rettype()

        for (i, arg) in enumerate(info.args):
            new_name = arg.name
            name = new_name
            
            if reuse_names and i < len(old_argnames):
                old_name = old_argnames[i]
                new_name = arg.name
                if new_name != old_name and self.ti.heuristics.can_ovrd_member(new_name, old_name):
                    veryverboseln(f"Rename arg {i}: {old_name} -> {new_name}")
                else:
                    veryverboseln(f"Reuse arg name {i}: {old_name}")
                    name = old_name
            else:
                veryverboseln(f"Add arg {i}: {arg.name}")

            if arg.tyyaml:
                t = self.ti.tyyaml.parse_tyyaml(arg.tyyaml)
                veryverboseln(f"Using new type for arg {i}")
                visitor.visit_argument(name, t)
            elif has_existing and i < len(old_argnames):
                veryverboseln(f"Keep existing arg {i} type")
                visitor.visit_old_argument(i, old_func)
            else:
                veryverboseln(f"Use dummy arg {i} type")
                visitor.visit_dummy_argument(name)

        visitor.finish()

    def _set_name(self, addr, name):
        """Set the name of an address if it's more preferred than current name"""
        existing_name = self.frontend.get_symbol_name_by_address(addr)
        if existing_name == name:
            return
        if not existing_name or self.ti.heuristics.can_ovrd_symbol(name, existing_name):
            verboseln(f"Rename: {existing_name} -> {name}")
            self.frontend.set_symbol_name_by_address(addr, name)
