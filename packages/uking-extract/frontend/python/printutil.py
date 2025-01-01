VERBOSE = 0
def set_verbose(level):
    global VERBOSE
    VERBOSE = level

DEPTH = 0
class PrintScope:
    def __enter__(self):
        global DEPTH
        DEPTH += 1
    def __exit__(self):
        global DEPTH
        DEPTH -= 1

def _print(level, arg):
    if level > VERBOSE:
        return
    padding = "  " * DEPTH
    if level == 1:
        prefix = "-- "
    elif level == 2:
        prefix = "--> "
    else:
        prefix = ""
    print(f"[uking-extract]{padding}{prefix}", arg)

def infoln(arg):
    _print(0, arg)

def verboseln(arg):
    _print(1, arg)

def veryverboseln(arg):
    _print(2, arg)