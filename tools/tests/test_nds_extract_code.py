"""Tests for tools/nds_extract_code.py on synthetic ROMs (no retail data).

Run: python3 -m unittest discover -s tools/tests -v
"""

import contextlib
import io
import json
import os
import random
import struct
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))

import nds_extract_code as x  # noqa: E402
import synth_nds  # noqa: E402


def quiet(fn, *a, **kw):
    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        return fn(*a, **kw), err.getvalue()


class BlzTests(unittest.TestCase):
    def test_round_trip_random(self):
        rng = random.Random(1234)
        for size in (64, 100, 1000, 5000):
            for prefix in (0, 8, 32):
                # Compressible but not trivial: a random motif, repeated with mutations.
                motif = bytes(rng.randrange(256) for _ in range(rng.randrange(3, 40)))
                data = bytearray((motif * (size // len(motif) + 1))[:size])
                for _ in range(size // 20):
                    data[rng.randrange(size)] = rng.randrange(256)
                data = bytes(data)
                blob = synth_nds.blz_compress(data, raw_prefix=prefix)
                self.assertLess(len(blob), len(data))
                self.assertEqual(x.blz_decompress(blob), data, (size, prefix))

    def test_long_matches_and_far_distances(self):
        data = bytes(range(256)) * 20 + b"\0" * 300
        self.assertEqual(x.blz_decompress(synth_nds.blz_compress(data)), data)

    def test_stored_uncompressed_when_extra_is_zero(self):
        data = b"abcdefgh" + struct.pack("<II", 0, 0)
        self.assertEqual(x.blz_decompress(data), data)

    def test_corrupt_footer_raises(self):
        blob = bytearray(synth_nds.blz_compress(b"\0" * 400))
        blob[-5] = 2  # footer length < 8
        with self.assertRaises(x.ExtractError):
            x.blz_decompress(bytes(blob))
        with self.assertRaises(x.ExtractError):
            x.blz_decompress(b"\0\0\0")

    def test_corrupt_stream_raises(self):
        blob = bytearray(synth_nds.blz_compress(bytes(range(64)) * 8))
        # Claim far more output than the stream produces.
        struct.pack_into("<I", blob, len(blob) - 4, 100000)
        with self.assertRaises(x.ExtractError):
            x.blz_decompress(bytes(blob))


class ExtractTests(unittest.TestCase):
    def check_rom(self, compress_arm9):
        rom, facts = synth_nds.build(compress_arm9=compress_arm9)
        (files, man, warnings), err = quiet(x.extract, rom, True)
        self.assertIn("UNSUPPORTED", err)
        self.assertFalse(man["rom"]["supported"])
        self.assertIn("UNSUPPORTED/SYNTHETIC", man["rom"]["note"])
        self.assertEqual(warnings, [])

        a9 = man["arm9"]
        self.assertEqual(files["arm9.bin"], facts["arm9"])
        self.assertEqual(a9["compressed"], compress_arm9)
        self.assertEqual(a9["ram_address"], "0x02000000")
        self.assertEqual(a9["entry"], "0x02000000")
        self.assertEqual(a9["static"]["size"], 0x800)
        self.assertEqual(a9["bss"], {"ram_address": "0x02000800", "size": 0x800})
        self.assertEqual(a9["module_params"]["found_by"], "footer")
        self.assertEqual(
            [(al["ram_address"], al["file_offset"], al["size"], al["bss_size"]) for al in a9["autoloads"]],
            [("0x01FF8000", 0x800, 0x20, 0), ("0x027E0000", 0x820, 0x10, 0x400)])

        self.assertEqual(len(man["overlays"]), 3)
        for ov, plain, (ov_id, ram, bss, comp) in zip(man["overlays"], facts["overlays"], facts["ov_meta"]):
            self.assertEqual(ov["id"], ov_id)
            self.assertEqual(files[ov["file"]], plain)
            self.assertEqual(ov["file"], f"overlay_{ov_id:04d}.bin")
            self.assertEqual(ov["compressed"], comp)
            self.assertEqual(ov["ram_address"], f"0x{ram:08X}")
            self.assertEqual(ov["ram_size"], len(plain))
            self.assertEqual(ov["bss_size"], bss)
            self.assertEqual(ov["bss_address"], f"0x{ram + len(plain):08X}")
            self.assertEqual(ov["static_init"], {"start": f"0x{ram + 0x18:08X}", "end": f"0x{ram + 0x1C:08X}"})
        # Overlays 0 and 1 share a RAM range: the reason for Ghidra overlay blocks.
        self.assertEqual(man["overlays"][0]["ram_address"], man["overlays"][1]["ram_address"])

    def test_plain_arm9(self):
        self.check_rom(False)

    def test_compressed_arm9(self):
        self.check_rom(True)

    def test_unknown_rom_refused(self):
        rom, _ = synth_nds.build()
        with self.assertRaises(x.ExtractError) as cm:
            x.extract(rom, allow_unknown=False)
        self.assertIn("not a supported dump", str(cm.exception))
        self.assertIn(x.sha1(rom), str(cm.exception))

    def test_supported_rom_accepted(self):
        rom, _ = synth_nds.build()
        saved = dict(x.SUPPORTED_DUMPS)
        try:
            x.SUPPORTED_DUMPS[x.sha1(rom)] = ("SyntheticForTest", len(rom))
            _, man, _ = x.extract(rom)
            self.assertTrue(man["rom"]["supported"])
            self.assertEqual(man["rom"]["id"], "SyntheticForTest")
            # Same hash, wrong size: refused.
            x.SUPPORTED_DUMPS[x.sha1(rom)] = ("SyntheticForTest", len(rom) + 1)
            with self.assertRaises(x.ExtractError):
                x.extract(rom)
        finally:
            x.SUPPORTED_DUMPS.clear()
            x.SUPPORTED_DUMPS.update(saved)

    def test_bad_overlay_file_id(self):
        rom, _ = synth_nds.build()
        rom = bytearray(rom)
        ov_table = struct.unpack_from("<I", rom, 0x50)[0]
        struct.pack_into("<I", rom, ov_table + 24, 999)
        with self.assertRaises(x.ExtractError) as cm:
            quiet(x.extract, bytes(rom), True)
        self.assertIn("outside the FAT", str(cm.exception))

    def test_truncated_rom(self):
        rom, _ = synth_nds.build()
        with self.assertRaises(x.ExtractError):
            quiet(x.extract, rom[:0x1100], True)

    def test_module_params_by_magic_search(self):
        rom, _ = synth_nds.build()
        rom = bytearray(rom)
        arm9_off, _, _, arm9_size = struct.unpack_from("<4I", rom, 0x20)
        rom[arm9_off + arm9_size:arm9_off + arm9_size + 12] = b"\0" * 12  # drop footer
        (_, man, _), _ = quiet(x.extract, bytes(rom), True)
        self.assertEqual(man["arm9"]["module_params"]["found_by"], "magic search")


class CliTests(unittest.TestCase):
    def test_cli_writes_outputs(self):
        rom, facts = synth_nds.build(compress_arm9=True)
        with tempfile.TemporaryDirectory() as tmp:
            rom_path = os.path.join(tmp, "synthetic.nds")
            with open(rom_path, "wb") as f:
                f.write(rom)
            out = os.path.join(tmp, "out")
            rc, _ = quiet(x.main, [rom_path, "-o", out])
            self.assertEqual(rc, 1)  # unknown ROM refused
            self.assertFalse(os.path.exists(out))
            rc, _ = quiet(x.main, [rom_path, "-o", out, "--allow-unknown"])
            self.assertEqual(rc, 0)
            names = sorted(os.listdir(out))
            self.assertEqual(names, ["arm9.bin", "manifest.json", "overlay_0000.bin",
                                     "overlay_0001.bin", "overlay_0002.bin"])
            with open(os.path.join(out, "manifest.json"), encoding="utf-8") as f:
                man = json.load(f)
            self.assertEqual(man["format"], "rescue-nds-code-manifest")
            self.assertEqual(man["rom"]["sha1"], x.sha1(rom))
            with open(os.path.join(out, "overlay_0001.bin"), "rb") as f:
                self.assertEqual(f.read(), facts["overlays"][1])

    def test_cli_refuses_tracked_dir_in_repo(self):
        rom, _ = synth_nds.build()
        with tempfile.TemporaryDirectory() as tmp:
            rom_path = os.path.join(tmp, "synthetic.nds")
            with open(rom_path, "wb") as f:
                f.write(rom)
            out = os.path.join(x.REPO_ROOT, "docs", "should-not-exist")
            rc, err = quiet(x.main, [rom_path, "-o", out, "--allow-unknown"])
            self.assertEqual(rc, 1)
            self.assertIn("not git-ignored", err)
            self.assertFalse(os.path.exists(out))


if __name__ == "__main__":
    unittest.main()
