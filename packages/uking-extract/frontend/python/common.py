def _assert(cond, msg):
    if not cond:
        raise RuntimeError(msg)