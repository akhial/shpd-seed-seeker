#!/usr/bin/env python3
"""Sum FLP3/FLP4 floor calibration shards into an exact-count FLP4 table.

Inputs must cover disjoint seed streams of the same generator/catalog version.
Sparse row indices are local to each shard; reconstruct intersections by room
identity before merging, including variables constant in only some shards.
"""
import argparse
from array import array
from pathlib import Path
import struct

PROFILES, FLOORS, GROUPS, FEATURES = 8, 20, 4, 96
ROWS = PROFILES * FLOORS * GROUPS
FIXED = PROFILES * FLOORS * 8 + PROFILES * 190 * (FEATURES + 64)
HEADER = 8 + ROWS * 4


def triangle(a, b):
    high, low = max(a, b), min(a, b)
    return high * (high - 1) // 2 + low


class Shard:
    def __init__(self, path):
        self.data = path.read_bytes()
        assert self.data[:4] in (b"FLP3", b"FLP4"), path
        self.width = 2 if self.data[:4] == b"FLP3" else 4
        self.code = "<H" if self.width == 2 else "<I"
        self.samples = struct.unpack_from("<I", self.data, 4)[0]

    def count(self, offset):
        return struct.unpack_from(self.code, self.data, offset)[0]

    def row(self, index):
        start = struct.unpack_from("<I", self.data, 8 + index * 4)[0]
        samples = self.count(start)
        counts = [self.count(start + (i + 1) * self.width) for i in range(FEATURES)]
        mapping = start + (FEATURES + 1) * self.width
        pairs = array("I")
        for a in range(FEATURES):
            for b in range(a):
                ca, cb = counts[a], counts[b]
                if ca in (0, samples) or cb in (0, samples):
                    pairs.append(min(ca, cb))
                else:
                    position = triangle(self.data[mapping + a], self.data[mapping + b])
                    pairs.append(self.count(mapping + FEATURES + position * self.width))
        return samples, counts, pairs


def merge(paths):
    assert len(set(p.resolve() for p in paths)) == len(paths), "duplicate shard"
    shards = [Shard(path) for path in paths]
    samples = sum(shard.samples for shard in shards)
    assert 0 < samples <= 0xFFFFFFFF
    data = bytearray(b"FLP4" + struct.pack("<I", samples) + bytes(ROWS * 4))
    for index in range(FIXED):
        data.extend(struct.pack("<I", sum(s.count(HEADER + index * s.width) for s in shards)))
    for index in range(ROWS):
        rows = [s.row(index) for s in shards]
        total = sum(row[0] for row in rows)
        counts = [sum(row[1][i] for row in rows) for i in range(FEATURES)]
        active = [i for i, count in enumerate(counts) if 0 < count < total]
        struct.pack_into("<I", data, 8 + index * 4, len(data))
        data.extend(struct.pack("<" + "I" * (FEATURES + 1), total, *counts))
        data.extend(active.index(i) if i in active else 255 for i in range(FEATURES))
        for a, room in enumerate(active):
            for other in active[:a]:
                pair = sum(row[2][triangle(room, other)] for row in rows)
                assert max(0, counts[room] + counts[other] - total) <= pair <= min(counts[room], counts[other])
                data.extend(struct.pack("<I", pair))
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("shards", type=Path, nargs="+")
    args = parser.parse_args()
    assert args.output.resolve() not in [p.resolve() for p in args.shards]
    data = merge(args.shards)
    args.output.write_bytes(data)
    print(f"{len(args.shards)} shards, {struct.unpack_from('<I', data, 4)[0]} samples/profile, {len(data)} bytes")


if __name__ == "__main__":
    main()
