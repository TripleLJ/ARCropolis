#!/usr/bin/env python3
"""Turn tools/addresses.toml into src/offsets.rs.

Each entry carries one address per game version it's known for (`addr` for 13.0.4, `addr_1305` for
13.0.5, and so on for any future `addr_<digits>` key). This script finds a short run of bytes around
the 13.0.4 address that only appears once in every dump present, so the crate can find the same code
on any version instead of hardcoding the offset. When a function's bytes moved enough that no single
pattern covers every version, it builds one pattern per version instead, and the crate tries them in
order at boot.

  ./patterns.py --dump                    pull .text out of the open Ghidra program into target/
  ./patterns.py --dump --program main_1305   same, for another open program
  ./patterns.py                           build the patterns and write src/offsets.rs
  ./patterns.py --check --program main_1304  re-run every pattern over that dump and print what it resolves to

The dump is only used to test that a pattern is unique, the game does the same search at boot with
crates/patterns.
"""

import argparse
import http.client
import json
import os
import re
import socket
import subprocess
import sys
import urllib.parse

IMAGE_BASE = 0x7100000000
PROGRAM = "main_1304"
VERSION_TAG = "1304"
CHUNK = 4 * 1024 * 1024

TOOLS = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(TOOLS)
ADDRESSES = os.path.join(TOOLS, "addresses.toml")
TARGET = os.path.join(CRATE, "target")
DUMP = os.path.join(TARGET, "main_1304.text")


def use_program(name):
    """Point every Ghidra call and the dump path at another open program, main_1305 for instance"""
    global PROGRAM, DUMP, VERSION_TAG
    PROGRAM = name
    DUMP = os.path.join(TARGET, name + ".text")
    VERSION_TAG = name.rsplit("_", 1)[-1]


def other_dumps():
    """Every other version's dump in target/, a pattern has to be unique in all of them too"""
    texts = []
    for name in sorted(os.listdir(TARGET)) if os.path.isdir(TARGET) else []:
        path = os.path.join(TARGET, name)
        if name.endswith(".text") and path != DUMP:
            with open(path, "rb") as handle:
                texts.append((name, handle.read()))
    return texts


EXTRA_TEXTS = []


def unique(text, bytes_string):
    """Exactly one hit in the dump being built from and in every other version's dump"""
    if len(count_matches(text, bytes_string)) != 1:
        return False
    return all(len(count_matches(other, bytes_string)) == 1 for _name, other in EXTRA_TEXTS)
OFFSETS_RS = os.path.join(CRATE, "src", "offsets.rs")

# how many instructions a window may hold, and how far past a function's end it may run as a last resort
MIN_WINDOW = 5
MAX_WINDOW = 16
PAST_END = 32


# ---------------------------------------------------------------------------- ghidra


class UnixConnection(http.client.HTTPConnection):
    """http.client over the unix socket the Ghidra plugin listens on"""

    def __init__(self, path):
        super().__init__("ghidra")
        self.socket_path = path

    def connect(self):
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(300)
        sock.connect(self.socket_path)
        self.sock = sock


def socket_path():
    tmp = os.environ.get("TMPDIR", "/tmp")
    folder = os.path.join(tmp, "ghidra-mcp-%s" % os.environ.get("USER", ""))
    socks = [os.path.join(folder, name) for name in os.listdir(folder) if name.endswith(".sock")]
    if not socks:
        sys.exit("no ghidra socket in %s, is Ghidra open?" % folder)
    return max(socks, key=os.path.getmtime)


def gget(route, **params):
    params.setdefault("program", PROGRAM)
    query = urllib.parse.urlencode(params)
    conn = UnixConnection(socket_path())
    try:
        conn.request("GET", "%s?%s" % (route, query))
        return conn.getresponse().read()
    finally:
        conn.close()


