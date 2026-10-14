# Symbols

Names for Blue's functions and globals, one row each. This file is the machine-readable side of the glossary (#43): [`tools/ghidra/RescueApplySymbols.java`](../tools/ghidra/RescueApplySymbols.java) reads the table below and applies every row to a Ghidra project built as in [`ghidra-setup.md`](ghidra-setup.md), then checks it.

Dump: `BlueRescueTeamEur`. Rules for each column:

- **Address:** RAM address in hex. Prefix `ov_NNNN::` for code or data in an overlay; no prefix means the ARM9 binary.
- **Type:** a C prototype for a function (its name must match the Name column), a data type for a global (`int`, `uint[5]`, ...), or `-` for a plain label.
- **Tag:** `blue-verified` (role read in Blue's code), `red-hypothesis` (role taken from `pmd-red`, not yet checked in Blue), `sdk-hypothesis` (NitroSDK function recognised by its behaviour), `tentative` (our own name, role only partly known).
- **Evidence:** why the name fits. Names that come from `pmd-red` keep its spelling.

Topic pages: [`re/rng.md`](re/rng.md), [`re/execution-model.md`](re/execution-model.md).

| Address | Name | Type | Tag | Evidence |
|---------|------|------|-----|----------|
| 0201F2E4 | GameLoop_Async | void GameLoop_Async(void) | blue-verified | Called from the boot code at `02000D2C`; same init calls, order and constant arguments as `pmd-red` `GameLoop_Async`, then the title-menu loop |
| 0201F268 | MainLoops_RunFrameActions | void MainLoops_RunFrameActions(uint unused) | blue-verified | Called once per frame with `0` in every wait loop, like `pmd-red` |
| 0201E3C8 | RunGameMode_Async | uint RunGameMode_Async(uint mode) | blue-verified | Title-menu choices call it with modes 0–3, like `pmd-red` |
| 0201E204 | LoadAndRunQuickSaveDungeon_Async | void LoadAndRunQuickSaveDungeon_Async(void * setup) | blue-verified | Same steps as `pmd-red`: two `MemoryAlloc(…, 7)`, quicksave read loop, then `LoadAndRunDungeon_Async` and `SetWindowBGColor` |
| 0201DB70 | LoadAndRunDungeon_Async | void LoadAndRunDungeon_Async(void * setup) | blue-verified | Unloads `ov_0000`, loads the language text overlay and `ov_0001`, runs `RunDungeon_Async`, unloads them, reloads `ov_0000` |
| 0206DC20 | RunDungeon_Async | void RunDungeon_Async(void * setup) | blue-verified | Dungeon RNG seeding matches `pmd-red` `RunDungeon_Async` (see `re/rng.md`) |
| 0201DCEC | NDS_LoadOverlay_GroundMain | void NDS_LoadOverlay_GroundMain(void) | blue-verified | Loads and starts overlay 0; `pmd-red` keeps it as an empty stub at the same place in `GameLoop_Async` |
| 0201DCB4 | NDS_UnloadOverlay_GroundMain | void NDS_UnloadOverlay_GroundMain(void) | tentative | Unloads overlay 0; NDS only |
| 0201DAC4 | RunOverlay7Mode | void RunOverlay7Mode(void) | tentative | Title-menu choice 5 (not in Red): swaps overlay 0 for overlay 7 and calls its entry `ov_0007::02139F80` |
| 02029C88 | NDS_DebugInit | void NDS_DebugInit(void) | blue-verified | `GameLoop_Async` call order |
| 02029C50 | ResetSoundEffectCounters | void ResetSoundEffectCounters(void) | blue-verified | `GameLoop_Async` call order |
| 02029CF4 | NDS_DebugPrintf | - | tentative | Takes a format string and arguments, for example `"GroundStation_SetBank %d"` |
| 0202F344 | ResetDialogueBox | void ResetDialogueBox(void) | blue-verified | `GameLoop_Async` and `LoadAndRunQuickSaveDungeon_Async` call order |
| 02003794 | LoadMonsterParameters | void LoadMonsterParameters(void) | blue-verified | `GameLoop_Async` call order |
| 0200CEC4 | InitializePlayTime | void InitializePlayTime(void) | blue-verified | `GameLoop_Async` call order |
| 0200CB28 | LoadGameOptions | void LoadGameOptions(void) | blue-verified | `GameLoop_Async` call order |
| 0200C63C | SetWindowBGColor | void SetWindowBGColor(void) | blue-verified | `GameLoop_Async` and `LoadAndRunQuickSaveDungeon_Async` call order |
| 0201181C | LoadExclusivePokemon | void LoadExclusivePokemon(void) | blue-verified | `GameLoop_Async` call order |
| 020092A4 | LoadFriendAreas | void LoadFriendAreas(void) | blue-verified | `GameLoop_Async` call order |
| 020086B4 | LoadItemParameters | void LoadItemParameters(void) | blue-verified | `GameLoop_Async` call order |
| 020089A4 | LoadRescueTeamInfo | void LoadRescueTeamInfo(void) | blue-verified | `GameLoop_Async` call order |
| 0200B940 | LoadWazaParameters | void LoadWazaParameters(void) | blue-verified | `GameLoop_Async` call order |
| 0200FCF4 | LoadMailInfo | void LoadMailInfo(void) | blue-verified | `GameLoop_Async` call order |
| 0200CAA4 | InitializeGameOptions | void InitializeGameOptions(bool initializeGender) | blue-verified | `GameLoop_Async` call order, argument `1` |
| 02018D7C | SetWindowTitle | void SetWindowTitle(char * title) | blue-verified | `GameLoop_Async` call order, a build string argument |
| 02012AE8 | SetSavingIconCoords | void SetSavingIconCoords(void * pos) | blue-verified | `GameLoop_Async` call order, argument `NULL` |
| 0202A1AC | ReadSaveFromPak | uint ReadSaveFromPak(uint * a) | blue-verified | `GameLoop_Async` call order; restores the global RNG state with `SetRNGSeed` |
| 02021310 | ThoroughlyResetScriptVars | void ThoroughlyResetScriptVars(void) | blue-verified | Called when `ReadSaveFromPak` returns non-zero, like `pmd-red` |
| 02022188 | MemoryAlloc | void * MemoryAlloc(int size, int group) | blue-verified | Size and group arguments; result freed with `MemoryFree` |
| 02022170 | MemoryFree | void MemoryFree(void * a) | blue-verified | Frees `MemoryAlloc` results |
| 0200D58C | GetFirstIndexofMailType | int GetFirstIndexofMailType(uchar mailType) | blue-verified | Title-menu choice 2 calls it with `7`, like `pmd-red` |
| 0200D79C | DeleteMailAtIndex | void DeleteMailAtIndex(uchar index) | blue-verified | Follows `GetFirstIndexofMailType`, like `pmd-red` |
| 0200E8BC | GeneratePelipperJobs | void GeneratePelipperJobs(void) | blue-verified | `LoadAndRunQuickSaveDungeon_Async` call order |
| 0202B0B8 | PrepareQuickSaveRead | void PrepareQuickSaveRead(void * buf, int size) | blue-verified | `LoadAndRunQuickSaveDungeon_Async` call order |
| 0202AFD4 | ReadQuickSave | int ReadQuickSave(void) | blue-verified | Polled once per frame until zero |
| 0202AFC0 | IsQuickSaveValid | int IsQuickSaveValid(void) | blue-verified | `LoadAndRunQuickSaveDungeon_Async` call order |
| 0202AF8C | FinishQuickSaveRead | void FinishQuickSaveRead(void) | blue-verified | `LoadAndRunQuickSaveDungeon_Async` call order |
| 02029C14 | StopBGMResetSoundEffectCounters | void StopBGMResetSoundEffectCounters(void) | blue-verified | `LoadAndRunQuickSaveDungeon_Async` call order |
| 02011A20 | TransferBGPaletteBuffer | void TransferBGPaletteBuffer(void) | blue-verified | Copies dirty 16-colour rows to palette RAM `05000000`; called from `MainLoops_RunFrameActions` |
| 0202A040 | GetLanguage | int GetLanguage(void) | blue-verified | Returns `gLanguage`; indexes `gLanguageTextOverlayIds`; `-1` before a language is chosen |
| 020E0488 | gLanguage | int | blue-verified | Read by `GetLanguage` |
| 020DF90C | gLanguageTextOverlayIds | int[5] | blue-verified | Overlay id per language (2, 4, 3, 5, 6), loaded by `LoadAndRunDungeon_Async` |
| 020DB8E0 | gGroundBankTable | - | blue-verified | 246 entries of {overlay id, pointer into it}, overlays 10–144; read by `GroundStation_SetBank` |
| 0207BED8 | FS_LoadOverlayInfo | int FS_LoadOverlayInfo(void * info, int target, int id) | sdk-hypothesis | Copies the 0x20-byte overlay table entry `id` |
| 0207BB28 | FS_UnloadOverlay | int FS_UnloadOverlay(int target, int id) | sdk-hypothesis | Loads the overlay info, then unloads |
| 0207BC9C | FS_StartOverlay | void FS_StartOverlay(void * info) | sdk-hypothesis | Runs the overlay's static initialisers |
| 0207B7F8 | FS_InitFile | void FS_InitFile(void * file) | sdk-hypothesis | Called on a stack file object before every overlay load |
| 0207BE40 | FS_LoadOverlayImageAsync | int FS_LoadOverlayImageAsync(void * info, void * file) | sdk-hypothesis | Overlay load sequence |
| 0207B390 | FS_WaitAsync | int FS_WaitAsync(void * file) | sdk-hypothesis | Overlay load sequence |
| 0207B460 | FS_CloseFile | int FS_CloseFile(void * file) | sdk-hypothesis | Overlay load sequence |
| ov_0000::0214FC50 | GroundStation_SetBank | int GroundStation_SetBank(int bank) | blue-verified | Name from its own debug message; loads or unloads a ground bank overlay |
| ov_0000::0214FD00 | GroundStation_LoadBank | void GroundStation_LoadBank(int bank) | tentative | Loads the overlay of `gGroundBankTable[bank]` |
| ov_0000::0214FCA4 | GroundStation_UnloadBank | void GroundStation_UnloadBank(void) | tentative | Unloads the current bank overlay |
