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


class Arm7Tests(unittest.TestCase):
    """ARM7 binary (header 0x30..0x3C) with module params, autoloads and an ARM7 overlay,
    appended to the synthetic ROM (#349). Written from scratch, no retail bytes."""

    ARM7_RAM = 0x02380000
    WRAM = 0x037F8000
    OV7_RAM = 0x02390000

    def arm7_blob(self):
        """Static part 0x60 bytes (params at +0x40, hook word at +0x58), autoload data at
        +0x60 (0x40 bytes to WRAM, then 0x20 bytes to 0x027E0000), autoload list at +0x100."""
        r = self.ARM7_RAM
        b = bytearray(0x120)
        struct.pack_into("<2I", b, 0, 0xE3A00000, 0xE12FFF1E)  # mov r0,#0; bx lr
        struct.pack_into("<6I", b, 0x40, r + 0x100, r + 0x118, r + 0x60, r + 0x60, r + 0x80, 0)
        struct.pack_into("<I", b, 0x58, r + 0x40)
        b[0x60:0xA0] = b"W" * 0x40
        b[0xA0:0xC0] = b"D" * 0x20
        struct.pack_into("<6I", b, 0x100, self.WRAM, 0x40, 0x10, 0x027E0000, 0x20, 0)
        return bytes(b)

    def rom_with_arm7(self, blob, hook=True, ov7=None):
        rom, _ = synth_nds.build()
        rom = bytearray(rom)
        off = len(rom)
        rom += blob
        struct.pack_into("<4I", rom, 0x30, off, self.ARM7_RAM, self.ARM7_RAM, len(blob))
        struct.pack_into("<I", rom, 0x74, self.ARM7_RAM + 0x5C if hook else 0)
        if ov7 is not None:
            # One ARM7 overlay: a fourth FAT entry (the FAT has room before the next file).
            fat_off, fat_size = struct.unpack_from("<2I", rom, 0x48)
            fid = fat_size // 8
            data_off = len(rom)
            rom += ov7
            struct.pack_into("<2I", rom, fat_off + fat_size, data_off, data_off + len(ov7))
            struct.pack_into("<I", rom, 0x4C, fat_size + 8)
            table_off = len(rom)
            rom += struct.pack("<8I", 0, self.OV7_RAM, len(ov7), 0x10, 0, 0, fid, 0)
            struct.pack_into("<2I", rom, 0x58, table_off, 32)
        return bytes(rom)

    def test_arm7_laid_out_from_module_params(self):
        blob = self.arm7_blob()
        files, manifest, warnings = x.extract(self.rom_with_arm7(blob), allow_unknown=True)
        self.assertEqual(warnings, [])
        self.assertEqual(files["arm7.bin"], blob)
        a7 = manifest["arm7"]
        self.assertEqual((a7["ram_address"], a7["entry"], a7["sha1"]),
                         ("0x02380000", "0x02380000", x.sha1(blob)))
        self.assertEqual(a7["static"], {"ram_address": "0x02380000", "file_offset": 0, "size": 0x60})
        self.assertEqual(a7["bss"], {"ram_address": "0x02380060", "size": 0x20})
        self.assertEqual(a7["module_params"]["found_by"], "header 0x74")
        self.assertEqual(a7["module_params"]["arm7_offset"], 0x40)
        self.assertNotIn("sdk_version", a7["module_params"])
        al = [(a["ram_address"], a["file_offset"], a["size"], a["bss_address"], a["bss_size"])
              for a in a7["autoloads"]]
        self.assertEqual(al, [("0x037F8000", 0x60, 0x40, "0x037F8040", 0x10),
                              ("0x027E0000", 0xA0, 0x20, "0x027E0020", 0)])

    def test_arm7_without_params_is_all_static(self):
        blob = self.arm7_blob()
        files, manifest, warnings = x.extract(self.rom_with_arm7(blob, hook=False),
                                              allow_unknown=True)
        self.assertTrue(any("ARM7 module params not found" in w for w in warnings), warnings)
        a7 = manifest["arm7"]
        self.assertEqual(a7["static"]["size"], len(blob))
        self.assertEqual((a7["autoloads"], a7["bss"], a7["module_params"]), ([], None, None))

    def test_bad_arm7_autoload_list_is_not_trusted(self):
        blob = bytearray(self.arm7_blob())
        struct.pack_into("<I", blob, 0x44, self.ARM7_RAM + 0x1000)  # list end past the binary
        _, manifest, warnings = x.extract(self.rom_with_arm7(bytes(blob)), allow_unknown=True)
        self.assertIsNone(manifest["arm7"]["module_params"])
        self.assertTrue(warnings)

    def test_arm7_overlay_extracted(self):
        ov7 = bytes(range(64))
        files, manifest, warnings = x.extract(self.rom_with_arm7(self.arm7_blob(), ov7=ov7),
                                              allow_unknown=True)
        self.assertEqual(warnings, [])
        self.assertEqual(files["overlay7_0000.bin"], ov7)
        self.assertEqual(manifest["arm7_overlay_count"], 1)
        o = manifest["arm7_overlays"][0]
        self.assertEqual((o["file"], o["ram_address"], o["size"], o["bss_size"], o["file_id"]),
                         ("overlay7_0000.bin", "0x02390000", 64, 0x10, 3))
        self.assertEqual(len(manifest["overlays"]), 3)  # ARM9 overlays unchanged

    def test_arm9_unchanged_by_arm7(self):
        base_files, base_manifest, _ = x.extract(synth_nds.build()[0], allow_unknown=True)
        files, manifest, _ = x.extract(self.rom_with_arm7(self.arm7_blob(), ov7=b"\x05" * 32),
                                       allow_unknown=True)
        self.assertEqual(files["arm9.bin"], base_files["arm9.bin"])
        self.assertEqual(manifest["arm9"], base_manifest["arm9"])
        self.assertEqual(manifest["overlays"], base_manifest["overlays"])

    def test_no_arm7_means_no_file(self):
        files, manifest, _ = x.extract(synth_nds.build()[0], allow_unknown=True)
        self.assertNotIn("arm7.bin", files)
        self.assertIsNone(manifest["arm7"])
        self.assertEqual(manifest["arm7_overlays"], [])

    def test_truncated_arm7_refused(self):
        rom = self.rom_with_arm7(self.arm7_blob())
        with self.assertRaises(x.ExtractError):
            x.extract(rom[:-16], allow_unknown=True)


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