def read_memory(addr, length):
    """The response carries the bytes twice, as a json array and as hex. Only the hex is worth parsing."""
    body = gget("/read_memory", address=hex(addr), length=length)
    match = re.search(rb'"hex":"([0-9a-f]*)"', body)
    if not match:
        sys.exit("read_memory failed at %x: %s" % (addr, body[:200]))
    return bytes.fromhex(match.group(1).decode())


def segments():
    out = {}
    for line in gget("/list_segments", limit=100).decode(errors="replace").splitlines():
        match = re.match(r"^(\S+):\s+([0-9a-fA-F]+)\s+-\s+([0-9a-fA-F]+)\s*$", line)
        if match:
            out[match.group(1)] = (int(match.group(2), 16), int(match.group(3), 16))
    return out


def function_body(addr):
    """Entry and last address of the function containing addr, or None when Ghidra has no function there"""
    text = gget("/get_function_by_address", address=hex(addr)).decode(errors="replace")
    match = re.search(r"Body:\s*([0-9a-fA-F]+)\s*-\s*([0-9a-fA-F]+)", text)
    if not match:
        return None
    return int(match.group(1), 16), int(match.group(2), 16)


PC_RELATIVE_MNEMONICS = {"adr", "adrp", "b", "bl", "cbz", "cbnz", "tbz", "tbnz", "ldr", "ldrsw", "prfm"}


def disassemble(addr, length):
    """Ghidra's own listing for a range, used to sanity check what we decode ourselves"""
    conn = UnixConnection(socket_path())
    body = json.dumps({"start_address": hex(addr), "length": length})
    try:
        conn.request("POST", "/disassemble_bytes?program=%s" % PROGRAM, body, {"Content-Type": "application/json"})
        raw = conn.getresponse().read().decode(errors="replace")
    finally:
        conn.close()

    listing = []
    for line in raw.splitlines():
        match = re.match(r"^([0-9a-fA-F]+):\s*(.*?)\s*$", line)
        if match:
            listing.append((int(match.group(1), 16), match.group(2)))
    return listing


def dump_text():
    segs = segments()
    if ".text" not in segs:
        sys.exit("no .text segment: %s" % sorted(segs))
    start = segs[".text"][0]
    # the crate searches everything between the text and rodata regions, so the plt belongs in the dump
    end = segs[".rodata"][0] if ".rodata" in segs else segs[".text"][1] + 1

    os.makedirs(TARGET, exist_ok=True)
    print("dumping %x - %x (%d MiB) to %s" % (start, end, (end - start) >> 20, DUMP))
    with open(DUMP, "wb") as out:
        addr = start
        while addr < end:
            size = min(CHUNK, end - addr)
            chunk = read_memory(addr, size)
            if len(chunk) != size:
                # the last block stops short of the next segment, pad the hole so offsets stay lined up
                print("\n  %x: %d bytes not mapped, padding" % (addr + len(chunk), size - len(chunk)))
                chunk += b"\x00" * (size - len(chunk))
            out.write(chunk)
            addr += size
            sys.stdout.write("\r  %x" % addr)
            sys.stdout.flush()
    print("\ndone, %d bytes" % os.path.getsize(DUMP))

    gitignore = os.path.join(CRATE, ".gitignore")
    if not os.path.exists(gitignore):
        with open(gitignore, "w") as out:
            out.write("target/\n")


def cross_check(text, addr, length):
    """Hold our own decoding up against Ghidra's listing, a mismatch means is_pc_relative is wrong"""
    for at, line in disassemble(addr, length):
        mnemonic = line.split()[0].lower() if line.split() else ""
        ours = is_pc_relative(insn_at(text, at - IMAGE_BASE))
        theirs = mnemonic in PC_RELATIVE_MNEMONICS or mnemonic.startswith("b.")
        # ldr and friends are only PC relative with a literal, which is the form without a bracket
        if mnemonic in ("ldr", "ldrsw", "prfm") and "[" in line:
            theirs = False
        if ours != theirs:
            print("  %x: we say pc relative=%s, ghidra says '%s'" % (at, ours, line))


