# qstep.py
import gdb
import re

ADDR_RE = re.compile(r"0x[0-9a-fA-F]+")


def _clean_line(line: str) -> str:
    line = line.strip()
    if line.startswith("=>"):
        line = line[2:].strip()
    return line


def _parse_insn(line: str):
    """
    Returns: (addr:int, mnem:str, op:str)
    line example:
      "0x7fffb7dd71a9:\tendbr64"
      "0x7fffb7dd71d6:\tcall   0x7fffb7dd70a0"
    """
    line = _clean_line(line)
    m = ADDR_RE.search(line)
    if not m:
        raise RuntimeError("failed to parse address from: " + line)
    addr = int(m.group(0), 16)

    # split at first ':'
    if ":" in line:
        _, rest = line.split(":", 1)
    else:
        rest = line[m.end() :]
    rest = rest.strip()

    if not rest:
        return addr, "", ""

    parts = rest.split(None, 1)
    mnem = parts[0]
    op = parts[1].strip() if len(parts) > 1 else ""
    return addr, mnem, op


def _disas2():
    out = gdb.execute("x/2i $pc", to_string=True)
    lines = [l for l in out.splitlines() if l.strip()]
    if len(lines) < 2:
        raise RuntimeError("x/2i $pc returned < 2 lines:\n" + out)
    return lines[0], lines[1]


def _addr_from_operand(op: str):
    """
    Very simple: pick the first 0x... in operand.
    Works for direct call/jcc/jmp like: 'call 0x....', 'je 0x....'
    """
    m = ADDR_RE.search(op)
    if not m:
        return None
    return int(m.group(0), 16)


def _read_u64(expr: str) -> int:
    v = gdb.parse_and_eval(expr)
    return int(v)


def _set_temp_bps(addrs):
    bps = []
    for a in addrs:
        if a is None:
            continue
        spec = "*{}".format(hex(a))
        try:
            bp = gdb.Breakpoint(spec, temporary=True, internal=True)
        except TypeError:
            # older gdb may not support internal=
            bp = gdb.Breakpoint(spec, temporary=True)
        bps.append(bp)
    return bps


def _delete_bps(bps):
    for bp in bps:
        try:
            bp.delete()
        except Exception:
            pass


def _step_like(next_over_call: bool):
    l0, l1 = _disas2()
    cur_addr, mnem, op = _parse_insn(l0)
    nxt_addr, _, _ = _parse_insn(l1)  # linear fall-through address

    mnem_l = (mnem or "").lower()

    addrs = []
    # ret: next is [rsp]
    if mnem_l.startswith("ret"):
        try:
            ra = _read_u64("(unsigned long long)*(unsigned long long*)$rsp")
            addrs = [ra]
        except Exception:
            # fallback: try fall-through (may not be correct, but avoids hard fail)
            addrs = [nxt_addr]

    # call: ssi -> into target, nni -> over to fall-through
    elif mnem_l.startswith("call"):
        if next_over_call:
            addrs = [nxt_addr]
        else:
            tgt = _addr_from_operand(op)
            addrs = [tgt] if tgt is not None else [nxt_addr]

    # jmp/jcc: break on possible next executed insn(s)
    elif mnem_l.startswith("j"):
        tgt = _addr_from_operand(op)
        if mnem_l in ("jmp", "jmpq"):
            # unconditional jump: only target
            addrs = [tgt] if tgt is not None else [nxt_addr]
        else:
            # conditional: either fall-through or target
            addrs = [nxt_addr]
            if tgt is not None:
                addrs.append(tgt)

    else:
        # normal linear instruction
        addrs = [nxt_addr]

    # place temp breakpoints and continue
    bps = _set_temp_bps(addrs)
    try:
        gdb.execute("continue")
    finally:
        # delete any leftover temp bps (e.g., branch not taken side)
        _delete_bps(bps)


class SSI(gdb.Command):
    """ssi [N] : step one instruction (best-effort) without using stepi/ni.
    - call: step into (break at call target if direct)
    - jcc: break at either fall-through or target
    - ret: break at *(u64*)$rsp
    """

    def __init__(self):
        super(SSI, self).__init__("ssi", gdb.COMMAND_USER)

    def invoke(self, arg, from_tty):
        arg = arg.strip()
        n = int(arg, 0) if arg else 1
        for _ in range(n):
            _step_like(next_over_call=False)


class NNI(gdb.Command):
    """nni [N] : next-instruction (best-effort) without using stepi/ni.
    - call: step over (break at fall-through)
    - jcc/ret: same behavior as ssi
    """

    def __init__(self):
        super(NNI, self).__init__("nni", gdb.COMMAND_USER)

    def invoke(self, arg, from_tty):
        arg = arg.strip()
        n = int(arg, 0) if arg else 1
        for _ in range(n):
            _step_like(next_over_call=True)


SSI()
NNI()
print("[qstep] loaded: commands = ssi, nni")
