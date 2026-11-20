"""Carry engine function names from one teardown.exe build to another.

usage:
  python -I tools/port_names.py --old <teardown.exe of a build with a table> --new <teardown.exe of the new build>
                                [--expect OLD:NEW ...] [--write]

The game's code is encrypted on disk (SteamStub). What is not encrypted: the exception table
(.pdata: every function's start and end) and the unwind info each entry points to (the shape of
the function's prologue: pushes, stack size, frame register). Each function gets a signature
(size + unwind codes); the two builds' signature lists, both in address order, are aligned with
difflib.

When both inputs are copies saved from the running game by the MCP tool save_game_code (code
decrypted; recognised by sections at file offset == RVA), the signature also includes a hash of
the function's code with the bytes that move between builds masked out (call/jump targets,
RIP-relative data references), so look-alike functions with different code no longer match.
Save each build's copy while that build is installed: an old build cannot be run later. A named function is carried over only when it sits inside an
aligned run of at least --min-run identical signatures, so a lone look-alike is not trusted.

--expect OLD:NEW checks known pairs: the same return address in both builds, e.g. from two crash
reports with the same stack (callstack.txt). Each must map to an address inside the new function
at the same offset, or the port is reported as wrong.

Tables live in server/names/<exe name>-<stamp>.tsv. --write saves the new table and regenerates
server/src/name_tables.rs; rebuild the server afterwards. Old exe copies: every crash report in
%LOCALAPPDATA%/Teardown/crash/ keeps the teardown.exe that crashed.
"""
import argparse, bisect, datetime, difflib, os, re, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, '..'))
NAMES = os.path.join(REPO, 'server', 'names')


def mask_code(b):
    """Code bytes with rel32 call/jump targets and RIP-relative displacements zeroed. A linear
    scan, not a disassembler: it can mis-step inside an instruction, but identically for
    identical code, which is all a signature needs."""
    b = bytearray(b)
    i, n = 0, len(b)
    while i < n:
        c = b[i]
        if c in (0xE8, 0xE9) and i + 5 <= n:  # call / jmp rel32
            b[i + 1:i + 5] = b'\0\0\0\0'
            i += 5
        elif c == 0x0F and i + 6 <= n and 0x80 <= b[i + 1] <= 0x8F:  # jcc rel32
            b[i + 2:i + 6] = b'\0\0\0\0'
            i += 6
        elif i > 0 and (c & 0xC7) == 0x05 and i + 5 <= n:  # ModRM: [rip + disp32]
            b[i + 1:i + 5] = b'\0\0\0\0'
            i += 5
        else:
            i += 1
    return bytes(b)


