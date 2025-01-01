from common import _assert

class NameAndType:
    name = "" # may be empty
    tyyaml = [] # may be empty

class FunctionDef:
    name = "" # may be empty
    tyyaml = [] # may be empty
    args: list[NameAndType] = [] # NameAndType[]

def _make_function(name, tyyaml, args):
    func = FunctionDef()
    func.name = name
    if tyyaml:
        func.tyyaml = tyyaml
    if args:
        func.args = args
    return func

def _make_name_type(name, tyyaml):
    nt = NameAndType()
    nt.name = name
    if tyyaml:
        nt.tyyaml = tyyaml
    return nt

class AddrImportVisitor:
    """Visitor for importing address symbol"""

    # Order:
    #   Functions:
    #     - rettype, or dummy_rettype, or old_rettype
    #     - argument or dummy_argument, or old_argument for each argument
    #     - finish
    #   Data:
    #     - data_type
    #     - finish

    def visit_data_type(self, tinfo):
        """Set the type of the data symbol"""
        _assert(False, "please implement visit_data_type")

    def visit_rettype(self, tinfo):
        """Set the return type of the function"""
        _assert(False, "please implement visit_rettype")

    def visit_dummy_rettype(self):
        """Set the return type of a function to a dummy type"""
        _assert(False, "please implement visit_dummy_rettype")

    def visit_old_rettype(self, func_obj):
        """Set the return type of a function to the old return type"""
        _assert(False, "please implement visit_old_rettype")

    def visit_argument(self, name: str, tinfo):
        """Visit an argument of the function with name and type"""
        _assert(False, "please implement visit_argument")

    def visit_dummy_argument(self, name: str):
        """Visit an argument of the function with only the name"""
        _assert(False, "please implement visit_dummy_argument")

    def visit_old_argument(self, name: str, i: int, func_obj):
        """Visit an argument of the function the name and the old type"""
        _assert(False, "please implement visit_old_argument")

    def finish(self):
        """Set the type and done"""
        _assert(False, "please implement finish")