def load_dump():
    if not os.path.exists(DUMP):
        sys.exit("no dump at %s, run --dump first" % DUMP)
    with open(DUMP, "rb") as handle:
        return handle.read()


# ---------------------------------------------------------------------------- aarch64


def insn_at(text, offset):
    return int.from_bytes(text[offset:offset + 4], "little")


def is_pc_relative(insn):
    """Instructions whose bytes hold a distance, so they change whenever anything around them moves"""
    if insn & 0x1F000000 == 0x10000000:  # ADR, ADRP
        return True
    if insn & 0x7C000000 == 0x14000000:  # B, BL
        return True
    if insn & 0xFF000000 == 0x54000000:  # B.cond
        return True
    if insn & 0x7E000000 == 0x34000000:  # CBZ, CBNZ
        return True
    if insn & 0x7E000000 == 0x36000000:  # TBZ, TBNZ
        return True
    if insn & 0x3B000000 == 0x18000000:  # LDR, LDRSW, PRFM with a literal
        return True
    return False


def is_adrp(insn):
    return insn & 0x9F000000 == 0x90000000


def is_ldr_unsigned(insn):
    return insn & 0x3B000000 == 0x39000000 and insn & 0x00C00000 == 0x00400000


def is_add_imm(insn):
    return insn & 0x7F800000 == 0x11000000


def adrp_page(text, offset):
    adrp = insn_at(text, offset)
    immhi = (adrp & 0x00FFFFE0) >> 3
    immlo = (adrp & 0x60000000) >> 29
    imm = (immhi | immlo) << 12
    if imm & (1 << 32):  # the immediate is signed over 33 bits
        imm -= 1 << 33
    return (offset & ~0xFFF) + imm


def ldr_imm(text, offset):
    ldr = insn_at(text, offset)
    size = (ldr & 0xC0000000) >> 30
    return ((ldr & 0x003FFC00) >> 10) << size


def add_imm(text, offset):
    return (insn_at(text, offset) & 0x003FFC00) >> 10


# ---------------------------------------------------------------------------- patterns


def pattern_bytes(text, offset, count, mask=True):
    """A lazysimd pattern for `count` instructions at `offset`, with the PC relative ones wildcarded"""
    parts = []
    for index in range(count):
        at = offset + index * 4
        insn = insn_at(text, at)
        if mask and is_pc_relative(insn):
            parts.append("?? ?? ?? ??")
        else:
            parts.append(" ".join("%02X" % byte for byte in text[at:at + 4]))
    return " ".join(parts)


def compile_pattern(bytes_string):
    regex = b""
    for token in bytes_string.split():
        regex += b"." if token == "??" else re.escape(bytes([int(token, 16)]))
    return re.compile(regex, re.DOTALL)


def count_matches(text, bytes_string, limit=2):
    """Matches anywhere in the dump, not just on instruction boundaries, which is how lazysimd scans"""
    regex = compile_pattern(bytes_string)
    found = []
    pos = 0
    while len(found) < limit:
        match = regex.search(text, pos)
        if not match:
            break
        found.append(match.start())
        pos = match.start() + 1
    return found


def trim_trailing_wildcards(text, offset, count):
    """A pattern ending in ?? tells the search nothing, drop those instructions"""
    while count > 1 and is_pc_relative(insn_at(text, offset + (count - 1) * 4)):
        count -= 1
    return count


def window_search(text, offset, starts, limit, mask, smallest, longest=MAX_WINDOW, is_unique=None):
    """First window in `starts` that only matches once, as short as it can be. Returns (pattern, start, count).

    `is_unique(bytes_string, at)` decides what counts as a hit, default is the plain cross-dump
    check. A per-version build passes its own, since it also cares about the *other* dumps.
    """
    check = is_unique or (lambda bytes_string, at: unique(text, bytes_string))
    for start in starts:
        if mask and is_pc_relative(insn_at(text, offset + start * 4)):
            continue  # a pattern starting on ?? would be dead weight
        widest = min(longest, limit - start)
        if widest < smallest:
            continue
        at = offset + start * 4

        def window(count):
            real = trim_trailing_wildcards(text, at, count) if mask else count
            return pattern_bytes(text, at, real, mask), real

        # a longer window can only match in fewer places, so the widest one says whether this start works
        if not check(window(widest)[0], at):
            continue
        for count in range(smallest, widest + 1):
            bytes_string, real = window(count)
            if check(bytes_string, at):
                return bytes_string, start, real
    return None