def read_pe(path):
    d = open(path, 'rb').read()
    pe = struct.unpack_from('<I', d, 0x3c)[0]
    stamp = struct.unpack_from('<I', d, pe + 8)[0]
    nsec = struct.unpack_from('<H', d, pe + 6)[0]
    opt_size = struct.unpack_from('<H', d, pe + 20)[0]
    opt = pe + 24
    exc_rva, exc_size = struct.unpack_from('<II', d, opt + 112 + 3 * 8)
    secs = []
    for i in range(nsec):
        s = opt + opt_size + i * 40
        vsize, va, rsize, rptr = struct.unpack_from('<IIII', d, s + 8)
        secs.append((va, max(vsize, rsize), rptr))

    def off(rva):
        for va, size, rptr in secs:
            if va <= rva < va + size:
                return rptr + rva - va
        return None

    # A save_game_code copy has its sections at file offset == RVA, code decrypted.
    is_dump = all(rptr == va for va, size, rptr in secs if size)
    base = off(exc_rva)
    funcs = []
    for i in range(exc_size // 12):
        b, e, u = struct.unpack_from('<III', d, base + 12 * i)
        if b == 0 and e == 0:
            continue
        sig = None
        uo = off(u & ~1)
        if uo is not None and uo + 4 <= len(d):
            flags_ver, prolog, ncodes, frame = d[uo], d[uo + 1], d[uo + 2], d[uo + 3]
            codes = d[uo + 4:uo + 4 + 2 * ncodes]
            # chained entries point at their parent by RVA, which moves between builds: leave it out
            sig = (e - b, flags_ver, prolog, ncodes, frame, codes)
        code = hash(mask_code(d[b:e])) if is_dump and e <= len(d) else None
        funcs.append((b, e, sig, code))
    funcs.sort()
    return stamp, funcs, is_dump


def load_table(path):
    names = {}
    for line in open(path, encoding='utf-8'):
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        parts = line.split('\t')
        names[int(parts[0], 16)] = parts[1]
    return names


def exe_name_of(path):
    """teardown.exe for a game exe or a crash report's copy; save_game_code copies are named
    <exe stem>-<stamp>.exe."""
    n = os.path.basename(path).lower()
    m = re.fullmatch(r'(.+)-[0-9a-f]{8}\.exe', n)
    return (m.group(1) + '.exe') if m else n


def table_path(exe_name, stamp):
    return os.path.join(NAMES, f'{exe_name.lower()}-{stamp:08x}.tsv')


def regenerate_rust():
    rows = []
    for f in sorted(os.listdir(NAMES)):
        m = re.fullmatch(r'(.+\.exe)-([0-9a-f]{8})\.tsv', f)
        if m:
            rows.append(f'    ("{m.group(1)}", 0x{m.group(2)}, include_str!("../names/{f}")),')
    out = os.path.join(REPO, 'server', 'src', 'name_tables.rs')
    with open(out, 'w', encoding='utf-8', newline='\n') as fh:
        fh.write('//! Generated by tools/port_names.py from server/names/*.tsv: (exe file name, PE timestamp, table).\n')
        fh.write('pub const TABLES: &[(&str, u32, &str)] = &[\n' + '\n'.join(rows) + '\n];\n')
    print(f'wrote {out} ({len(rows)} tables)')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--old', required=True, help='exe of a build that has a table in server/names')
    ap.add_argument('--new', required=True, help='exe of the build to make a table for')
    ap.add_argument('--old-table', help='default: server/names/<old exe name>-<old stamp>.tsv (or teardown.exe-...)')
    ap.add_argument('--expect', action='append', default=[], help='OLD:NEW hex return addresses known to match')
    ap.add_argument('--min-run', type=int, default=4, help='shortest aligned run of functions to trust (default 4)')
    ap.add_argument('--write', action='store_true', help='save the table and regenerate server/src/name_tables.rs')
    ap.add_argument('--show', action='store_true', help='print each ported name with its new address')
    a = ap.parse_args()

    old_stamp, old, old_dump = read_pe(a.old)
    new_stamp, new, new_dump = read_pe(a.new)
    use_code = old_dump and new_dump
    print('comparing code (both are save_game_code copies)' if use_code else
          'comparing function tables only (code needs save_game_code copies of both builds)')
    new_name = exe_name_of(a.new)
    table = a.old_table
    if not table:
        for n in (exe_name_of(a.old), 'teardown.exe', 'teardown_modtest.exe'):
            if os.path.isfile(table_path(n, old_stamp)):
                table = table_path(n, old_stamp)
                break
    if not table or not os.path.isfile(table):
        sys.exit(f'no name table for the old build {old_stamp:#010x} in {NAMES}')
    names = load_table(table)
    print(f'old {a.old}: build {old_stamp:#010x}, {len(old)} functions, {len(names)} names ({os.path.basename(table)})')
    print(f'new {a.new}: build {new_stamp:#010x}, {len(new)} functions')
    if old_stamp == new_stamp:
        print('same build: nothing to port')

    old_sigs = [repr((f[2], f[3] if use_code else None)) for f in old]
    new_sigs = [repr((f[2], f[3] if use_code else None)) for f in new]
    sm = difflib.SequenceMatcher(None, old_sigs, new_sigs, autojunk=False)
    index, how = {}, {}  # old index -> new index, and how it was matched
    for blk in sm.get_matching_blocks():
        if blk.size >= a.min_run:
            for k in range(blk.size):
                index[blk.a + k] = blk.b + k
                how[blk.a + k] = 'same'
    aligned = len(index)
    # Second pass, between aligned runs: a changed function keeps its place. Where the stretch
    # has as many functions on both sides, pair them in order; otherwise pair functions whose
    # prologue shape (unwind codes without the size) is unique in the stretch on both sides.
    anchors = sorted(index.items())
    shape = lambda f: repr(f[2][1:]) if f[2] else None
    for (oa, na), (ob, nb) in zip([(-1, -1)] + anchors, anchors + [(len(old), len(new))]):
        oi, ni = list(range(oa + 1, ob)), list(range(na + 1, nb))
        if not oi or not ni:
            continue
        if len(oi) == len(ni) and len(oi) <= 8:
            for x, y in zip(oi, ni):
                index[x], how[x] = y, 'changed (same place)'
            continue
        os_, ns_ = {}, {}
        for x in oi:
            os_.setdefault(shape(old[x]), []).append(x)
        for y in ni:
            ns_.setdefault(shape(new[y]), []).append(y)
        for k, xs in os_.items():
            if k and len(xs) == 1 and len(ns_.get(k, [])) == 1:
                index[xs[0]], how[xs[0]] = ns_[k][0], 'changed (same prologue)'
    print(f'aligned {aligned} of {len(old)} functions ({100 * aligned / max(1, len(old)):.1f}%), '
          f'{len(index) - aligned} more paired in the second pass')

    starts = [f[0] for f in old]
    out, missing = [], []
    for rva, name in sorted(names.items()):
        i = bisect.bisect_left(starts, rva)
        if i < len(old) and old[i][0] == rva and i in index:
            out.append((new[index[i]][0], name, how[i]))
        else:
            missing.append((rva, name))
    print(f'ported {len(out)} of {len(names)} names')
    if a.show:
        for rva, name, h in sorted(out, key=lambda x: x[1]):
            print(f'  {name}\t{rva:#x}\t{h}')
    for rva, name in missing:
        print(f'  not ported: {name} ({rva:#x})')

    bad = 0
    new_starts = [f[0] for f in new]
    for pair in a.expect:
        o, n = (int(x, 16) for x in pair.split(':'))
        i = bisect.bisect_right(starts, o) - 1
        ok = False
        if i >= 0 and o < old[i][1] and i in index:
            j = index[i]
            got = new[j][0] + (o - old[i][0])
            inside = new[j][0] <= n < new[j][1]
            ok = got == n or (how[i] != 'same' and inside)
            note = 'OK' if got == n else ('OK, inside the paired function (changed code)' if ok else 'WRONG')
            print(f'  expect {o:#x} -> {n:#x}: function {old[i][0]:#x} -> {new[j][0]:#x} ({how[i]}) {note}')
        else:
            print(f'  expect {o:#x} -> {n:#x}: its function was not aligned')
        bad += not ok
    if a.expect:
        print(f'{len(a.expect) - bad} of {len(a.expect)} expected pairs match')

    if a.write:
        if bad:
            sys.exit('not written: an expected pair did not match')
        os.makedirs(NAMES, exist_ok=True)
        path = table_path(new_name, new_stamp)
        with open(path, 'w', encoding='utf-8', newline='\n') as fh:
            fh.write(f'# {new_name} build {new_stamp:#010x}: ported from {os.path.basename(table)} by tools/port_names.py '
                     f'on {datetime.date.today()} ({len(out)} of {len(names)} names; aligned runs >= {a.min_run}; '
                     f'{"code compared" if use_code else "function tables only"})\n')
            for rva, name, h in sorted(out):
                fh.write(f'{rva:#08x}\t{name}\t{h}\n')
        print(f'wrote {path}')
        regenerate_rust()


if __name__ == '__main__':
    main()
