#!/usr/bin/env python3
"""Uploads dist/<TITLE_ID>/ to /data/homebrew/<TITLE_ID>/ on the console.

Everything else first, then eboot.bin, then sce_sys/param.json last, so a half-finished deploy
never looks like a title. Files whose size already matches are skipped (not eboot.bin, whose
size the server reports decoded). With TELEMETRY=ip:port, writes launch.json so the title
streams its log there. Close the title on the console before deploying.
"""
import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(__file__))
import ps5ftp  # noqa: E402


def main() -> int:
    title_id = os.environ.get("TITLE_ID") or sys.exit("Set TITLE_ID (see .env.example).")
    root = os.path.join(os.path.dirname(__file__), "..", "dist", title_id)
    if not os.path.isfile(os.path.join(root, "eboot.bin")):
        sys.exit(f"{root} has no eboot.bin: run task package first.")
    files = []
    for base, _, names in os.walk(root):
        for n in names:
            files.append(os.path.relpath(os.path.join(base, n), root).replace(os.sep, "/"))
    order = sorted(f for f in files if f not in ("eboot.bin", "sce_sys/param.json"))
    order += ["eboot.bin", "sce_sys/param.json"]
    ftp = ps5ftp.connect()
    remote_root = ps5ftp.title_dir(title_id)
    for rel in order:
        local = os.path.join(root, rel)
        remote = f"{remote_root}/{rel}"
        if rel != "eboot.bin" and ps5ftp.size(ftp, remote) == os.path.getsize(local):
            print(f"  same  {rel}")
            continue
        ps5ftp.upload(ftp, local, remote)
        print(f"  sent  {rel}")
    telemetry = os.environ.get("TELEMETRY", "")
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump({"telemetry": telemetry, "telemetry_level": os.environ.get("TELEMETRY_LEVEL", "info")}, f)
    ps5ftp.upload(ftp, f.name, f"{remote_root}/launch.json")
    os.unlink(f.name)
    print(f"deployed {title_id}; a new title folder needs up to 45 s before it launches")
    ftp.quit()
    return 0


if __name__ == "__main__":
    sys.exit(main())
