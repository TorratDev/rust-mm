# Supported dumps

The tools accept only the dumps listed here. A dump is identified by the SHA-1 of the whole file: two files with the same SHA-1 are byte-for-byte identical, so every parser can rely on the data being where it expects. Any other file (another region or revision, a trimmed or patched ROM, a bad dump) is refused with an error that states its SHA-1 and points to this page. See [`legal.md`](legal.md): you supply a dump of a cartridge you own; the project never distributes one.

| ID (`rescue_nds::rom::RomId`) | Game | Platform | Region / revision | Size (bytes) | SHA-1 |
|-------------------------------|------|----------|-------------------|--------------|-------|
| `BlueRescueTeamEur` | Blue Rescue Team | NDS | EUR | 33,554,432 | `6f01887356268de39085df01490347bbcc3aa330` |

The list in code is `SUPPORTED_DUMPS` in [`crates/nds/src/rom.rs`](../crates/nds/src/rom.rs). Keep the two in sync. This is the reference dump used in Phase 0 ([`verification.md`](verification.md#dumps-used)).

Only whole-ROM hashes belong here. Hashes, sizes or other data of extracted files go in [`file-survey.md`](file-survey.md) under the rules in [`legal.md`](legal.md).

## Pointing the tools at your dump

1. Set the `RESCUE_ROM` environment variable to the path of your dump, or
2. create `rescue.toml` (git-ignored) with:

   ```toml
   rom_path = "roms/my-dump.nds"   # relative paths are relative to this file
   ```

   Use single quotes for Windows paths with backslashes: `rom_path = 'C:\roms\my-dump.nds'`.

`RESCUE_ROM` takes precedence over the config file. Apps read `rescue.toml` from the current directory; the test suite reads it from the workspace root. Running ROM-dependent tests is described in [`rom-tests.md`](rom-tests.md).

## Checking a dump by hand

```sh
sha1sum my-dump.nds        # Linux
certutil -hashfile my-dump.nds SHA1   # Windows
```
