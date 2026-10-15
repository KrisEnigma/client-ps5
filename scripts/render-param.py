#!/usr/bin/env python3
"""Fills packaging/sce_sys/param.json for one title id: render-param.py TEMPLATE TITLE_ID NAME.

attribute3 0x80040 declares 119.88 Hz output (and the newer VRR bit); a console that rejects it
for VRR keeps HFR. downloadDataSize 256 gives the title /download0. The concept id is the title
id's five digits, which is what the console checks it against.
"""
import json
import re
import sys


def main() -> int:
    template, title_id, name = sys.argv[1], sys.argv[2], sys.argv[3]
    if not re.fullmatch(r"PPSA\d{5}", title_id):
        print(f"title id {title_id!r} is not PPSA12345-shaped", file=sys.stderr)
        return 2
    text = open(template, encoding="utf-8").read()
    text = text.replace("@TITLE_ID@", title_id).replace("@CONCEPT_ID@", title_id[4:])
    text = text.replace("@TITLE_NAME@", name)
    doc = json.loads(text)
    if not re.fullmatch(r"[A-Z]{2}\d{4}-PPSA\d{5}_00-[A-Z0-9]{16}", doc["contentId"]):
        print(f"contentId {doc['contentId']!r} is malformed", file=sys.stderr)
        return 2
    print(json.dumps(doc, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
