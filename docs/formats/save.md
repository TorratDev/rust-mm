# Save format

Blue's main save: the backup media, how the data is laid out on it, how it is checked, and what is still open. Issue: [#53](https://github.com/TorratDev/rust-mm/issues/53). V4 in [`verification.md`](../verification.md#v4--non-audio-formats-and-save-layout). Function names and addresses are in [`symbols.md`](../symbols.md).

These are research notes. Nothing here has been checked against a real save file; real saves are user data and are never committed ([`legal.md`](../legal.md)).

## Tags

- **[blue-verified]**: a fact tied to an address in Blue's ARM9. Only facts with an address get this tag.
- **[lead]**: a name, layout or reading taken from `pmd-red` (`src/save.c`, `src/data_serializer.c`) or from an investigator's notes that is not yet confirmed in Blue code (it may cite an address). Not a fact.
- **[open]**: not known. Needs a Blue address or a runtime check.

`pmd-red` is cited by function name only; no code is copied ([`legal.md`](../legal.md)).

## 1. Media, size, slots, checksum

**Media and sector size**

- **[blue-verified]** `InitFlash` (`020190CC`) identifies the backup chip by calling `CARD_IdentifyBackup(0x1202)` (`0207E598`). The literal `0x00001202` is read from `0x02019124`.
- **[blue-verified]** `0x1202` decodes as a 2 Mbit (256 KiB) flash chip. The low byte `0x02` selects the flash family (`FUN_0207E3A8`, `0x0207E3A8`, branch `param_1 == 2`). The size is `1 << ((type >> 8) & 0xFF)` = `0x40000` (same function, `0x40000` branch). The SDK size is stored at `+0x18` of the backup struct and read by `FUN_0207E6E0` (`0x0207E6E0`).
- **[blue-verified]** `InitFlash` stores the SDK size in the global `0x020F38B4` (from `FUN_0207E6E0`). Runtime value at the title screen: `0x40000` (#197 P7.1, `save_load_probe.py` run with no save).
- **[blue-verified]** `InitFlash` stores the sector unit `0x1000` in the global `0x020F38B8` (`*DAT_0201912C = 0x1000`). Runtime value `0x1000` (same run).
- **[blue-verified]** Reads and writes go through `ReadFlashData` (`02018F0C`) and `WriteFlashData` (`02018DC4`) in 4 KiB sectors, up to 4 attempts each (source: `docs/symbols.md`, lines 184–185).

**Main save block and slots**

- **[blue-verified]** `WriteSavetoPak` (`0202A53C`) builds one block of `0x57D8` bytes and writes it twice in a row with `WriteSaveSector`, which advances the sector after each copy. Each copy takes 6 sectors (`0x6000` bytes).
- **[blue-verified]** `ReadSaveFromPak` (`0202A1AC`) reads copy A first. Copy A status 0: copy B is skipped and the sector counter advances by 6. Status 1 (read failed) or 2 (checksum mismatch): copy B is read. If the status is still 0 and the magic at `+0x414` is not `0x06061320`, the result is 4. A copy A with a bad magic does **not** fall back to copy B (#197 P1).
- **[blue-verified]** Sector map: copy A is sectors 0–5 (offset `0x0000`), copy B is sectors 6–11 (offset `0x6000`). Sector counts come from `(size + 0xFFF) / 0x1000` (`0x02029F6C`), with two `WriteSaveSector` calls in `WriteSavetoPak` (`0x0202A53C`).
- **[blue-verified]** The main block is `0x57D8` bytes (`0x0202A410`, `0x0202A73C`). All five `WriteSavetoPak` call sites start at sector 0, so there is no save slot index; the layout is fixed.
- **[blue-verified]** Sectors 12–15 and 21–30 are not used by any decompiled save routine, and sectors 32–63 are not referenced either. This is from the decompiles; Ghidra's x-ref list omitted `ReadQuickSave` and `FUN_0202B140` as callers, so it is not a full-program proof (#197 P7.6).
- **[blue-verified]** Quicksave (suspended dungeon): sectors 16–20 (`0x10` to `0x14`), 0x4800 bytes starting at sector `0x10` (`local_10 = 0x10`, 5 sectors) (`ReadQuickSave` `0x0202AFD4`, write state case 1 `0x0202B140`; size from `FUN_0206DB78`, `0x0206DB78`). Validity: `IsQuickSaveValid` (`0x0202AFC0`) reads byte `+0x0C` of the struct at `*DAT_0202AFD0`. Read state: `PrepareQuickSaveRead` (`0x0202B0B8`) builds a 16-byte struct from `MemoryAlloc(0x10, 5)`: buffer at `+4`, size at `+8`, `+0x0C` = 0, `+0` = 1.
- **[blue-verified]** Status marker at sector `0x1F`, 0x800 bytes (writer `0x0202A4A0`; readers `0x0202A0A4`, `0x0202A11C`).
- **[blue-verified]** The marker erase `FUN_0202A444` writes sector `0x1F` from an 0x800 buffer in which only the first 4 bytes are set to `0xFF` (`thunk_FUN_02021F8C(src, 0xFF, 4)`). The rest is whatever `MemoryAlloc` returns. `MemoryAlloc` does not zero memory **[blue-verified]**: the allocator `020221A8` has no fill call on its path (#239 X.1). In the boot-to-dungeon probe the erase runs at frames 2460 and 2563, and the 0x800-byte buffer holds 4 bytes of `0xFF` and 0x7FC bytes of leftover heap, different at the two calls. So the sector written at `0x1F` carries that heap data after its marker. Whether any reader uses bytes past the marker is **[open]** (#197 X.1).
- **[lead]** Marker values: `0xF1207` success, `0xF1208` invalid, `0xF1209` none (`0x0201E3C0`, `0x0202B310`, `0x0202B324`). Semantics not verified.
- **[lead]** Sector `0x10` is not erased on load; the load path (`0x0201E204`) writes only the marker.
- **[blue-verified]** The alternate branch is gated by the word at `0x020E0490`, the **session struct pointer** (not a debug flag). The literal at `0x02029EC8` (in `ReadSaveSector`) and the one at `0x02029F68` (in `WriteSaveSector`) both point at that cell. `FUN_020660E4` allocates the session struct and calls `FUN_0202A778`, which stores the pointer. `FUN_02065F88` clears it. In the boot-to-dungeon run (new game, 24,960 frames) neither fired and the pointer stayed 0. `FUN_02065CFC` is reached only through `FUN_02065E8C` and `FUN_02065EEC`, which the mode-0xB family of the game loop (`FUN_020628BC`) calls. So the pointer is set only there, and only when a device is found (**[blue-verified]** structure; which player-facing scene that is was not read, #239 P7.3). When the pointer is set, `FUN_0201D638` checks the device, and with no device `ReadSaveSector` returns status 1. Otherwise the read goes through `FUN_0201D730` and `FUN_02074780` to the bus window at `0x0A000000`, with bank switching for 0x20000-byte chips. The device is a GBA cartridge in Slot-2, identified by its header cache (maker `01`, game code `B24P`). The window is that cartridge's backup flash (**[blue-verified]** constants and window; "Red cartridge" is **[hypothesis]**, #239 P7.5).
- **Red cross-check** (`pret/pmd-red` `src/save.c`): marker sector `0x1F` and default `0xF1209` match. Red's quicksave sector was not checked.
- No local real save was available for a check. Runtime values from the #197 probes (an empty card and injected cases) are in sections 3 and 4.

**Quick-save write and buffer (#197 P3; static decompile, no runtime run)**

- **[blue-verified]** `LoadAndRunQuickSaveDungeon_Async` (`0x0201E204`) allocates the quick-save buffer as `setup+0x58`, with size `0x4800` from `FUN_0206DB78` (`0x0206DB78`). A second buffer at `setup+0x5C` has the size of the literal at `0x0206DB8C` (`0x0001AC5C`).
- **[blue-verified]** At dungeon exit `RunDungeon_Async` (`0x0206DC20`) passes the buffer to the overlay-1 serializer `ov_0001::0219B81C`, with size `0x4800`. On a fresh start (`setup+6 == 0`) byte 0 is set to 0 first. On resume (`setup+6 != 0`), `ov_0001::0219A268` restores it before the dungeon starts.
- **[blue-verified]** The stream starts with a 4-byte zero word, then seven sections written by `0219B7E0`, `0219AB38`, `0219AA14`, `0219A9C0`, `0219B700`, `0219B5F8` and `0219B47C`, then the 8-byte end tag at `PTR_DAT_ov_0001__021AE3B0`, written by `0219BC34` (an 8-byte write through `0219BCA0`); `0219BC44`, called last, is empty. `0219B47C` also calls the per-monster writer `0219ADFC` (at `0219B5B0` and `0219B5DC`, #171 P2b). The restore discards the 4 bytes and calls the matching readers. Sizes, measured in one state (a forced call at the first turn of floor 1, #239 P3.7): 12,075 bytes before the monster section (the 4-byte zero word, the six sections and the header of `0219B47C`), 5,280 bytes for the monster section (20 slots of 264 bytes), and the 8-byte end tag, 17,363 in total.
- **[blue-verified]** The write starts only in `LoadAndRunQuickSaveDungeon_Async` (call sites `0x0201E348`, `0x0201E35C`), and only when `setup+0x60` is `3` or `-2`. `FUN_0202B32C` (`0x0202B32C`) allocates a 0x14-byte state (`MemoryAlloc(0x14, 5)`) with the buffer at `+4`, the size at `+8`, and a flag at `+0xC` that is set when `setup+0x60 == -2`.
- **[blue-verified]** State machine `FUN_0202B140` (`0x0202B140`). State 1 calls `FUN_0202B5DC` (stops BGM and the icon) and `WriteSaveSector(sector 0x10, buf, 0x4800)`, stores the result at `+0x10`, and returns 1, so the caller frees both buffers. State 2 calls `WriteSavetoPak` only when `+0x10 == 0`, then shows a message. State 5 returns 3 if the result is nonzero, else 2.
- **[blue-verified] Correction:** the `+0xC` flag does **not** decide whether the main save is written. It selects only the message text.
- **[blue-verified] Risk:** the byte writer `ov_0001::0219BCA0` and the byte reader `ov_0001::0219BC48` increment the count but never compare it with the end pointer set by `0219BCEC` and `0219BC88`. A payload over `0x4800` bytes would write past the buffer. In the one measured state the payload is 17,363 bytes, 1,069 under 0x4800 (#239 P3.7). The monster section is a fixed 20 × 264 bytes, so only the 12,075-byte prefix could vary with game state. Its six writers were not decompiled for variable-length loops, so whether the payload fits in every state is **[open]** (#197 P3.7).
- **[open]** The meaning of the setup values `1`, `2` and `4`, and of dungeon `+3` and `+6`.

**Quick-save and the global RNG state** (investigator C on [#171](https://github.com/TorratDev/rust-mm/issues/171), P2; static Ghidra only, no runtime run)

- **[lead]** Answer: a quick-save resume does not set the global LCG `sPRNGState` (`0x020DF704`). The quick-save data does not carry it, so after a resume the global LCG keeps whatever value the session had before the load. High confidence for the code path; the runtime effect is not run.
- **[blue-verified]** Writers of `0x020DF704` are `SetRNGSeed` (`0x020118F8`, `0x02011904`), `Rand16Bit` (`0x020119BC`, `0x020119C4`) and `SeedRng` (`0x02011A0C`). In the save code, `SetRNGSeed` is called only from `ReadSaveFromPak` (`0x0202A250`), the main-save reader.
- **[blue-verified]** `LoadAndRunQuickSaveDungeon_Async` (`0x0201E204`, branch `setup+6 != 0`) calls `PrepareQuickSaveRead`, a loop of `ReadQuickSave` with `MainLoops_RunFrameActions`, `IsQuickSaveValid`, `FinishQuickSaveRead`, `FUN_0202A4A0` and `LoadAndRunDungeon_Async`. None of these calls `SetRNGSeed`, `ReadSaveFromPak` or a `Rand*` function, and none writes `0x020DF704` directly.
- **[blue-verified]** `FUN_0200D4DC` (`0x0200D4DC`) is the one place the branch touches the global LCG. It runs only when `setup+0x60 == -2`. It calls `Rand32Bit()` (two LCG steps, `0x0201198C`), seeds the Mersenne Twister, and reads `unk418` through `sub_8011C34` (`0x0202A060`). See the `unk418` accessors above.
- **[blue-verified]** `setup+0x60` is set to `-2` in `RunDungeon_Async` (`0x0206DC20`), on the branch where dungeon `+6 != 0` and `0x0219C868` returns nonzero. It is set to `-1` when dungeon `+6 == 0` (#197 P3.4). So the `FUN_0200D4DC` draw happens on that resume branch.
- **[blue-verified]** The quick-save write (`FUN_0202B140`, state 1, `0x0202B1xx`) calls `FUN_0202B5DC` and `WriteSaveSector` only. It does not call `GetRNGState`. `WriteSavetoPak` (state 2) is the only caller of `GetRNGState` on this path, so the global state is saved only in the main save's `+0x420`.
- **[blue-verified]** `FUN_0202B57C`, `FUN_0202B5DC`, `FUN_0202EDAC`, `FUN_0202F280` and `FUN_0206DB78` contain no RNG accesses.
- **[lead]** Per-frame draws keep moving the global LCG during the read loop. `MainLoops_RunFrameActions` runs each frame, and the frame callback `FUN_02018ACC` (see #175 P3 and #44) draws from it. So the global state changes during the read even though the read does not set it.
- **[blue-verified]** The dungeon LCG `gDungeonRngState` (`ov_0001::021B3F58`) is not in the quick-save (high for the code path). Its only accesses are `DungeonRand16Bit` and `InitDungeonRNG`, and no serializer section copies it. The quick-save stores the dungeon run preseed `gDungeon+0x66C` and the floor seed `gDungeon+0x670` as raw bytes in the block `gDungeon+0x63C` to `0x68C` (writer `ov_0001::0219B7E0`, reader `ov_0001::0219A22C`).
- **[blue-verified]** The resume condition in `RunDungeon_Async` (`0206DC20`) is `setup+6 != 0`. The preseed `+0x66C` is set only when `setup+6 == 0`, and so are the floor seed `+0x670`, `+0x658 = 10` and `InitDungeonRNG` (`0219D0B0`). `setup+6` is never reassigned in the function, so on a resume `SetDungeonRNGPreseed23Bit` (`0219D124`), `GenerateDungeonRNGSeed` (`0219D0C4`) and `InitDungeonRNG` do not run on any iteration. The dungeon LCG is not reseeded on resume. This confirms the preseed is drawn once per run, and extends the rule to the floor seed and `InitDungeonRNG`.
- **[open]** The runtime consequence: whether the first dungeon draw after a resume uses the stale state. Overlays 2 to 144 were not scanned for calls to these three routines. V7 traces must capture the state at resume.

**Checksum**

- **[blue-verified]** The header's first 4 bytes (word 0) hold a checksum: the sum of the other 32-bit words in the block. `CalculateChecksum` (`02029FD4`) writes it; `ValidateChecksum` (`02029F98`) checks it.
- **[blue-verified]** `ValidateChecksum` (`0x02029F98`) returns true when word 0 differs from the sum (`return *in != sum`). `docs/symbols.md` says the same, so the return sense is settled; status 2 is a mismatch (#197 P1).
- **[blue-verified]** Word 0 is the 32-bit sum of words 1..N−1, with no complement (`CalculateChecksum`, `0x02029FD4`). A checksum mismatch makes `ReadSaveSector` (`0x02029E2C`) return status 2 (`ValidateChecksum`, `0x02029F98`).

## 2. Serialized layout

**Header** (layout from `pmd-red`; the Status column says which offsets are tied to Blue addresses)

| Offset | Size | Field | Status | Notes |
|--------|------|-------|--------|-------|
| `0x000` | 4 | checksum | [blue-verified] | Word 0; see section 1 (`02029FD4`, `02029F98`) |
| `0x004` | `0x400` | global script variables | [blue-verified] | `SaveGlobalScriptVars` `0201F884` / `RestoreGlobalScriptVars` `0201F81C`; raw copy in Red |
| `0x404` | `0x10` | game internal name | [blue-verified] | `GetGameInternalName` `0202A000`, copied by `WriteSavetoPak` (`0202A53C`) |
| `0x414` | 4 | magic | [lead] | Blue `0x06061320` (Red `0x05071412`); mismatch fails the load |
| `0x418` | 4 | `unk418` | [blue-verified] | Getter `sub_8011C34` `0202A060` (called by `WriteSavetoPak`); setter `sub_8011C40` `0202A050` (called by `ReadSaveFromPak`). Global `0x02132D98`. Struct `+0x48`. Reset to `-1`. Save/session identity nonce ([hypothesis], medium; see the accessors below) |
| `0x41C` | 4 | `unk41C` | [blue-verified] | Setter `sub_8011C28` `0202A070` (called by `ReadSaveFromPak`). Global `0x020E048C`. Struct `+0x4C`. Reset to `0`. Save kind: 0 none (reset), 1 save, 2 quick save ([blue-verified] uses; the name is red-hypothesis) |
| `0x420` | 4 | global RNG state | [lead] | `GetRNGState` / `SetRNGSeed`; see [`re/rng.md`](../re/rng.md). Struct `+0x50` |
| `0x424` | 4 | language | [blue-verified] | Written by `WriteSavetoPak` (`0202A53C`). Reader side `SetLanguage` `0202A010` is tentative. `ReadSaveFromPak` (`0202A240`) sets `-1` when `unk41C` is 0. Struct `+0x54` |
| `0x428`–`0x448` | 4 each | section results | [lead] | One value per section; compared on load |
| `0x44C` | | section data | [lead] | Starts here |

**`unk418` and `unk41C` accessors** (investigator C on [#171](https://github.com/TorratDev/rust-mm/issues/171); static Ghidra decompiles only, no runtime run)

- **[blue-verified]** `unk418` uses one global, `0x02132D98`. `sub_8011C40` (`0202A050`) writes it through `DAT_0202A05C`. Its only writer is `0202A054`. `sub_8011C34` (`0202A060`) returns it through `DAT_0202A06C`. Its only reader is `0202A064`.
- **[blue-verified]** `unk41C` has its own global, `0x020E048C`. `sub_8011C28` (`0202A070`) writes it through `DAT_0202A07C`. Its only writer is `0202A074`. `FUN_0202A080` reads it, at `0202A0A0`, and returns the session struct's `+0x4C` when the struct pointer is set.
- **[blue-verified]** Setter `sub_8011C40` has four call sites (#171 P1b): `FUN_0202A954` (`0202A968`, reset to -1), `ReadSaveFromPak` (`0202A248`, the saved word at `+0x418`), `FUN_ov_0000__02140860` (`ov_0000::021409D0`, a `Rand32Bit()` result when the value is -1) and `FUN_0206C6E4` case 0xF (`0206C85C`, random). Getter `sub_8011C34` (`0202A060`) has eight call sites: `WriteSavetoPak` (`0202A578`, stores it at `+0x418`), `FUN_0200D4DC` (`0200D558`, mail record `+0x24`), `FUN_02051AD4` (`02051AEC`, kind-4 record `+0x28`), `FUN_02065224` (`02065244`, validity check), `FUN_02066D14` (`02066E7C`, message 0x20 check), `FUN_02062FD0` (`02062FD8`, one message set when the value is -1), `FUN_ov_0007__021390E8` (`ov_0007::021391B0`, a team-id byte at `+0x33A`) and `FUN_ov_0000__02140860` (`ov_0000::021409BC`).
- **[hypothesis, medium]** Role of `unk418`: a per-save identity nonce, -1 until generated. Its data flow: it is copied into the mail record (`FUN_0200D4DC`) and the kind-4 record (`FUN_02051AD4`); the two validity checks accept a record only when its token equals the local one; it sets a team-id byte; and `FUN_02062FD0` tests it for -1. It is not a seed for the dungeon or global LCG in the code read. The label "save/session identity token" is a hypothesis.
- **[blue-verified]** Save kind writers (`sub_8011C28`, 14 call sites, #171 P1c). Value 0: the reset `FUN_0202A954` (`0202A960`). Value 1: `FUN_0204CD40`, `FUN_0204D7B8`, `FUN_0204DDFC`, `FUN_0204DF00`, `FUN_020508C8`, `FUN_02050900`, `FUN_02050BF8`, `FUN_02051AD4`, `FUN_02050758`, `FUN_ov_0000__0213A8AC` and `FUN_ov_0000__02140860` (two sites). Each sets 1 right before `FUN_0202AE50(0)`, the start of the save-write state machine. No caller passes 2 to `sub_8011C28`. Two callers pass the kind straight to `WriteSavetoPak`, without `sub_8011C28`: `FUN_0201ED4C` and `FUN_02069848` pass 1 (blue-verified, decompile), as the quick-save writer passes 2. The value 2 enters the save through the quick-save writer `FUN_0202B140` (case 2), which calls `WriteSavetoPak(&state, 2)`; that stores 2 at `+0x41C` when no session struct is active. `ReadSaveFromPak` (`0202A240`) restores the value saved in the file.
- **[blue-verified]** Save kind readers (`FUN_0202A080`, 9 call sites, #171 P1c): `FUN_0202AC00` (`0202AD08`) and `FUN_020665F4` (`02066640`) pass it to `WriteSavetoPak`. `RunGameMode_Async` (`0201E8CC`, `0201E944`, `0201E964`) resumes a suspended dungeon only when it is 2 and the marker check `FUN_0202A11C` passes; its other uses compare with `!= 2` or `== 1`. `FUN_020679B0` (`020679F4`) is true only when it is 2 and a marker matches `DAT_02067A18`. `FUN_0206796C` (`0206799C`) returns 1 when it is 2. Both also need `FUN_0200D690(1)` and `FUN_0200D690(7)` to be 0. `FUN_02055614` (`02055670`) clears a byte unless it is 2. `FUN_02067AE0` (`02067CF8`) branches only when it is 1.
- **[blue-verified]** Load: `ReadSaveFromPak` calls `sub_8011C28(+0x41C)` and `sub_8011C40(+0x418)`, then `SetRNGSeed(+0x420)`. If `+0x41C` is `0`, it sets language (`+0x424`) to `0xFFFFFFFF` (`-1`); if it is 1 or 2, it calls `SetLanguage(+0x424)`. So the saved language is trusted only when the save was written with a kind. Red has no language field here.
- **[blue-verified]** Save: `WriteSavetoPak` first sets `+0x41C` from its argument, `+0x418` from `sub_8011C34()`, and `+0x420` from `GetRNGState()`. It then overwrites `+0x41C`, `+0x418`, `+0x420` and `+0x424` from the game struct, so those three values are not the final saved values.
- **[blue-verified]** Reset: `FUN_0202A954` calls `sub_8011C28(0)` and `sub_8011C40(-1)`, so `unk41C` becomes `0` and `unk418` becomes `-1`.
- **[blue-verified]** `unk418` is written by `FUN_0206C6E4` case 0xF, as a random value (Rand32 times 13 struct words, then a Mersenne Twister draw, forced not to be `-1`). `FUN_0200D4DC` stores the value it reads at its struct `+0x24` (#197 P8).
- **[blue-verified]** `unk41C` is the save kind passed to `WriteSavetoPak`. The writers and readers are in the tables just above. `RunGameMode_Async` resumes a suspended dungeon only when the value is 2 and the marker check `FUN_0202A11C` matches `DAT_0201EB78`. Otherwise it takes the error path `FUN_0200E3FC` (#197 P8.2). The name "save kind" is red-hypothesis.
- **[blue-verified]** Both routines read the struct pointer at `0x020E0490`, which they dereference. Save fields map to struct offsets: `+0x418` ↔ `+0x48`, `+0x41C` ↔ `+0x4C`, `+0x420` ↔ `+0x50`, `+0x424` ↔ `+0x54`.
- **[blue-verified]** `0x020E0490` is the session struct pointer, not a debug save-state pointer. The `gUnknown_203B184` label in `symbols.md` is wrong for this cell. It is set by `FUN_020660E4` and cleared by `FUN_02065F88`, and it is 0 in the new-game boot to a dungeon (#197 P7.2, P7.3).
- **[red-hypothesis]** The name `unk41C` (save kind) comes from Red. `unk418` is named after its data flow (hypothesis; see its role above). Struct `+0x48` and `+0x4C` hold the session copies of the two fields.
- **[open]** The content of the session struct beyond `+0x48` to `+0x54` (only those and the 13-word copy in `FUN_0202A778` were seen). The return codes 0x18 and 0xB of the two validity checks (`FUN_02065224`, `FUN_02066D14`). Whether `ov_0000` case 3 discards its `Rand32Bit()` roll. Runtime use of `unk418` (the own-save check and the team id) is not checked. The x-ref lists cover only statically linked calls.

**Sections** (offsets from `0x44C`; order and sizes from the save/restore pairs in `symbols.md`, except where marked [lead])

| Offset | Size | Contents | Restore (read) / save (write), with addresses | Status |
|--------|------|----------|-----------------------------------------------|--------|
| `0x0000` | `0x4650` | recruited Pokémon | `RestoreRecruitedPokemon` `02003F2C` / `SaveRecruitedPokemon` `0200409C` | [blue-verified] pair; size is [lead] (not in `symbols.md`) |
| `0x4650` | `0x258` | `Poke2` records (150 × 4) | `RestorePoke2s` `020038AC` / `SavePoke2s` `02003AD0` | [blue-verified] |
| `0x48A8` | `0x1D8` | team inventory | `RestoreTeamInventory` `020068AC` / `SaveTeamInventory` `020069C8` | [blue-verified] |
| `0x4A80` | `0x10` | rescue team info | `ReadRescueTeamInfo` `020086CC` / `SaveRescueTeamInfo` `02008758` | [blue-verified] |
| `0x4A90` | `0x8` | friend areas | `ReadSavedFriendAreas` `02008CB0` / `SaveFriendAreas` `02008D30` | [blue-verified] |
| `0x4A98` | `0x100` | adventure data | `RestoreAdventureData` `02010EFC` / `SaveAdventureData` `02010F4C` | [blue-verified] |
| `0x4B98` | `0x594` | unnamed in Red | `sub_80954CC` `0200D2B8` / `sub_8095624` `0200D0CC` | [blue-verified] |
| `0x512C` | `0x221` | mail | `RestoreMailInfo` `0200DD5C` / `SaveMailInfo` `0200DB80` | [blue-verified] |

The sections end at `0x534D`, inside the `0x57D8` block. The restore function for each section must return the value stored in its header slot.

**Bit packing** (`pmd-red` `src/data_serializer.c`)

- **[lead]** Every section except the script variables is bit-packed. `WriteBits` copies the low `n` bits of a value, least significant bit first, filling each byte from bit 0. `ReadBits` is the reverse.
- **[lead]** A section's header result is the bit position (0–7) inside its last byte when the section ends. On load, the reader must end at the same position.
- **[lead]** Multi-byte values are written low byte first.

**Field widths** (an investigator compared each section's `WriteBits` widths in Blue's code against the `pmd-red` functions; the result is reported as matching, but the comparison is not final and no Blue address is attached to the list)

- **[lead]** `Poke2` records, friend areas, mail and rescue records, mail info: reported as the same widths in both.
- **[lead]** Script variables: a raw 0x400-byte copy in both. The copy is confirmed in Blue (see below); the validation target is open.
- **Recruited Pokémon, rescue team info, adventure data, team inventory:** confirmed from Blue's code. See the subsections below.

**Recruited Pokémon records** (`SaveRecruitedPokemon` `0200409C`, `RestoreRecruitedPokemon` `02003F2C`)

- **[blue-verified]** A record is 0x40 bytes in RAM; the save is a bit stream, not byte fields (stride `0x40` in the writer loop, `0200409C`).
- **[blue-verified]** Global pointer to the record array: `020DDF84`. Record count 413 (0x19D): `02004094`, `0200421C`. Team block offset `0x6890`: `02004098`, `02004220`.
- **[blue-verified]** Field widths in writer order (`WritePoke1Bits` `02003E1C`; helpers `0200617C`, `0200684C`, `020094D8`, `02009550`, `0200388C`). Decompiled per helper. Level helper calls `WriteBits` through a pointer at `0x02003898` (value `0x0200C0F4`):
  - `+3` (7 bits); `+6` (9); `+4` and `+5` (7 each, dungeon location); `+8` (7); `+9` (7); `+10` (10); `+12` (10); `+14` (8); `+15` (8); `+16` (8); `+17` (8); `+20` (24); `+24` (24); `+27` (4); `+28` (8) and `+29` (7), the held item; moves at `+30` (4 × 20 bits, stride 6); `+54` (80).
- **[lead]** Record is 323 bits. Arithmetic from the widths above (7+9+14+7+7+10+10+8+8+8+8+24+24+4+15+80+80).
- **[blue-verified]** The reader mirrors the writer for every field: `FUN_02003CD8`, called from `RestoreRecruitedPokemon` (`02003F2C`).
- **[blue-verified]** Read helpers (#197 P2). `FUN_02006140` reads the dungeon location as two 7-bit fields into record `+4` (x) and `+5` (y). `FUN_0200389C` is not a level reader: it is `ReadBits(…, 7)` through the pointer at `0x020038A8` (value `0x0200C050`), used for `unkC[0]` and `unkC[1]` at record `+8` and `+9`. The record's level is at `+3`, read directly by `FUN_02003CD8`. `FUN_020094A0` reads four 6-byte move records: `+0` 4 bits (`moveFlags`), `+2` 9 bits (move id), `+4` 7 bits (the ginseng value, not PP; see [`structs.md`](../re/structs.md#roster-move-layout-239-p2c)), the mirror of `WritePoke1MovesBits` (`0x020094D8`). Held item `ReadHeldItemBits` (`0x0200687C`) is decompiled but not re-read here.
- **[blue-verified]** Move byte `+1` (`moveFlags2`) is not written by the save, and the loader zeroes it (`FUN_02003CD8`). At a new game it is `0x02` in the first two moves of both in-use records (one state, #239 P2c), so a nonzero value is dropped at save time. Byte `+5` is not used in the roster format. Which code writes `+1 = 0x02`, and which code sets the ginseng after a dungeon, are **[open]** (#197 P2c, #239 P2c). Whether the largest move id fits in 9 bits is not checked either.
- **[open]** The meaning of the `unkC` fields. The names are hypotheses.
- **[open]** Meaning of the record fields (species, level, item, moves). Names are inferred only from offsets and widths.
- **[blue-verified]** Team block: 4 entries, each a 1-bit flag then `WritePoke1Bits`; 4 × 324 bits (`SaveRecruitedPokemon`, second loop).
- **[blue-verified] Flag asymmetry:** the writer stores `flags & 1`; the reader sets `3` or `0`. Writer loop at `+0x6890`; reader second loop. The round trip is exact only for values 0 and 3: a flags value 1 comes back as 3, and 2 as 0. Whether the game stores other values is **[open]** (#197 P6.3). The team block is the flags words of records 418–421 (`0x6890 = 418 × 0x40`).
- **[blue-verified]** Team-index list: six 16-bit entries, then one 16-bit leader index. The writer sets bit 2 on each listed record and byte `+2` = 1 on the leader (`SaveRecruitedPokemon` tail; `RestoreRecruitedPokemon` tail).
- **[blue-verified]** Valid bit: on load it is rebuilt from byte `+3` being nonzero (`FUN_02003CD8`, first lines). The writer (`0x0200409C`, line 36) zeroes byte `+3` in the live record for invalid roster records, so the save changes live memory. A record with `flags & 1` set and `+3 == 0` is written with `+3 = 0` and loads as invalid, so it does not round-trip (#197 P6.4). Whether valid records with level 0 occur in play is **[open]**. The loader sets the in-use bit only for a nonzero level (`FUN_02003CD8`), so an in-use record with level 0 would be saved and then load as not in use. Both in-use records of a new game have level 5, and no creator of a record was found, so this is expected not to occur (**[hypothesis]**, medium; #239 P6.4). A port-side reject or assert on save is a suggested check. Team records 418–421 take validity from their 1-bit field, so a zero level does not change them, and the reader still overwrites their flags with 3 or 0 (#197 P6.5).
- **[lead]** Stream size: 413 × 323 + 4 × 324 + 112 = 134,807 bits = 16,851 bytes of the 0x4650 (18,000) section. Derived arithmetic.
- **[lead]** The section's header result is 7 (bit index after the last write). Not checked against a save.

**Team inventory** (`SaveTeamInventory` `020069C8`, `RestoreTeamInventory` `020068AC`)

- **[blue-verified]** 20 item slots × 23 bits, slot bytes 8, 7, 8 (`WriteItemSlotBits` `020067CC`); symmetric with `ReadItemSlotBits` (`0200680C`).
- **[blue-verified]** Storage: 240 counts × 10 bits, from the ushort at `+0x3C`.
- **[blue-verified]** Kecleon shop: 8 entries × 15 bits (`WriteHeldItemBits` 8 + 7), from `+0x21C`.
- **[blue-verified]** Kecleon wares: 4 entries × 15 bits, from `+0x22C`.
- **[blue-verified]** Money: 24 bits at `+0x234`; savings: 24 bits at `+0x238`.
- **[lead]** Total: 3,088 bits (386 bytes) of the `0x1D8`-byte section (arithmetic from the widths).
- **[blue-verified]** The bank is the savings account. Script id `0x24` `BANK_GOLD` (flag table A, entry 36) reads and writes `teamSavings` at `+0x238`, through `FUN_02020D90` (case 8) and the setters `FUN_020208E8` and `FUN_02020B48`. Script id `0x23` `GOLD` is `teamMoney` at `+0x234`. In-game dialogue names a bank ("Felicity Bank" and "Snobilikat-Bank" in overlays 11, 12 and 18). The 240-entry block is item storage: no script id maps to it, and the string search found no name for it as a bank (#197 P9). The `+0x238` base pointer, `gTeamInventoryRef` (`0x020DECF8`), is not re-checked here.

**Rescue team info and adventure data** (`SaveRescueTeamInfo` `02008758`, `SaveAdventureData` `02010F4C`)

- **[blue-verified]** Rescue team info widths: 0x58 bits from the struct at `*DAT_020087E8`, 0x20 bits at `+0x0C`, and a 1-bit flag at `+0x10` from `FUN_0201F994(0)`. Meanings **[open]**.
- **[lead]** Rescue team info is 121 bits in total (arithmetic from the widths).
- **[lead]** pmd-red: `learnedMoves` is 13 x 32 bits; not yet matched to a Blue sub-block.
- **[blue-verified]** Adventure sub-block widths: 15 bits (`FUN_0200C870`, `0x0200C870`), 32 bits (`FUN_0200CD64`, `0x0200CD64`), 1,463 bits (`FUN_02010520`, `0x02010520`), and `FUN_020112C8` (`0x020112C8`). Meaning of the flag arrays **[open]**.
- **[lead]** `FUN_020112C8` is 532 bits (arithmetic from the widths): 1 + 424 + 64 + 31 + 12 one-bit flags, indexed by ID through `FUN_0201151C`, `FUN_02011570`, `FUN_02011470`.
- **[lead]** Adventure data is 2,042 bits in a 2,048-bit section (arithmetic from the widths).
- **[blue-verified]** `DAT_02010ACC` is the serialized struct. The cells `DAT_02010ACC`, `DAT_02010648`, `DAT_02010E18` and `DAT_02010DF8` all hold `0x020DF6FC` (static cell values; the runtime value of that cell was not read). `FUN_02010520` serializes through `*DAT_02010648`, and `FUN_0201064C` writes through `*DAT_02010ACC`, so both are the same struct (#197 P5.1).
- **[blue-verified]** `FUN_0201064C` (`0x0201064C`) runs before the adventure write. It ORs species bits (the id split by 32, into the word at `+0x18 + 4·(id/32)`) and move bits (into `+0x88`) from each roster record with `flags & 1`. It sets the `+0x16` count (short) and calls the unlock setter `FUN_02010DFC` for IDs `0x0D`, `0x0E`, `0x0F`, `0x10`–`0x1F` and `0x07`. Every write lands in a serialized field, so the save changes the live adventure struct (#197 P5.2, P5.3). Repeated saves give the same bits, by the code's ORs and recount. This is argued from the code, not run (**[hypothesis]**, #197 P5.4).
- **[blue-verified logic; hypothesis names]** `FUN_02008FC8` returns 1 only when the friend-area bytes at indexes 1 to 0x39 are all nonzero. `FUN_0201064C` calls it only while unlock `0x0D` is unset, then sets `0x0D`, so unlock `0x0D` means "all friend areas owned" (#239 P5-a). The species loop of `FUN_0201064C` skips the ids 0, 377 to 379 and 417 to 423; its third loop skips 0 and 355 to 357 (#239 P5-b). Unlocks `0x0E` and `0x0F` need every non-excluded bit in the `+0x50` and `+0x18` bitsets. Unlocks `0x10` to `0x1F` are set for particular ids: `0x92`→`0x10`, `0x91`→`0x11`, `0x90`→`0x12`, 414→`0x13`, 269→`0x14`, `0x10C`→`0x15`, 270→`0x16`, 275→`0x17`, 410→`0x18`, 411→`0x19`, `0x19C`→`0x1A`, 274→`0x1B`, `0x114`→`0x1C`, `0x97`→`0x1D`, `0x96`→`0x1E`, 413→`0x1F`. The meaning of each unlock, of the excluded ids and the roster-id to species mapping are **[open]** (#197 P5).

**Other sections** (#197 P8; decompile only, widths blue-verified, meanings open)

- **4B98 sub-block** (section `0x4B98`, 0x594 bytes; save `sub_8095624` `0x0200D0CC`, restore `sub_80954CC` `0x0200D2B8`). 32 records of 0x30 bits through `sub_8095824` (`0x0200CEEC`), then one record at `*DAT_0200D2B0` with the same shape as a Pokémon record (its moves through `FUN_020094A0`, its held item through `WriteHeldItemBits`), then 0x20 bits and 32 words of 0x20 bits. Restore mirrors the save field by field.
- **Record 0 (#239 P8; [blue-verified] field widths, [red-hypothesis] names)**: its cell `0x020DF6EC` is the cell `FUN_0200D4DC` writes, so record 0 is the mail record of that function. Save widths from `FUN_0200CEEC`: state 4 bits (`+0`, set to 1), dungeon location as two 7-bit fields (`+4`), seed 24 bits (`+8`), client species 9 bits (`+0xC`), a 32-bit random value (`+0x10`), an 80-bit name text (`+0x14`), the `unk418` nonce 32 bits (`+0x24`), `rescuesAllowed` 8 bits (`+0x2C`) and one flag bit (`+0x2D`). Not interpreted: `+0x1E`, `+0x1F`, `+0x20` (three bytes), `+0x28` (32 bits), the Pokémon-shaped record at `*0x020DF6F0`, and the trailing 0x20 bits and 32 words at `*0x020DF6F4`.
- **Friend areas** (section `0x4A90`): 58 one-bit flags, one per element of the byte array at `0x020DED04`. 58 matches Red's friend-area count (red-hypothesis); the bit means "area owned" (hypothesis, #239 P8). Save `SaveFriendAreas` `0x02008D30`, restore `ReadSavedFriendAreas` `0x02008CB0`. Symmetric by decompile.
- **Mail** (section `0x512C`): 4 + 8 + 8 items at stride 0x14, 56 + 1 flag bits, 0x140 and 0x3C0 bit blocks, and 16 entries at stride 0xC. Save `SaveMailInfo` `0x0200DB80`, restore `RestoreMailInfo` `0x0200DD5C`. Symmetric by decompile. Entry widths of `FUN_0200DA08` (#239 P8): 4, 3, 4, 9, 9, 8, 4, 8, 6 and 24 bits, plus a two-field dungeon location per entry; the cell is `0x020DF6F8`. The 4 + 8 + 8 entries have the shape of Wonder-Mail mission slots (**[red-hypothesis]**; not matched field by field). Which of these widths is the item width is not decoded.
- The restores return the bit position from the struct's trailing field. This is how the section result is compared, not a bug.

**Story flags** (`SaveGlobalScriptVars` `0201F884`, `RestoreGlobalScriptVars` `0201F81C`)

- **[blue-verified]** The 0x400-byte script-variable block is copied raw (`thunk_FUN_02021F5C`).
- **[blue-verified]** The debug asserts are empty in retail: `FUN_02029D8C` and `FUN_02029DAC` decompile to a bare `return`. The bounds checks in `FUN_02021108` and in the getters and setters (`FUN_02020D90`, `FUN_020208E8`, `FUN_02020B48`, `FUN_02020F74`) are therefore not run, so an out-of-range flag index is not caught (#197 P4a).
- **[blue-verified]** Descriptor tables of 16-byte entries: table A (83 entries at `0x02098060`, base word `0x021EE640`) and table B (index `0x400` and up; 4 valid entries by the layout, medium confidence). Entry: `+0` type (1 bit, 2 or 7 byte, 3 s8, 4 u16, 5 s16, 6 s32, 8 special); `+4` offset in the block; `+6` bit shift; `+8` count; `+0xA` a value read only by `RestoreGlobalScriptVars`; `+0xC` pointer to the debug name (#197 P4b).
- **[blue-verified]** The only check that runs is in `RestoreGlobalScriptVars` (`0x0201F81C`). It compares `VERSION` (table A id 0, expected 30 at `+0xA`) with the first word of the block, and a mismatch sets the load result to 4. It runs only in the branch of `ReadSaveFromPak` where `*DAT_0202A41C == 0`. The other branch copies the block with no check. So `VERSION == 30` is the only check, and only on that branch (#197 P4c). The meaning of 30 is not confirmed against a real save.

## 3. Corrupt or blank detection

**Load result** (`ReadSaveFromPak` `0202A1AC`; the codes are Blue's, tied to addresses where marked)

| Value | Meaning | Status |
|-------|---------|--------|
| 0 | valid | [lead]; the valid-copy runs are not done (#197 P1.5; follow-up [#260](https://github.com/TorratDev/rust-mm/issues/260)) |
| 1 | read failed: `ReadFlashData` gave up after 4 tries (its return 3), or the alternate branch failed. Copy B is tried. In normal play this is the four-failure case; with the session pointer set and no device it is immediate (#239 P1.2, P7.7) | [blue-verified] code; a forced status 1 leads to the state-5 stall (section 4); the forced run is not a real flash failure |
| 2 | checksum mismatch (`ValidateChecksum` returns true on mismatch). Copy B is tried | [blue-verified] |
| 3 | a section's restore result does not match the header | [lead] |
| 4 | magic mismatch after the copy-A check, with no copy-B fallback; or `VERSION != 30` in `RestoreGlobalScriptVars` (first branch only) | [blue-verified] |

**Blank and zeroed cards**

- **[blue-verified runtime]** Blank card (no save; DeSmuME RAM bytes `FF`): both copies return status 2, the boot read result is 2, and the state machine goes 2 → 4 → 6 at frame 2473 with no message (#197 P1.3). This is DeSmuME's empty backup, not a real card. A second blank-card run (#239 P1.5), with a scratch copy of a local DeSmuME card whose save area is blank (`0xFF` over both copies; only the marker sector differs), gives the same result: both copies return status 2 and the read buffers start with `0xFF`. Its output hash is in [`verification.md`](../verification.md#239--save-format-remaining-findings). The valid-copy cases are not run.
- **[blue-verified, injected]** Zeroed copy A (zeros written at the `ReadSaveSector` return, so a fault injection and not a real card): the checksum passes (word 0 = 0 = sum), `ReadSaveFromPak` returns 4 from the magic check, and copy B is not read (#197 P1.4). A real all-zero card is not tested: importing a zeroed backup file did not reach the game in this build.
- **[open]** Whether a failed copy A followed by a failed copy B is reported as a read failure, and what the game does then.

**Sector read** (`ReadSaveSector` `0x02029E2C`)

- **[blue-verified]** Status 0 is valid. Status 1 is read failed: `ReadFlashData` returned 3 after its 4 tries, or the alternate branch returned 1 with no device. Status 2 is a checksum mismatch (`ValidateChecksum` `0x02029F98`, which returns true on mismatch).
- Codes 1, 2 and 4 of the `ReadSaveFromPak` table above are tied to addresses. Code 0 stays [lead] until a valid copy is run (#260), and code 3 stays [lead] until its section check is decompiled.

## 4. Open runtime checks

These need a run on Blue with `py-desmume` and the scripts in `verification/v7-trace/`. Outputs go to a scratch directory outside the repository; only the hash and the command are recorded in notes. The runs and their results are in [`verification.md`](../verification.md#197--save-load-and-session-runs) and [`verification.md`](../verification.md#239--save-format-remaining-findings).

- **[blue-verified runtime]** Load states from `save_load_probe.py`: a blank card (no save), an injected zeroed copy A, and a forced status 1 on both copies. Section 3 has the results.
- **[open]** Copy A valid, copy A bad with copy B valid, and both bad, with real data (#197 P1.5). No valid save image is available for these runs; they move to follow-up [#260](https://github.com/TorratDev/rust-mm/issues/260). A real save is user data and stays local.
- **[blue-verified runtime]** The state-5 stall: forced status 1 moves `FUN_0202AA14` from state 2 (mode 1) to state 5 at frame 2459, and state 5 has no exit, so the call repeats (1,543 more calls to frame 4000). In normal play the static trigger of status 1 is four failed DS flash reads (#239 P1.2). A real flash failure was not injected, so that is **[open]** at runtime. The alternate branch failing is the other trigger (P7).
- **[blue-verified runtime]** Size global `0x020F38B4` = `0x40000` and sector unit `0x020F38B8` = `0x1000` at the title screen.
- **[open]** Which player-facing scene reaches the alternate read branch (#197 P7.3; the call chain is in section 1, #239 P7.3). The boot-to-dungeon run did not enter it. No cartridge was present in any run, so whether the device is ever present in play is **[open]** (#197 P7.5, P7.7).
- **[open]** Friend areas and mail: the layouts are read (section 2), but their meanings are not checked. The role of `unk418` (a per-save identity token, hypothesis) needs runtime values. `unk41C`'s uses are settled by static reads; its values in a real save are not checked.
- **[open]** Marker erase (X.1): `MemoryAlloc` does not zero (#239 X.1). Whether a reader uses the bytes after the marker is open.
- **[open]** Quick-save payload size: 17,363 bytes in one state, under `0x4800` (#239 P3.7). Whether it fits in every state is open (#197 P3.7).
- **[open]** P6.4 (a valid roster record at level 0; no creator found), P6.3 (team flag values other than 0 and 3), P2c (who writes roster `+1 = 0x02` and who sets the ginseng after a dungeon), P5 (the unlock meanings and the roster-id to species mapping), and P8 (the meanings of the 4B98, friend-area, mail and `unk418` fields).
- **[open]** Check the whole layout on a real save file (local only, never committed).

## Links

- Symbols: [`symbols.md`](../symbols.md)
- Verification: [`verification.md`](../verification.md)
- RNG state: [`re/rng.md`](../re/rng.md)
- Legal: [`legal.md`](../legal.md)