def build_function_pattern(text, offset, body_instructions, min_start=0, is_unique=None):
    """Look for a unique window over the function, loosening the rules until one turns up"""
    # skip past a prologue another plugin overwrites with its own hook, and past any leading
    # PC relative instruction since a pattern cannot start on a wildcard
    first = min_start
    while first < body_instructions and is_pc_relative(insn_at(text, offset + first * 4)):
        first += 1

    deep = range(first + 1, max(first + 1, body_instructions - MIN_WINDOW + 1))
    past_end = body_instructions + PAST_END
    attempts = [
        ([first], body_instructions, True, MIN_WINDOW, MAX_WINDOW, ""),
        (deep, body_instructions, True, MIN_WINDOW, MAX_WINDOW, "the top of the function is shared"),
        ([first], past_end, True, MIN_WINDOW, 2 * MAX_WINDOW, "the function is too short to tell apart on its own"),
        (range(0, first + 2), past_end, False, 4, MAX_WINDOW, "nothing is wildcarded, a plt stub only differs in its ADRP"),
    ]

    for starts, limit, mask, smallest, longest, why in attempts:
        found = window_search(text, offset, starts, limit, mask, smallest, longest, is_unique)
        if found:
            bytes_string, start, count = found
            note = "start +%d insn, %d insn window" % (start, count)
            if start + count > body_instructions:
                note += ", %d insn past the end" % (start + count - body_instructions)
            return bytes_string, -4 * start, start, count, note + (", " + why if why else "")
    return None


def build_anchor_pattern(text, anchor_offset, anchor_end, target, want_add):
    """Find the ADRP + LDR/ADD pair in the anchor that lands on `target`, pattern the bytes just before it"""
    pair = None
    for at in range(anchor_offset, anchor_end, 4):
        insn = insn_at(text, at)
        if not is_adrp(insn):
            continue
        following = insn_at(text, at + 4)
        if want_add and not is_add_imm(following):
            continue
        if not want_add and not is_ldr_unsigned(following):
            continue
        resolved = adrp_page(text, at) + (add_imm(text, at + 4) if want_add else ldr_imm(text, at + 4))
        if resolved == target:
            pair = at
            break
    if pair is None:
        return None

    for count in range(3, MAX_WINDOW + 1):
        start = pair - count * 4
        if start < anchor_offset - 4 * MAX_WINDOW or start < 0:
            break
        if is_pc_relative(insn_at(text, start)):
            continue
        bytes_string = pattern_bytes(text, start, count)
        if unique(text, bytes_string):
            return bytes_string, 4 * count, pair, count, "%d insn before it" % count

    # the code in front of the ADRP can be boilerplate shared by a whole family of functions (every
    # scene factory looks the same), so keep going over the pair itself and what follows it
    for after in range(2, MAX_WINDOW):
        for count in range(3, MAX_WINDOW + 1):
            start = pair - count * 4
            if start < anchor_offset - 4 * MAX_WINDOW or start < 0:
                break
            if is_pc_relative(insn_at(text, start)):
                continue
            total = trim_trailing_wildcards(text, start, count + after)
            bytes_string = pattern_bytes(text, start, total)
            if unique(text, bytes_string):
                return bytes_string, 4 * count, pair, count, "%d insn before it, %d after" % (count, total - count - 1)
    return None


def dump_for_version(tag):
    """The dump bytes for a version tag like '1305', and the Ghidra program name to go with it.
    None when that version hasn't been dumped yet."""
    name = "main_" + tag
    path = os.path.join(TARGET, name + ".text")
    if not os.path.exists(path):
        return None
    with open(path, "rb") as handle:
        return name, handle.read()


