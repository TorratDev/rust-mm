# Ghidra setup: ARM9 and overlays at their real addresses

How to build, on your own machine and from your own dump, a Ghidra project with Blue's ARM9 binary and every ARM9 overlay mapped at its real RAM address. This is the setup for Phase 0 check V8 ([#18](https://github.com/TorratDev/rust-mm/issues/18)) and the shared project of [#42](https://github.com/TorratDev/rust-mm/issues/42). It works on Windows and Linux.

**Legal:** the extracted binaries and the Ghidra project contain game code. Keep both outside the repository, or in an ignored folder (`extracted/`, `*.gpr`, `*.rep/` are git-ignored). Never commit, upload or attach them. Only the scripts are committed. See [`legal.md`](legal.md).

## What you need

| Tool | Version | Notes |
|------|---------|-------|
| Your dump | SHA-1 `6f01887356268de39085df01490347bbcc3aa330`, 33,554,432 bytes | the only supported dump ([`supported-dumps.md`](supported-dumps.md)) |
| Python | 3.8 or later | standard library only |
| JDK | 21 (64-bit) | e.g. Eclipse Temurin 21, or `openjdk-21-jdk` on Debian/Ubuntu |
| Ghidra | 11.4.3 (tested) | [release page](https://github.com/NationalSecurityAgency/ghidra/releases), file `ghidra_11.4.3_PUBLIC_20251203.zip`. Ghidra 12.1.4 also works (retail run, see "Tested so far"). |

Unzip Ghidra anywhere and set `GHIDRA_INSTALL_DIR` to the folder that holds `ghidraRun` / `ghidraRun.bat`:

```sh
export GHIDRA_INSTALL_DIR=~/tools/ghidra_11.4.3_PUBLIC                 # Linux
```
```powershell
$env:GHIDRA_INSTALL_DIR = "C:\tools\ghidra_11.4.3_PUBLIC"              # Windows
```

## 1. Extract the code from your dump

```sh
python3 tools/nds_extract_code.py path/to/your-dump.nds -o ~/rescue-re/code
```

(`py tools\nds_extract_code.py ...` on Windows. The ROM argument can be left out when `RESCUE_ROM` is set. Without `-o`, the output goes to `extracted/code` in the repository, which is git-ignored.)

The tool checks the dump's size and SHA-1 first and refuses any other file. It writes:

| File | Contents |
|------|----------|
| `arm9.bin` | ARM9 binary. If the ROM stores it BLZ-compressed (module params "compressed static end" non-zero), it is decoded first. |
| `overlay_NNNN.bin` | one file per ARM9 overlay (`NNNN` = overlay id), BLZ-decoded when flag bit 24 is set |
| `manifest.json` | ROM SHA-1, entry point, ARM9 RAM address, static size, static BSS, autoload sections (ITCM/DTCM), module params, and for each overlay: id, file id, RAM address and size, BSS, static initialiser range, flags |

It also refuses to write into a folder of the repository that git does not ignore. Read any `warning:` lines it prints: for example, a secure-area warning means the first 2 KiB of the ARM9 may be encrypted in your dump.

Format notes and the BLZ algorithm are in the tool's docstring. The tool is tested on synthetic ROMs (`python3 -m unittest discover -s tools/tests -v`), and its BLZ decoder was cross-checked against `ndspy` in both directions.

## 2a. Build the project headless (recommended)

```sh
tools/ghidra/import.sh ~/rescue-re/code ~/rescue-re/ghidra --check ov_0001+0x72688
```
```powershell
.\tools\ghidra\import.ps1 -CodeDir C:\rescue-re\code -ProjectDir C:\rescue-re\ghidra -Check ov_0001+0x72688
```

This runs Ghidra's `analyzeHeadless`, which:

1. imports `arm9.bin` as a raw binary with language **`ARM:LE:32:v5t`** (ARM946E-S is ARMv5TE; this is Ghidra's closest language id), compiler spec `default`;
2. runs [`RescueNdsImport.java`](../tools/ghidra/RescueNdsImport.java) **before** analysis. It replaces the raw block with the real memory map (table below) and labels `_start`, `ModuleParams`, each `ov_NNNN_start`, each overlay's static initialiser table (`ov_NNNN_sinit`, whose targets become entry points), and the ARM9 I/O registers;
3. runs auto analysis (skip it with `--no-analysis` / `-NoAnalysis` for a quick mapping check);
4. runs [`RescueNdsReport.java`](../tools/ghidra/RescueNdsReport.java), which prints every block with its function count, plus the references to each `--check` address.

The project is `rescue-arm9.gpr` in the project folder, and the full log is `rescue-arm9-import.log` next to it. Full analysis of the real ARM9 and all overlays can take a long time (tens of minutes). `analyzeHeadless` exits 0 even when a script fails, so the wrappers also check the log and exit 1 on a script error.

Memory map created by the script:

| Block | Space | Contents |
|-------|-------|----------|
| `arm9` | RAM | ARM9 static code and data, from the RAM address up to the autoload data |
| `arm9_bss` | RAM | ARM9 static BSS (uninitialised) |
| `autoload_N_XXXXXXXX` (+ `_bss`) | RAM | each autoload section at its destination (ITCM `0x01FF8000`, DTCM, ...). The crt0 copies these at boot, so they are not mapped where they sit in `arm9.bin`. |
| `ov_NNNN` | overlay space `ov_NNNN` | overlay `NNNN`, decoded, at its RAM address |
| `ov_NNNN_bss` | same overlay space | the overlay's BSS |
| `io`, `io_ipc` | RAM | ARM9 I/O registers `0x04000000`–`0x04001FFF` and `0x04100000`, volatile, labelled `REG_*` |

Each overlay gets its own overlay address space, because several overlays share the same RAM range. Inside an overlay, addresses within the overlay resolve to that overlay, and other addresses resolve to the base RAM space (the ARM9). Calls from ARM9 code into an overlay range do **not** resolve, because Ghidra cannot know which overlay is loaded. That is expected, not a mapping error.

## 2b. Or build it in the Ghidra GUI

1. `ghidraRun` (Linux) or `ghidraRun.bat` (Windows); create a project **outside the repository**.
2. *File → Import File* → `arm9.bin`. Format **Raw Binary**, Language **ARM v5t 32 little `ARM:LE:32:v5t`**, compiler `default`. Under *Options*, set the base address to the ARM9 RAM address in `manifest.json` (`0x02000000` normally; the script rebuilds the map anyway).
3. Open the program and answer **No** when asked to analyse.
4. *Window → Script Manager* → *Manage Script Directories* → add `tools/ghidra` from this repository. Run **RescueNdsImport.java** (category *NDS*) and choose `manifest.json`.
5. *Analysis → Auto Analyze*. Then run **RescueNdsReport.java** and enter `ov_0001+0x72688`.

The GUI path uses the same script, but it has not been run in this repository's tests (only the headless path has, see below).

## 3. What success looks like

The headless wrapper prints lines like these (from the synthetic test ROM; your dump has many more blocks):

```
RESCUE: block arm9                   02000000 - 020007ff  size 0x800
RESCUE: block autoload_0_01FF8000    01ff8000 - 01ff801f  size 0x20
RESCUE: block ov_0000                ov_0000::02100000 - ov_0000::021001ff  size 0x200  overlay
RESCUE: block ov_0001                ov_0001::02100000 - ov_0001::021003ff  size 0x400  overlay
RESCUE: mapped arm9 + 3 overlays, 12 blocks; ROM SHA-1 ...
RESCUE-REPORT: references to 02000020 (arm9+0x20): 4
RESCUE-REPORT:   from ov_0000::02100004 UNCONDITIONAL_CALL in ov_0000_start @ ov_0000::02100000
...
OK: project /home/you/rescue-re/ghidra/rescue-arm9.gpr
```

Check on your dump:

- `RESCUE: mapped arm9 + N overlays` where N is the number of ARM9 overlays in the manifest. The survey counted 145 overlays across the ARM9 and ARM7 tables ([`file-survey.md`](file-survey.md)); the manifest's `overlays` and `arm7_overlay_count` give the split.
- No `RESCUE: WARNING` about overlapping blocks, and the ROM SHA-1 is the supported one (no `UNSUPPORTED/SYNTHETIC` line).
- Functions are found in `arm9` and in the `ov_NNNN` blocks, and calls from overlays into the ARM9 resolve (`from ov_...` lines).
- **V8 spot check:** the type chart `gTypeEffectivenessChart` is in overlay 1 at decoded offset `0x72688` (V6). `--check ov_0001+0x72688` should list at least one reference, ideally from code or from a literal pool used by a function in `ov_0001`. If it lists none, look at the address in the GUI. The table may be reached through a pointer computed at run time, or analysis may have missed the function that uses it.

## 4. Record the V8 result

Fill the V8 entry of [`verification.md`](verification.md#v8--ghidra-loads-arm9-and-overlays-at-real-addresses) using its template. Record:

- **Environment:** OS, Ghidra version, JDK version (`java -version`), commit of this repository.
- **Evidence:** the command you ran; the `RESCUE: mapped ...` line; the block count; the number of BLZ-decoded overlays (`nds_extract_code.py` prints it); the function count from `RESCUE-REPORT: functions total`; and the `references to ov_0001::...` lines for the type-chart spot check. Addresses, counts and names are fine to commit. Code bytes, hex dumps and decompiler output are not ([`legal.md`](legal.md)).
- **Outcome:** *confirmed* only if all ARM9 overlays are mapped and the type-chart reference resolves; otherwise *partial*, with what is missing.

Also note whether you tried an NDS loader extension (e.g. NTRGhidra), as #18 asks. The script route does not need one.

## 5. REA (V9) on top of this project

V9 ([#19](https://github.com/TorratDev/rust-mm/issues/19)) checks [REA](https://github.com/morluto/rea) against this project. REA is a Node.js MCP server that drives an existing Ghidra install; per its README, setup is `npx rea-agents setup` (needs Node.js and npm). Point it at the Ghidra install above and at the project built in step 2, then test the four capabilities listed in V9 (function search, xrefs into and between overlays, decompiled summaries on ARM and Thumb code, evidence notes) on the code around `ov_0001+0x72688`. Record the results in the V9 entry of [`verification.md`](verification.md#v9--rea-workflow-on-arm-ds-binaries). These REA steps come from its README and have not been run here.

## 6. ghidra-cli

[ghidra-cli](https://github.com/akiselev/ghidra-cli) (GPL-3.0, tested with v0.2.2) runs Ghidra headless as a resident "bridge" and answers queries from the command line in well under a second: decompile, cross-references, renames, signatures, scripts. It is how the RNG work in [`re/rng.md`](re/rng.md) was done.

Setup (Windows example; the release zip holds one `ghidra.exe`):

```powershell
ghidra config set ghidra_install_dir C:\tools\ghidra_12.1.4_PUBLIC
ghidra config set ghidra_project_dir C:\rescue-re\ghidra
ghidra doctor
```

Use:

```powershell
ghidra --project rescue-arm9 --program arm9.bin decompile ov_0001::0219d058
ghidra --project rescue-arm9 --program arm9.bin x-ref to 0x020df704
ghidra --project rescue-arm9 --program arm9.bin script run tools\ghidra\RescueRngLabels.java
ghidra --project rescue-arm9 --program arm9.bin script run tools\ghidra\RescueNdsReport.java -- ov_0001+0x72688
```

Things to know:

- **One opener at a time.** The bridge opens the project like the GUI does. Save and close the GUI before you start the bridge, and stop the bridge (`ghidra --project rescue-arm9 stop`) before you reopen the GUI. Changes made through the bridge are on disk after it stops.
- **Stop it when you are done.** The bridge listens on `127.0.0.1` without authentication and can run scripts, so any local program could drive it while it runs.
- **Starting from a script:** the first command starts the bridge as a child process, which keeps the caller's output pipe open. When you capture output (for example `$x = ghidra ...` in PowerShell), start it first with `Start-Process ghidra -ArgumentList '--project','rescue-arm9','--program','arm9.bin','start'` and wait for `ghidra --project rescue-arm9 status` to show `State: running`.
- **Script arguments** go after `--`. `script run` does not show compile errors; compile the script with `javac` against Ghidra's jars to see them. A script method must not reuse a `FlatProgramAPI` name (`find`, `setPlateComment`, ...).
- Overlay addresses are written `ov_NNNN::ADDRESS`. Calls from the ARM9 into an overlay land on the `ram` address, not the overlay's.

## Tested so far

- **2026-10-14, Linux, sandbox, no retail dump.** Ghidra 11.4.3 (`ghidra_11.4.3_PUBLIC_20251203.zip`), OpenJDK 21.0.12. Synthetic ROM from [`tools/tests/synth_nds.py`](../tools/tests/synth_nds.py) (`--compress-arm9`: BLZ-compressed ARM9, two autoloads, three overlays, two of them BLZ-compressed, two sharing `0x02100000`). `import.sh` mapped 12 blocks. Analysis found the overlay functions from their static-initialiser entries. Calls from all three overlays to `0x02000020` resolved to the ARM9, and each overlay's literal-pool pointer resolved inside its own overlay space. `import.ps1` was not run (the test session could not run PowerShell).
- **2026-10-14, Windows 11, supported dump.** Ghidra 12.1.4, JDK 25.0.4, headless import wrapper. All 145 overlays mapped (154 blocks); auto-analysis found functions in `arm9` and overlays 0, 1, 2 and 7 only. Details in V8 of [`verification.md`](verification.md#v8--ghidra-loads-arm9-and-overlays-at-real-addresses). ghidra-cli v0.2.2 used against the same project (section 6).
