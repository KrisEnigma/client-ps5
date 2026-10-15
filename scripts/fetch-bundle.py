#!/usr/bin/env python3
"""Fetches the newest test-run bundle into bundles/<TITLE_ID>/<stamp>/ and prints its verdict."""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import ps5ftp  # noqa: E402


def main() -> int:
    title_id = os.environ.get("TITLE_ID") or sys.exit("Set TITLE_ID (see .env.example).")
    ftp = ps5ftp.connect()
    verdicts = f"{ps5ftp.title_dir(title_id)}/verdict"
    stamps = sorted((s for s in ps5ftp.listing(ftp, verdicts) if s.isdigit()), key=int)
    if not stamps:
        sys.exit("No bundle on the console yet: queue a run with task test-run, launch the title, let it close.")
    stamp = stamps[-1]
    local = os.path.join(os.path.dirname(__file__), "..", "bundles", title_id, stamp)
    os.makedirs(local, exist_ok=True)
    for name in ps5ftp.listing(ftp, f"{verdicts}/{stamp}"):
        with open(os.path.join(local, name), "wb") as f:
            ftp.retrbinary(f"RETR {verdicts}/{stamp}/{name}", f.write)
    ftp.quit()
    verdict = os.path.join(local, "verdict.json")
    if os.path.isfile(verdict):
        v = json.load(open(verdict, encoding="utf-8"))
        print(f"{'ok' if v.get('ok') else 'FAILED'}: {v.get('scenario')} — {v.get('detail')} ({v.get('build')})")
    print(f"bundle: {os.path.normpath(local)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
