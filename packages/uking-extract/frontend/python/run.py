from frontend import Frontend
from printutil import set_verbose, infoln
from type_importer import TypeImporter
from addr_importer import AddrImporter
import time
import traceback

START_TIME = time.time()
def _fmt_time(seconds):
    seconds = int(seconds)
    h, r = divmod(seconds, 3600)  # 3600 seconds in an hour
    m, s = divmod(r, 60)  # 60 seconds in a minute
    return f"{h:02d}:{m:02d}:{s:02d}"


def _done():
    t = _fmt_time(time.time() - START_TIME)
    infoln(f"Done in {t}")

def run_with_frontend(verbose, frontend: Frontend, process_imports: function):
    try:
        set_verbose(verbose)
        ti = TypeImporter(frontend)
        ai = AddrImporter(ti)
        process_imports(ti, ai)
        _done()
    except Exception as e:
        print(f"[uking-extract] Error: {e}")
        traceback.print_exception(e)
        _done()
        infoln("[uking-extract] There were errors!")