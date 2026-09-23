#!/usr/bin/env python3
"""Pack calibrate_wand_repeats JSON reports into a directly readable WDR2 table.

Missing profiles keep zero sample counts and use the analytical fallback.
Each profile accepts one report; duplicate profiles are rejected.
"""
import argparse
import json
import struct
from pathlib import Path

PROFILES = ["none", "mimic_tooth", "parchment_scrap", "rat_skull", "exotic_crystals", "mossy_clump", "trap_mechanism", "cracked_spyglass"]
BANDS = 24
ROWS = 2 * BANDS * 4 * 24
HEADER = 4 + len(PROFILES) * 4


def pack(reports):
    output = bytearray(HEADER + len(PROFILES) * ROWS * 4)
    output[:4] = b"WDR2"
    seen = set()
    for report in reports:
        profile = PROFILES.index(report["profile"])
        if profile in seen:
            raise ValueError("duplicate profile")
        seen.add(profile)
        samples = report["samples"]
        counts = report["counts"]
        if not isinstance(samples, int) or not 0 < samples <= 0xFFFFFFFF // 13:
            raise ValueError("invalid sample count")
        if len(counts) != ROWS:
            raise ValueError("invalid table dimensions")
        for group in range(2):
            for band in range(BANDS):
                for copies in range(4):
                    start = ((group * BANDS + band) * 4 + copies) * 24
                    for depth, count in enumerate(counts[start:start + 24]):
                        if not isinstance(count, int) or not 0 <= count <= samples * (10 if group == 0 else 3):
                            raise ValueError("invalid presence count")
                        if depth > 0 and count < counts[start + depth - 1]:
                            raise ValueError("presence decreases with depth")
                        if copies > 0 and count > counts[start + depth - 24]:
                            raise ValueError("more copies increase presence")
                        if ((0 < band < 4) or (band >= 4 and (band - 4) % 5 > 0)) and count > counts[start + depth - 4 * 24]:
                            raise ValueError("higher upgrades increase presence")
        struct.pack_into("<I", output, 4 + profile * 4, samples)
        struct.pack_into(f"<{ROWS}I", output, HEADER + profile * ROWS * 4, *counts)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("reports", nargs="+", type=Path)
    args = parser.parse_args()
    if args.output.resolve() in [p.resolve() for p in args.reports]:
        parser.error("output must differ from input reports")
    result = pack([json.loads(p.read_text()) for p in args.reports])
    args.output.write_bytes(result)
    print(f"{len(result)} bytes, {len(args.reports)} profiles")


if __name__ == "__main__":
    main()
