##### BUNDLE_IMPORT
from common import _assert
##### BUNDLE_IMPORT

class TyyamlVisitor:
    """
        Visitor to convert Tyyaml (Type YAML) to frontend-specific type binding value

        Type YAML is our representation for C data types. It is designed to be human-readable
        as well as easily machine parsable
    """
    def visit_pointer(self, base):
        """Return a pointer type that equals base* (pointer to base type). Return None if failed"""
        _assert(False, "please implement visit_pointer")

    def visit_array(self, base, length):
        """Return a pointer type that equals base[length] (array of base type of length). Return None if failed"""
        _assert(False, "please implement visit_array")

    # Subroutine types are visited as:
    #  - subroutine_start (returns value that will be passed to visit_function_arg and visit_subroutine_args)
    #  - function_arg for each argument type (returns value that will be passed to visit_subroutine_args as a list)
    #  - subroutine_args
    #  - subroutine_end (returns the type value that represents rettype(argtypes...))
    #
    # Note that subroutine type should NOT return a pointer to the subroutine type,
    # that's already baked into the extracted type info

    def visit_subroutine_start(self, rettype):
        """Return a subroutine visitor state for a subroutine with return type"""
        _assert(False, "please implement visit_subroutine_start")
    
    def visit_function_arg(self, subroutine, argtype):
        """Returns a func arg visitor state for a subroutine with an argument with argtype"""
        _assert(False, "please implement visit_function_arg")

    def visit_subroutine_args(self, subroutine, args):
        """Set the args as a list to the subroutine"""
        _assert(False, "please implement visit_subroutine_args")
    
    def visit_subroutine_end(self, subroutine):
        """Finish visiting a subroutine type and return the type value"""
        _assert(False, "please implement visit_subroutine_end")


    # Named types
    def visit_name(self, name):
        """Return the identifier for the type with name"""
        _assert(False, "please implement visit_name")

    def visit_named(self, name):
        """Return the type value for a named type"""
        _assert(False, "please implement visit_named")

    # Base types
    def visit_void(self):
        """Return the type value for `void` (NOT void*)"""
        _assert(False, "please implement visit_void")

    def visit_bool(self):
        """Return the type value for `bool`"""
        _assert(False, "please implement visit_bool")

    def visit_u8(self):
        """Return the type value for unsigned, 8-bit integer"""
        _assert(False, "please implement visit_u8")

    def visit_u16(self):
        """Return the type value for unsigned, 16-bit integer"""
        _assert(False, "please implement visit_u16")

    def visit_u32(self):
        """Return the type value for unsigned, 32-bit integer"""
        _assert(False, "please implement visit_u32")

    def visit_u64(self):
        """Return the type value for unsigned, 64-bit integer"""
        _assert(False, "please implement visit_u64")

    def visit_u128(self):
        """Return the type value for unsigned, 128-bit integer"""
        _assert(False, "please implement visit_u128")

    def visit_i8(self):
        """Return the type value for signed, 8-bit integer"""
        _assert(False, "please implement visit_i8")

    def visit_i16(self):
        """Return the type value for signed, 16-bit integer"""
        _assert(False, "please implement visit_i16")

    def visit_i32(self):
        """Return the type value for signed, 32-bit integer"""
        _assert(False, "please implement visit_i32")

    def visit_i64(self):
        """Return the type value for signed, 64-bit integer"""
        _assert(False, "please implement visit_i64")

    def visit_i128(self):
        """Return the type value for signed, 128-bit integer"""
        _assert(False, "please implement visit_i128")

    def visit_f32(self):
        """Return the type value for 32-bit floating point"""
        _assert(False, "please implement visit_f32")

    def visit_f64(self):
        """Return the type value for 64-bit floating point"""
        _assert(False, "please implement visit_f64")

    def visit_f128(self):
        """Return the type value for 128-bit floating point"""
        _assert(False, "please implement visit_f128")