def entry_versions(entry):
    """Every version this entry carries an address for, tag -> address. '1304' comes from `addr`,
    everything else from an `addr_<digits>` key."""
    versions = {"1304": entry["addr"]}
    for key, value in entry.items():
        match = re.match(r"^addr_(\d+)$", key)
        if match:
            versions[match.group(1)] = value
    return versions


def per_version_check(own_text, own_offset, others_base):
    """The tight preference: a window is good when it's unique in its own dump, and in every
    other dump it either never shows up or shows up exactly once, right where that version's own
    address for this entry says it should (same bytes happened to survive into that version too).
    Doesn't work for something like a plt stub, where only one word differs between versions and
    the rest of the stubs all look alike, so build_function_pattern_per_version falls back to
    own_dump_unique for those."""
    def is_unique(bytes_string, at):
        if len(count_matches(own_text, bytes_string)) != 1:
            return False
        for other_text, other_offset in others_base:
            hits = count_matches(other_text, bytes_string, limit=2)
            if not hits:
                continue
            expected = other_offset + (at - own_offset) if other_offset is not None else None
            if len(hits) == 1 and hits[0] == expected:
                continue
            return False
        return True
    return is_unique


def own_dump_unique(own_text):
    """The loose fallback: only has to be unique in its own dump. Whether the list as a whole is
    safe to ship gets decided afterwards by actually simulating find_any over every dump, this
    check alone says nothing about the other versions."""
    def is_unique(bytes_string, at):
        return len(count_matches(own_text, bytes_string)) == 1
    return is_unique


def simulate_find_any(patterns, text):
    """What the crate's find_any would resolve `patterns` to over this dump: first pattern in
    the list that matches exactly once, tried in order. Mirrors resolve()'s rule but doesn't
    need the adrp math since this only ever runs for fn-kind entries."""
    for bytes_string, adjust in patterns:
        hits = count_matches(text, bytes_string, limit=2)
        if len(hits) == 1:
            return hits[0] + adjust
    return None


def build_function_pattern_per_version(entry):
    """When no single pattern covers every version, build one per version instead. Only versions
    with both a known address and a dump on disk take part. If even one of those can't get a
    pattern at all, the whole entry still falls back to a normal failure, we don't want a
    half-covered list silently missing a version at boot.

    Each candidate tries the tight per_version_check first (unique everywhere, cheap to reason
    about), and only drops to own_dump_unique when that fails. A pattern built the loose way can
    spuriously match once in another version's dump at the wrong spot (a plt stub next to this
    one, say), but that's harmless as long as an earlier pattern in the list already claims that
    dump first, which is exactly what the final simulate_find_any pass checks: the whole ordered
    list, oldest version first, has to resolve every dump to its own known address.

    Returns a list of (bytes, adjust) in version order, or None.
    """
    versions = entry_versions(entry)
    dumps = {}
    for tag, addr in versions.items():
        found = dump_for_version(tag)
        if found:
            program_name, text = found
            dumps[tag] = (addr, program_name, text)

    if len(dumps) < 2:
        return None  # nothing to tell it apart from

    order = sorted(dumps, key=int)  # oldest version first, so find_any prefers it at boot
    original_program = PROGRAM
    patterns = []
    try:
        for tag in order:
            addr, program_name, own_text = dumps[tag]
            use_program(program_name)
            own_offset = addr - IMAGE_BASE
            body = function_body(addr)
            size = (body[1] + 1 - addr) // 4 if body else MAX_WINDOW
            cross_check(own_text, addr, min(64, size * 4))
            min_start = entry.get("skip", 0) // 4

            others_base = [(t, a - IMAGE_BASE) for other_tag, (a, _p, t) in dumps.items() if other_tag != tag]
            built = build_function_pattern(own_text, own_offset, max(size, MIN_WINDOW), min_start, per_version_check(own_text, own_offset, others_base))
            preference = "tight"
            if not built:
                built = build_function_pattern(own_text, own_offset, max(size, MIN_WINDOW), min_start, own_dump_unique(own_text))
                preference = "loose"
            if not built:
                return None
            bytes_string, adjust, _start, _count, note = built
            patterns.append((bytes_string, adjust, "%s: %s (%s)" % (program_name, note, preference)))
    finally:
        use_program(original_program)

    # a pattern that only had to be unique in its own dump might resolve some other version to
    # the wrong place, so replay the actual boot algorithm before trusting the list
    ordered = [(bytes_string, adjust) for bytes_string, adjust, _note in patterns]
    for tag in order:
        addr, _program_name, text = dumps[tag]
        if simulate_find_any(ordered, text) != addr - IMAGE_BASE:
            return None

    return patterns


