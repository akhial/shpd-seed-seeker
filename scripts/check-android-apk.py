#!/usr/bin/env python3
"""Check the packaged native architectures and report APK size in CI."""

import argparse
from pathlib import Path
from zipfile import ZipFile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    parser.add_argument("abis", help="Space-separated expected architectures")
    args = parser.parse_args()
    expected = set(args.abis.split())
    if not expected or not expected <= {"arm64-v8a", "x86_64"}:
        parser.error("expected arm64-v8a and/or x86_64")
    with ZipFile(args.apk) as apk:
        libraries = [
            entry for entry in apk.infolist()
            if entry.filename.startswith("lib/") and entry.filename.endswith(".so")
        ]
        actual = {entry.filename.split("/")[1] for entry in libraries}
        engines = {
            entry.filename.split("/")[1] for entry in libraries
            if entry.filename.endswith("/libshpd_seedfinder.so")
        }
        if actual != expected or engines != expected:
            raise SystemExit(
                f"Expected {sorted(expected)}; packaged libraries: {sorted(actual)}, "
                f"engines: {sorted(engines)}"
            )
        for entry in libraries:
            print(
                f"{entry.filename}: {entry.file_size:,} bytes "
                f"({entry.compress_size:,} in APK)"
            )
    size = args.apk.stat().st_size
    print(f"{args.apk}: {size:,} bytes ({size / 2**20:.2f} MiB)")


if __name__ == "__main__":
    main()
