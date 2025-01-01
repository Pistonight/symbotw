from common import _assert

class EnumDef:
    size = 0
    enumerators = [] # (name, int)[]

    def __init__(self, size, enumerators):
        self.size = size
        self.enumerators = enumerators

    def __eq__(self, other):
        if isinstance(other, self.__class__):
            return self.size == other.size and self.enumerators == other.enumerators
        return False
    
    def __ne__(self, other):
        return not self.__eq__(other)

class EnumImportVisitor:
    """Interface for visitor state when importing an EnumDef"""

    # Order: 
    # - size
    # - enumerator (for each enumerator)
    # - finish
    def visit_size(self, size):
        """Set the size in bytes for the enum type"""
        _assert(False, "please implement visit_size")

    def visit_enumerator(self, enumerator_name, value):
        """Set an enumerator"""
        _assert(False, "please implement visit_enumerator")

    def finish(self):
        """Finish the enum and set the type"""
        _assert(False, "please implement finish")