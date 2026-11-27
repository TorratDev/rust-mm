"""Tests for tools/ov_read.py on a synthetic ROM (no retail data).

Run: python3 -m unittest discover -s tools/tests -v
"""

import contextlib
import io
import json
import os
import struct
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))

import nds_extract_code as x  # noqa: E402
import ov_read  # noqa: E402
import synth_nds  # noqa: E402


class OvReadTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        rom, _ = synth_nds.build()
        files, manifest, _ = x.extract(rom, allow_unknown=True)
        cls.tmp = tempfile.TemporaryDirectory()
        cls.code = cls.tmp.name
        for name, data in files.items():
            with open(os.path.join(cls.code, name), "wb") as f:
                f.write(data)
        with open(os.path.join(cls.code, "manifest.json"), "w", encoding="utf-8") as f:
            json.dump(manifest, f)
        cls.files = files
        cls.ov_a = next(o for o in manifest["overlays"]
                        if int(o["ram_address"], 16) == synth_nds.OV_RAM_A)
        cls.ov_label = f"ov_{cls.ov_a['id']:04d}"

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def read(self, spec, count=1):
        _, spaces = ov_read.load_spaces(self.code)
        return ov_read.read_words(self.code, spaces, spec, count)

    def test_overlay_pool_word_by_name(self):
        # The synthetic overlay stores a pointer to its own data (ram + 0x14) at +0x10.
        addr = synth_nds.OV_RAM_A + 0x10
        self.assertEqual(self.read(f"{self.ov_label}::{addr:08X}"),
                         [(self.ov_label, addr, synth_nds.OV_RAM_A + 0x14)])

    def test_prefix_and_0x_and_case_accepted(self):
        addr = synth_nds.OV_RAM_A + 0x10
        self.assertEqual(self.read(f"{self.ov_label}::0x{addr:08x}")[0][2], synth_nds.OV_RAM_A + 0x14)

    def test_count_reads_consecutive_words(self):
        base = synth_nds.OV_RAM_A
        got = self.read(f"{self.ov_label}::{base:08X}", 2)
        data = self.files[self.ov_a["file"]]
        self.assertEqual([w for _, _, w in got],
                         [struct.unpack_from("<I", data, 0)[0], struct.unpack_from("<I", data, 4)[0]])
        self.assertEqual([a for _, a, _ in got], [base, base + 4])

    def test_arm9_word_by_bare_address(self):
        got = self.read(f"{synth_nds.ARM9_FUNC:08X}")
        self.assertEqual(got[0][0], "arm9")
        self.assertEqual(got[0][2], struct.unpack_from("<I", self.files["arm9.bin"],
                                                       synth_nds.ARM9_FUNC - synth_nds.ARM9_RAM)[0])

    def test_bare_address_in_two_spaces_needs_a_name(self):
        # Overlays are mapped on top of each other in one region; the bare address is ambiguous.
        with self.assertRaisesRegex(ov_read.ReadError, "name the one you mean"):
            self.read(f"{synth_nds.OV_RAM_A + 0x14:08X}")

    def test_wrong_overlay_name_refused(self):
        with self.assertRaisesRegex(ov_read.ReadError, "no space named"):
            self.read(f"ov_9999::{synth_nds.OV_RAM_A:08X}")
        with self.assertRaisesRegex(ov_read.ReadError, "below|no extracted"):
            self.read(f"{self.ov_label}::{synth_nds.OV_RAM_A - 4:08X}")

    def test_read_past_end_refused(self):
        end = synth_nds.OV_RAM_A + self.ov_a["size"]
        with self.assertRaisesRegex(ov_read.ReadError, "run past the end"):
            self.read(f"{self.ov_label}::{end - 2:08X}")

    def test_address_outside_program_refused(self):
        with self.assertRaisesRegex(ov_read.ReadError, "no extracted space"):
            self.read("00000010")

    def test_bad_input_refused(self):
        with self.assertRaises(ov_read.ReadError):
            self.read("xyz")
        with self.assertRaises(ov_read.ReadError):
            self.read(f"{self.ov_label}::{synth_nds.OV_RAM_A:08X}", 0)

    def test_cli_prints_line(self):
        out = io.StringIO()
        addr = synth_nds.OV_RAM_A + 0x10
        with contextlib.redirect_stdout(out):
            rc = ov_read.main([f"{self.ov_label}::{addr:08X}", "--code", self.code])
        self.assertEqual(rc, 0)
        self.assertEqual(out.getvalue(), f"{self.ov_label}::{addr:08X} = 0x{synth_nds.OV_RAM_A + 0x14:08X}\n")

    def test_cli_missing_manifest_fails(self):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            rc = ov_read.main(["00000000", "--code", tempfile.gettempdir() + "/no-such-code-dir"])
        self.assertEqual(rc, 1)
        self.assertIn("manifest.json", err.getvalue())


if __name__ == "__main__":
    unittest.main()
