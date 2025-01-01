from common import Frontend

class Heuristics:
    frontend: Frontend

    def __init__(self, frontend):
        self.frontend = frontend

    def can_ovrd_member(self, m1: str, m2: str):
        """If m1 can override m2 as a member name"""
        r = _prefers_a_by(m1, m2, self.frontend.get_member_heuristics())
        if r is not None:
            return r
        return self.frontend.member_fallback_heuristic(m1, m2)

    def can_ovrd_symbol(self, m1: str, m2: str):
        """If m1 can override m2 as a symbol (function or data) name"""
        r = _prefers_a_by(m1, m2, self.frontend.get_symbol_heuristics())
        if r is not None:
            return r
        return self.frontend.symbol_fallback_heuristic(m1, m2)


def _prefers_a_by(a, b, heuristics):
    """
    Compare A and B using the given heuristics
    Return True if A is preferred over B, False if B is preferred over A, None if no preference
    The heuristics can return a bool or a number. 
    For bools, True is more preferred.
    For numbers, greater is more preferred.
    """
    for h in heuristics:
        h_a = h(a)
        h_b = h(b)
        if isinstance(h_a, bool):
            if h_a and not h_b:
                return True
            if h_b and not h_a:
                return False 
        else:
            if h_a > h_b:
                return True
            if h_b > h_a:
                return False
    return None