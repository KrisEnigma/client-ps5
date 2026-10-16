#!/usr/bin/env python3
"""The probe title's files over FTP: probe.py host ADDRESS [PIN] | fetch.

`host` writes probe.json into the probe's folder so its network checks reach that host; the PIN
pairs it on the first launch. `fetch` copies probe.log to bundles/<TITLE_ID>/, one file per call
(each launch rewrites the log), and prints the verdict.
"""
import ftplib
import json
import os
import re
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(__file__))
import ps5ftp  # noqa: E402

USAGE = "usage: probe.py host ADDRESS [PIN] | fetch"


def put_host(ftp: ftplib.FTP, remote: str, args: list) -> None:
    config = {"host": args[0]}
    if len(args) > 1:
        config["pin"] = args[1]
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(config, f)
    ps5ftp.upload(ftp, f.name, f"{remote}/probe.json")
    os.unlink(f.name)
    print(f"probe.json names {args[0]}")


def verdict(text: str) -> str:
    """One line for the tester to paste: the counts, or the check the probe died in."""
    results = re.findall(r"^probe (\d\d) (ok|fail|skip) (\S+?):", text, re.M)
    if "probe end:" not in text:
        started = re.findall(r"^probe (\d\d) start (\S+)", text, re.M)
        last = f"check {started[-1][0]} {started[-1][1]}" if started else "start-up"
        return f"STOPPED in {last}"
    counts = {s: sum(1 for _, r, _ in results if r == s) for s in ("ok", "fail", "skip")}
    failed = ", ".join(name for _, r, name in results if r == "fail")
    head = "ok" if not counts["fail"] else f"FAILED ({failed})"
    return f"{head}: {counts['ok']} ok, {counts['fail']} fail, {counts['skip']} skip"


def fetch(ftp: ftplib.FTP, remote: str, title_id: str) -> None:
    local_dir = os.path.join(os.path.dirname(__file__), "..", "bundles", title_id)
    os.makedirs(local_dir, exist_ok=True)
    local = os.path.normpath(os.path.join(local_dir, f"probe-{time.strftime('%Y%m%d-%H%M%S')}.log"))
    chunks = []
    try:
        ftp.retrbinary(f"RETR {remote}/probe.log", chunks.append)
    except ftplib.error_perm:
        sys.exit("No probe.log on the console yet: launch the probe and wait for it to close.")
    with open(local, "wb") as f:
        f.write(b"".join(chunks))
    print(verdict(b"".join(chunks).decode("utf-8", "replace")))
    print(f"log: {local}")


def main() -> int:
    title_id = os.environ.get("TITLE_ID") or sys.exit("Set TITLE_ID (see .env.example).")
    args = sys.argv[1:]
    if not ((args[:1] == ["host"] and len(args) in (2, 3)) or args == ["fetch"]):
        sys.exit(USAGE)
    ftp = ps5ftp.connect()
    remote = ps5ftp.title_dir(title_id)
    if args[0] == "host":
        put_host(ftp, remote, args[1:])
    else:
        fetch(ftp, remote, title_id)
    ftp.quit()
    return 0


if __name__ == "__main__":
    sys.exit(main())
