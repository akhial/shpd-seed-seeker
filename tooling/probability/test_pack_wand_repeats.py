import struct
import unittest

from pack_wand_repeats import HEADER, ROWS, pack


class WandPackingTests(unittest.TestCase):
    def test_profile_order_and_integer_counts_are_preserved(self):
        reports = [
            {"profile": "trap_mechanism", "samples": 100, "counts": [23] * ROWS},
            {"profile": "none", "samples": 200, "counts": [47] * ROWS},
        ]
        data = pack(reports)
        self.assertEqual(data[:4], b"WDR2")
        self.assertEqual(len(data), HEADER + 8 * ROWS * 4)
        self.assertEqual(struct.unpack_from("<8I", data, 4), (200, 0, 0, 0, 0, 0, 100, 0))
        self.assertEqual(struct.unpack_from("<I", data, HEADER)[0], 47)
        self.assertEqual(struct.unpack_from("<I", data, HEADER + 6 * ROWS * 4)[0], 23)

    def test_duplicate_profiles_and_impossible_counts_are_rejected(self):
        report = {"profile": "none", "samples": 100, "counts": [23] * ROWS}
        with self.assertRaisesRegex(ValueError, "duplicate profile"):
            pack([report, report])
        for index, value in [(0, 1001), (1, 22), (24, 24), (96, 24), (0, -1)]:
            invalid = {**report, "counts": list(report["counts"])}
            invalid["counts"][index] = value
            with self.assertRaises(ValueError):
                pack([invalid])


if __name__ == "__main__":
    unittest.main()
