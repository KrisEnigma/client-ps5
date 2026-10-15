#!/usr/bin/env python3
"""Queues a scripted run: test-run.py menu | stream HOST [SECONDS].

Writes test-run.json into the title folder; the next launch reads and deletes it, runs the
scenario alone, writes a bundle into verdict/ and closes. Then run fetch-bundle.py.
"""
import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(__file__))
import ps5ftp  # noqa: E402


def main() -> int:
    title_id = os.environ.get("TITLE_ID") or sys.exit("Set TITLE_ID (see .env.example).")
    if len(sys.argv) < 2 or sys.argv[1] not in ("menu", "stream"):
        sys.exit("usage: test-run.py menu | stream HOST [SECONDS]")
    run = {"scenario": sys.argv[1]}
    if run["scenario"] == "stream":
        if len(sys.argv) < 3:
            sys.exit("stream needs the host's address, paired from the console first")
        run["host"] = sys.argv[2]
        run["seconds"] = int(sys.argv[3]) if len(sys.argv) > 3 else 60
        run["captures"] = [5, max(6, run["seconds"] - 5)]
    ftp = ps5ftp.connect()
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(run, f)
    ps5ftp.upload(ftp, f.name, f"{ps5ftp.title_dir(title_id)}/test-run.json")
    os.unlink(f.name)
    ftp.quit()
    print(f"queued {run}; launch the title, wait for it to close, then run task fetch-bundle")
    return 0


if __name__ == "__main__":
    sys.exit(main())
