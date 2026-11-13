#!/usr/bin/env python3
"""Names the frames of a crash: symbolize.py LOG ELF.

Reads `rip`/frame addresses from a klog crash record or the panic hook's backtrace, rebases them
on the "xotext" load base the crash record names (0x400000 when absent), and asks
llvm-addr2line about the unstripped ELF CI keeps (dist/<TITLE_ID>.llvm-pie.elf).
"""
import re
import shutil
import subprocess
import sys


def main() -> int:
    if len(sys.argv) != 3:
        sys.exit("usage: symbolize.py LOG ELF")
    text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
    base_match = re.search(r"xotext:\s*([0-9a-fA-F]{16}):", text)
    base = int(base_match.group(1), 16) if base_match else 0x400000
    addrs = []
    # A crash record prints bare 16-digit hex after `rip:`, `fault address:` and on each
    # backtrace line; the rest of a klog is other processes' noise. The panic hook prints 0x.
    if "# backtrace:" in text:
        pattern = r"^#\s+(?:rip:\s+|fault address:\s+)?([0-9a-fA-F]{16})\b"
    else:
        pattern = r"0x([0-9a-fA-F]{6,16})"
    for m in re.finditer(pattern, text, re.MULTILINE):
        a = int(m.group(1), 16)
        if a >= base and a - base < 0x1000_0000 and a not in addrs:
            addrs.append(a)
    tool = shutil.which("llvm-addr2line") or shutil.which("addr2line")
    if not tool or not addrs:
        sys.exit("no llvm-addr2line on PATH" if not tool else "no addresses in the log")
    args = [tool, "-f", "-C", "-i", "-e", sys.argv[2]] + [hex(a - base) for a in addrs]
    out = subprocess.run(args, capture_output=True, text=True, check=False).stdout.splitlines()
    for a, (fn, loc) in zip(addrs, zip(out[0::2], out[1::2])):
        print(f"{a:#x}  {fn}  {loc}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