class TyyamlParser:
    visitor: TyyamlVisitor

    def __init__(self, visitor, import_named):
        self.import_named = import_named
        self.visitor = visitor

    def parse_tyyaml(self, tyyaml):
        """Convert a type YAML to a type info. The exact value returned is frontend-specific"""
        _assert(isinstance(tyyaml, list), f"Invalid type YAML (not a list): {tyyaml}")
        _assert(len(tyyaml) > 0, f"Invalid type YAML (empty list): {tyyaml}")
        base = self._parse_tyyaml_base(tyyaml[0])
        return self._parse_tyyaml_recur(tyyaml[1:], base)

    def _parse_tyyaml_recur(self, tyyaml, base):
        """Convert a type YAML to a tinfo_t recursively"""
        if not tyyaml:
            # done
            return base
        spec = tyyaml[0]
        # pointer
        if spec == "*": 
            tinfo = self.visitor.visit_pointer(base)
            _assert(tinfo is not None, f"Failed to create pointer type: {tyyaml}")
            return self._parse_tyyaml_recur(tyyaml[1:], tinfo)
        # array
        if isinstance(spec, list) and len(spec) == 1 and isinstance(spec[0], int):
            tinfo = self.visitor.visit_array(base, spec[0])
            _assert(tinfo is not None, f"Failed to create array type: {tyyaml}")
            return self._parse_tyyaml_recur(tyyaml[1:], tinfo)
        # subroutine
        if spec == "()":
            args = tyyaml[1]
            func = self.visitor.visit_subroutine_start(base) # base is return type
            _assert(func is not None, f"Failed to create function type at visit_subroutine_start: {tyyaml}")
            funcargs = self._visit_tyyaml_funcargs(func, args)
            self.visitor.visit_subroutine_args(func, funcargs)
            tinfo = self.visitor.visit_subroutine_end(func)
            _assert(tinfo is not None, f"Failed to create function type: {tyyaml}")
            return self._parse_tyyaml_recur(tyyaml[2:], tinfo)
        # PTMF
        if spec == "(ptmf)":
            # PTMF are generated as THISTYPE_ptmf in the data
            # we ignore the base type generated previously
            name_t = tyyaml[1]
            _assert(
                isinstance(name_t, list) and len(name_t) == 1 and isinstance(name_t[0], str), 
                f"Invalid type YAML (expected PTMF class name): {tyyaml}"
            )
            name_t = name_t[0]
            _assert(name_t.startswith("\"") and name_t.endswith("\""), f"Invalid PTMF class name: {name_t}")
            name_t = name_t[1:-1] + "_ptmf"
            base = self._parse_tyyaml_base(f"\"{name_t}\"")
            # 0 - ptmf, 1 - class type, 2 - args, start from 3
            return self._parse_tyyaml_recur(tyyaml[3:], base)
        raise RuntimeError(f"Unknown type spec: {spec}")

    def _visit_tyyaml_funcargs(self, func, args):
        _assert(isinstance(args, list), f"Invalid type YAML (not a list of args): {args}")
        args = [self.parse_tyyaml(arg) for arg in args]
        funcargs = []
        for arg in args:
            funcarg = self.visitor.visit_function_arg(func, arg)
            _assert(funcarg is not None, f"Failed to create function arg type")
            funcargs.append(funcarg)
        return funcargs

    def _parse_tyyaml_base(self, ident):
        """Convert a base type YAML to a type value"""
        _assert(isinstance(ident, str), f"Invalid base type YAML (None)")
        if ident.startswith("\"") and ident.endswith("\""):
            name = self.visitor.visit_name(ident[1:-1])
            _assert(name, f"Invalid name: {ident[1:-1]}")
            self.import_named(name) # Ensure the type is imported before we reference it
            tinfo = self.visitor.visit_named(name)
            _assert(tinfo is not None, f"Failed to get type by name: {name}")
            return tinfo
        # base types
        tinfo = None
        if ident == "void":
            tinfo = self.visitor.visit_void()
        elif ident == "bool":
            tinfo = self.visitor.visit_bool()
        elif ident == "u8":
            tinfo = self.visitor.visit_u8()
        elif ident == "u16":
            tinfo = self.visitor.visit_u16()
        elif ident == "u32":
            tinfo = self.visitor.visit_u32()
        elif ident == "u64":
            tinfo = self.visitor.visit_u64()
        elif ident == "u128":
            tinfo = self.visitor.visit_u128()
        elif ident == "i8":
            tinfo = self.visitor.visit_i8()
        elif ident == "i16":
            tinfo = self.visitor.visit_i16()
        elif ident == "i32":
            tinfo = self.visitor.visit_i32()
        elif ident == "i64":
            tinfo = self.visitor.visit_i64()
        elif ident == "i128":
            tinfo = self.visitor.visit_i128()
        elif ident == "f32":
            tinfo = self.visitor.visit_f32()
        elif ident == "f64":
            tinfo = self.visitor.visit_f64()
        elif ident == "f128":
            tinfo = self.visitor.visit_f128()
        else:
            raise RuntimeError(f"Unknown base type: {ident} (<- this should have quotes if it's a named type)")
        _assert(tinfo is not None, f"Failed to get base type: {ident}")
        return tinfo