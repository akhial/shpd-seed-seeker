"""Regression for merging local sparse indices and counts exceeding u16."""
from pathlib import Path
import struct
import tempfile
import unittest

from merge_floors import FEATURES, FIXED, HEADER, ROWS, Shard, merge


def fixture(path, width, samples, counts, pairs):
    code = "<H" if width == 2 else "<I"
    data = bytearray((b"FLP3" if width == 2 else b"FLP4") + struct.pack("<I", samples))
    data.extend(bytes(ROWS * 4 + FIXED * width))
    for index in range(ROWS):
        struct.pack_into("<I", data, 8 + index * 4, len(data))
        marginal = counts + [0] * (FEATURES - len(counts)) if index == 0 else [0] * FEATURES
        active = [i for i, c in enumerate(marginal) if 0 < c < samples]
        for c in [samples, *marginal]:
            data.extend(struct.pack(code, c))
        data.extend(active.index(i) if i in active else 255 for i in range(FEATURES))
        for a, room in enumerate(active):
            for other in active[:a]:
                data.extend(struct.pack(code, pairs[(room, other)]))
    path.write_bytes(data)


class MergeTest(unittest.TestCase):
    def test_sparse_indices_constants_and_wide_counts(self):
        with tempfile.TemporaryDirectory() as directory:
            a, b, output = [Path(directory) / name for name in ("a", "b", "output")]
            # Feature zero is constant only in a, changing every following
            # sparse index. The merged sample and marginals overflow u16.
            fixture(a, 2, 60000, [60000, 30000, 20000], {(2, 1): 12000})
            fixture(b, 4, 70000, [35000, 40000, 30000], {(1, 0): 20000, (2, 0): 15000, (2, 1): 22000})
            output.write_bytes(merge([a, b]))
            result = Shard(output)
            samples, counts, pairs = result.row(0)
            self.assertEqual(result.samples, 130000)
            self.assertEqual(samples, 130000)
            self.assertEqual(counts[:3], [95000, 70000, 50000])
            self.assertEqual(list(pairs[:3]), [50000, 35000, 34000])
            self.assertEqual(result.count(HEADER), 0)


if __name__ == "__main__":
    unittest.main()