# ---------------------------------------------------------------------------- addresses.toml


def parse_addresses():
    """Small hand parser so the section comments survive into offsets.rs"""
    entries = []
    comment = []
    entry = None
    with open(ADDRESSES) as handle:
        for line in handle:
            line = line.rstrip("\n")
            stripped = line.strip()
            if stripped.startswith("#"):
                comment.append(stripped.lstrip("#").strip())
                continue
            if not stripped:
                comment = []
                continue
            if stripped == "[[entry]]":
                entry = {"section": " ".join(comment) if comment else None}
                entries.append(entry)
                comment = []
                continue
            match = re.match(r'^(\w[\w-]*)\s*=\s*(.*)$', stripped)
            if match and entry is not None:
                key, value = match.group(1), match.group(2).strip()
                if value.startswith('"'):
                    entry[key] = value.split('"')[1]
                else:
                    entry[key] = int(value.split("#")[0].strip(), 0)
    return entries


# ---------------------------------------------------------------------------- output


HEADER = "// Generated by tools/patterns.py from tools/addresses.toml, edit those and re-run instead of this file.\n"


def static_name(entry):
    return entry["name"].upper() + ("_PATTERN" if entry["kind"] == "fn" else "_ANCHOR")


def write_offsets_rs(entries):
    out = [HEADER, "\nuse std::sync::LazyLock;\n\nuse patterns::Pattern;\n"]

    section = None
    for entry in entries:
        if entry["section"] and entry["section"] != section:
            section = entry["section"]
            out.append("\n// %s\n" % section)
        out.append("static %s: &[Pattern] = &[\n" % static_name(entry))
        for bytes_string, adjust in entry["patterns"]:
            out.append('    Pattern { bytes: "%s", adjust: %d },\n' % (bytes_string, adjust))
        out.append("];\n")

    out.append("\n#[derive(serde::Serialize, serde::Deserialize)]\npub struct Offsets {\n")
    for entry in entries:
        out.append("    pub %s: usize,\n" % entry["name"])
    out.append("}\n")

    for entry in entries:
        out.append("\npub fn %s() -> usize {\n    OFFSETS.%s\n}\n" % (entry["name"], entry["name"]))

    out.append('\nstatic OFFSETS: LazyLock<Offsets> = LazyLock::new(|| patterns::load_or_build("arcadia_offsets.toml", Offsets::new));\n')

    # each field resolved into a local, so a boot that misses one names every field it could not find
    out.append("\nimpl Offsets {\n    fn new() -> Option<Self> {\n        let text = patterns::text();\n        let mut missing = Vec::new();\n\n")
    for entry in entries:
        call = {"fn": "find_any", "adrp_ldr": "adrp_ldr_any", "adrp_add": "adrp_add_any"}[entry["kind"]]
        out.append("        let %s = patterns::%s(text, %s);\n" % (entry["name"], call, static_name(entry)))
        out.append("        if %s.is_none() {\n            missing.push(\"%s\");\n        }\n" % (entry["name"], entry["name"]))
    out.append("\n        if !missing.is_empty() {\n            error!(\"arcadia could not resolve these offsets on this game version: {:?}\", missing);\n            return None;\n        }\n\n")
    out.append("        Some(Self {\n")
    for entry in entries:
        out.append("            %s: %s.unwrap(),\n" % (entry["name"], entry["name"]))
    out.append("        })\n    }\n}\n")

    with open(OFFSETS_RS, "w") as handle:
        handle.write("".join(out))

    try:
        subprocess.run(["rustfmt", "--edition", "2021", OFFSETS_RS], check=True)
    except (OSError, subprocess.CalledProcessError) as err:
        print("rustfmt skipped: %s" % err)


