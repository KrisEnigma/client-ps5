#!/usr/bin/env python3
"""Compares a console frame with the host's: compare.py CONSOLE.png HOST.png [TOLERANCE].

Both are scaled to 480x270 and compared by mean absolute difference per channel; the default
tolerance is 4.5/255 (the PS5_VulkanTemplate figure). Exit 1 when they differ more.
"""
import sys

from PIL import Image, ImageChops, ImageStat


def main() -> int:
    if len(sys.argv) < 3:
        sys.exit("usage: compare.py CONSOLE.png HOST.png [TOLERANCE]")
    tolerance = float(sys.argv[3]) if len(sys.argv) > 3 else 4.5
    a, b = (Image.open(p).convert("RGB").resize((480, 270), Image.BILINEAR) for p in sys.argv[1:3])
    diff = ImageStat.Stat(ImageChops.difference(a, b)).mean
    mean = sum(diff) / len(diff)
    print(f"mean difference {mean:.2f}/255 (tolerance {tolerance})")
    return 0 if mean <= tolerance else 1


if __name__ == "__main__":
    sys.exit(main())