# ---------------------------------------------------------------------------- commands


def resolve(text, entry):
    """What the crate would resolve this entry to at boot: same first-pattern-that-matches-
    exactly-once rule as find_any in the patterns crate. Returns (offset_or_None, hits per pattern)."""
    hit_counts = []
    for pattern_bytes, adjust in entry["patterns"]:
        if pattern_bytes == "TODO":
            # the entry never got a real pattern, nothing to search for
            hit_counts.append(0)
            continue
        hits = count_matches(text, pattern_bytes, limit=2)
        hit_counts.append(len(hits))
        if len(hits) == 1:
            offset = hits[0] + adjust
            if entry["kind"] == "adrp_ldr":
                offset = adrp_page(text, offset) + ldr_imm(text, offset + 4)
            elif entry["kind"] == "adrp_add":
                offset = adrp_page(text, offset) + add_imm(text, offset + 4)
            return offset, hit_counts
    return None, hit_counts


def build(entries):
    global EXTRA_TEXTS
    text = load_dump()
    EXTRA_TEXTS = other_dumps()
    for name, _other in EXTRA_TEXTS:
        print("also keeping every pattern unique in %s" % name)
    failed = []

    for entry in entries:
        offset = entry["addr"] - IMAGE_BASE
        if entry["kind"] == "fn":
            body = function_body(entry["addr"])
            size = (body[1] + 1 - entry["addr"]) // 4 if body else MAX_WINDOW
            cross_check(text, entry["addr"], min(64, size * 4))
            built = build_function_pattern(text, offset, max(size, MIN_WINDOW), entry.get("skip", 0) // 4)
            if built:
                bytes_string, adjust, _start, _count, note = built
                entry["patterns"] = [(bytes_string, adjust)]
                entry["note"] = note
            else:
                per_version = build_function_pattern_per_version(entry)
                if per_version:
                    entry["patterns"] = [(bytes_string, adjust) for bytes_string, adjust, _note in per_version]
                    entry["note"] = "per version: " + "; ".join(note for _b, _a, note in per_version)
                else:
                    entry["patterns"] = [("TODO", 0)]
                    entry["note"] = "NOT UNIQUE"
                    failed.append(entry["name"])
        else:
            anchor = entry.get("anchor")
            if not isinstance(anchor, int):
                sys.exit("entry '%s' still has a TBD anchor" % entry["name"])
            body = function_body(anchor)
            if not body:
                sys.exit("no function at anchor %x for '%s'" % (anchor, entry["name"]))
            cross_check(text, body[0], min(256, body[1] + 1 - body[0]))
            built = build_anchor_pattern(text, body[0] - IMAGE_BASE, body[1] + 1 - IMAGE_BASE, offset, entry["kind"] == "adrp_add")
            if built:
                bytes_string, adjust, pair, _count, how = built
                entry["patterns"] = [(bytes_string, adjust)]
                entry["note"] = "adrp at %x, %s" % (pair + IMAGE_BASE, how)
            else:
                # no per-version fallback here yet, that'd need an anchor address per version too
                entry["patterns"] = [("TODO", 0)]
                entry["note"] = "NO ADJACENT PAIR OR NOT UNIQUE"
                failed.append(entry["name"])

        if len(entry["patterns"]) == 1:
            print("%-44s %-9s adjust %-5d %s" % (entry["name"], entry["kind"], entry["patterns"][0][1], entry["note"]))
        else:
            print("%-44s %-9s %d patterns  %s" % (entry["name"], entry["kind"], len(entry["patterns"]), entry["note"]))

    if failed:
        print("\n!! hand pick a pattern for: %s" % ", ".join(failed))

    write_offsets_rs(entries)
    print("\nwrote %s" % OFFSETS_RS)
    return failed


def parse_existing_patterns():
    """Read the pattern lists back out of offsets.rs so --check can run on its own"""
    with open(OFFSETS_RS) as handle:
        source = handle.read()
    found = {}
    for match in re.finditer(r"static (\w+): &\[Pattern\] = &\[(.*?)\];", source, re.DOTALL):
        pairs = re.findall(r'bytes: "([^"]*)",\s*adjust: (-?\d+)', match.group(2))
        found[match.group(1)] = [(bytes_string, int(adjust)) for bytes_string, adjust in pairs]
    return found


def check(entries):
    text = load_dump()
    existing = parse_existing_patterns()
    bad = 0
    for entry in entries:
        name = static_name(entry)
        if name not in existing:
            print("%-44s MISSING from offsets.rs" % entry["name"])
            bad += 1
            continue
        entry["patterns"] = existing[name]
        key = "addr" if VERSION_TAG == "1304" else "addr_" + VERSION_TAG
        expected = entry[key] - IMAGE_BASE if key in entry else None
        offset, hit_counts = resolve(text, entry)
        if offset is None:
            print("%-44s MISMATCH  hits per pattern: %s" % (entry["name"], hit_counts))
            bad += 1
        elif expected is None:
            print("%-44s %#-11x UNVERIFIED, no %s in addresses.toml" % (entry["name"], offset + IMAGE_BASE, key))
        elif offset != expected:
            print("%-44s MISMATCH  got %#x, expected %#x" % (entry["name"], offset, expected))
            bad += 1
        else:
            print("%-44s %#-11x expected %#-11x OK" % (entry["name"], offset, expected))
    print("\n%d entries, %d bad" % (len(entries), bad))
    return bad


def write_hardcoded(entries):
    """offsets.rs as plain constants for one game version, no search and no cache at boot"""
    key = "addr" if VERSION_TAG == "1304" else "addr_" + VERSION_TAG
    version = "%s.%s.%s" % (VERSION_TAG[0:2], VERSION_TAG[2], VERSION_TAG[3])
    out = ["// Generated by tools/patterns.py --hardcode --program %s from tools/addresses.toml, edit those and re-run instead of this file.\n" % PROGRAM,
           "// Every value is a %s offset from the start of the game's main module.\n\n" % version]
    missing = [entry["name"] for entry in entries if key not in entry]
    if missing:
        sys.exit("no %s for: %s" % (key, ", ".join(missing)))
    for entry in entries:
        out.append("pub fn %s() -> usize {\n    %#x\n}\n\n" % (entry["name"], entry[key] - IMAGE_BASE))
    with open(OFFSETS_RS, "w") as handle:
        handle.write("".join(out).rstrip("\n") + "\n")
    try:
        subprocess.run(["rustfmt", "--edition", "2021", OFFSETS_RS], check=True)
    except (OSError, subprocess.CalledProcessError) as err:
        print("rustfmt skipped: %s" % err)
    print("wrote %s with %d hardcoded %s offsets" % (OFFSETS_RS, len(entries), version))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hardcode", action="store_true", help="write offsets.rs as plain constants for --program's version instead of patterns")
    parser.add_argument("--dump", action="store_true", help="pull .text out of Ghidra first")
    parser.add_argument("--check", action="store_true", help="re-run the patterns in offsets.rs over the dump")
    parser.add_argument("--program", default=PROGRAM, help="which open Ghidra program and dump to use")
    args = parser.parse_args()
    use_program(args.program)

    if args.dump:
        dump_text()
        return 0

    entries = parse_addresses()
    if args.hardcode:
        write_hardcoded(entries)
        return 0
    if args.check:
        return 1 if check(entries) else 0
    return 1 if build(entries) else 0


if __name__ == "__main__":
    sys.exit(main())
