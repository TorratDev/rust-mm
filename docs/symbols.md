# Symbols

Names for Blue's functions and globals, one row each. This file is the machine-readable side of the glossary (#43): [`tools/ghidra/RescueApplySymbols.java`](../tools/ghidra/RescueApplySymbols.java) reads the table below and applies every row to a Ghidra project built as in [`ghidra-setup.md`](ghidra-setup.md), then checks it.

Dump: `BlueRescueTeamEur`. Rules for each column:

- **Address:** RAM address in hex. Prefix `ov_NNNN::` for code or data in an overlay; no prefix means the ARM9 binary.
- **Type:** a C prototype for a function (its name must match the Name column), a data type for a global (`int`, `uint[5]`, ...), or `-` for a plain label.
- **Tag:** `blue-verified` (role read in Blue's code), `red-hypothesis` (role taken from `pmd-red`, not yet checked in Blue), `sdk-hypothesis` (NitroSDK function recognised by its behaviour), `tentative` (our own name, role only partly known).
- **Evidence:** why the name fits. Names that come from `pmd-red` keep its spelling.

Naming rules:

- **Canonical address:** the uppercase table form, for example `ov_0001::0217BA94`. Prose uses the same form. Ghidra accepts the lowercase form too, but the repo does not use it.
- **`sub_` names:** the suffix is the address in `pmd-red`, not in Blue. The name is unverified until Blue's function is checked, and the 14 `sub_` rows keep their names until then.
- **`FUN_` names:** Ghidra's unnamed defaults. They are not rows here; a function gets a row only when its role is known.

Topic pages: [`formats/save.md`](formats/save.md), [`re/rng.md`](re/rng.md), [`re/execution-model.md`](re/execution-model.md), [`re/combat.md`](re/combat.md), [`re/turns.md`](re/turns.md), [`re/structs.md`](re/structs.md), [`re/dungeon-generation.md`](re/dungeon-generation.md).

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
| 0201DAC4 | RunOverlay7Mode | void RunOverlay7Mode(void) | blue-verified | Title-menu choice 5 (not in Red): loads overlay 7, calls its entry `ov_0007::02139F80`, then reloads overlay 0. Called from `GameLoop_Async` at `0201F7E8` |
| 02029C88 | NDS_DebugInit | void NDS_DebugInit(void) | blue-verified | `GameLoop_Async` call order |
| 02029C50 | ResetSoundEffectCounters | void ResetSoundEffectCounters(void) | blue-verified | `GameLoop_Async` call order |
| 02029CF4 | NDS_DebugPrintf | - | tentative | Takes a format string and arguments, for example `"GroundStation_SetBank %d"` |
| 0202F344 | ResetDialogueBox | void ResetDialogueBox(void) | blue-verified | `GameLoop_Async` and `LoadAndRunQuickSaveDungeon_Async` call order |
| ov_0001::0217AD64 | GetDefaultMonsterAnimId | uint GetDefaultMonsterAnimId(void * entity) | blue-verified | Returns the animation a monster shows when not acting: 5 while asleep, 11 while bide-ing, else 7; Sudowoodo with 127 turns asleep stays on 7. Same logic as `pmd-red` `sub_806CEBC` |
| ov_0001::0217ADF8 | SetDefaultMonsterAnim | void SetDefaultMonsterAnim(void * entity, int dir) | blue-verified | Monsters only: sets `entity+0x6A` to `GetDefaultMonsterAnimId` and `entity+0x6C` to `dir` (0–7) |
| ov_0001::0217AF58 | SetMonsterAnim | void SetMonsterAnim(void * entity, int anim) | blue-verified | Explicit animation setter (Red `sub_806CDD4`). Most callers pass the default helper; `ov_0001::02182900` passes the literal 7. It also stores `entity+0x64` (the sprite handle, see `GetMonsterSpriteHandle`) into the animation state with `FUN_020128dc`, never an animation ID. Consumes one global `Rand32Bit` step per call; 13 call sites in overlay 1 (see [`re/status.md`](re/status.md#open-questions)). |
| ov_0001::02178C04 | GetMonsterSpriteHandle | uint GetMonsterSpriteHandle(int species) | blue-verified | Returns `*(dungeon + 0x158DC + species*4)`. The callers store it at `entity+0x64`, and `SetMonsterAnim` passes it to `FUN_020128dc` (`FUN_02012848(state, handle+0xC, anim, dir, ...)`) as the sprite resource. It is a per-species sprite handle, not a walk or hit animation ID |
| ov_0001::0217AED0 | SetMonsterAnimAndDir | void SetMonsterAnimAndDir(void * entity, uint8 anim, int dir) | blue-verified | Monsters only (entity type 1): sets `entity+0x6A` to `anim`; sets `entity+0x6C` to `dir` when `0 <= dir < 8` (8 keeps the current direction). The direct way the game picks walk (0), hit (6) and cutscene poses; about 55 call sites, see `formats/sprites.md` |
| ov_0001::0217AE88 | SetMonsterAnimAndDirIfChanged | void SetMonsterAnimAndDirIfChanged(void * entity, uint anim, uint dir) | blue-verified | Calls `SetMonsterAnimAndDir` unless `entity+0x6B == anim` and `entity+0x6D == dir` |
| ov_0001::0213DD38 | PlayMoveAttackerAnim | void PlayMoveAttackerAnim(void * entity, void * move) | blue-verified | Gets the animation from `GetMoveAnimId`, applies it with `SetMonsterAnimAndDir` and waits up to 120 frames for the finish bit (`entity+0x40` bit 0); bit 1 fires the move effect. 98 and 99 are spin sentinels |
| ov_0001::0215B218 | PlayMoveAnim | uint PlayMoveAnim(void * entity, void * move, int a, void ** targets, uint b, char c) | blue-verified | Calls effect setup `0213E260`, then `PlayMoveAttackerAnim` (call at `0215B354`); an invisible attacker goes through `SetDefaultMonsterAnim` instead. Called three times by the move executor `0215B384` |
| ov_0001::0217ACF4 | WaitForOneShotAnimEnd | void WaitForOneShotAnimEnd(void * entity) | blue-verified | Runs `DungeonRunFrameActions(0x21)` up to 100 times while `entity+0x6B` is a one-shot group |
| ov_0001::0217B120 | UpdateMonsterSprite | void UpdateMonsterSprite(void * entity) | blue-verified | Per frame: applies `entity+0x6A` through `FUN_020128dc`, copies it to `+0x6B`, and after a one-shot picks the follow-up animation from the table at `021AE1D0` |
| ov_0001::0219E6AC | SetAnimForced | void SetAnimForced(void * entity, uchar anim) | blue-verified | Cutscene helper: sets the animation, then stores `anim` at `monster+0xDB` (99 means none) |
| 02023E9C | GetMoveAnimId | uchar GetMoveAnimId(int species, int move) | blue-verified | Default anim byte `+0x10` of the 36-byte move entry at `02098CF0`, replaced by a matching `{species, anim, sfx}` record from the table at `020A0868` |
| 02002438 | IsOneShotAnim | uchar IsOneShotAnim(int species, uint anim) | blue-verified | Byte table `0208BEAC` (`0 1 1 1 1 0 ...`) for anim < 13; for 13 and up a species list at `0208BEF8` |
| 02025674 | GetMoveEntry | void * GetMoveEntry(int move) | blue-verified | `DAT_02025684 + move*0x24`: the 36-byte move entry at ARM9 `02098CF0` |
| 02025688 | GetMoveEntry2 | void * GetMoveEntry2(int move) | blue-verified | Same stride, base `DAT_02025698`: the second table indexed by the move's four ids (`+0x00..+0x06`) |
| 020241C0 | MoveHasTypeFourAt | bool MoveHasTypeFourAt(void * move, int slot) | blue-verified | Reads move word `slot` (0..3), looks it up in `GetMoveEntry2`, returns whether that entry's first word is 4. Used in `PlayMoveAttackerAnim` |
| 02024238 | MoveHasTypeFour | bool MoveHasTypeFour(void * move) | blue-verified | Any of the four move words whose `GetMoveEntry2` entry has first word 4 |
| 020242D4 | GetMoveSfxId | uint GetMoveSfxId(int species, int move) | blue-verified | Override sfx from the `020A0868` record (`+4`), else the default at move entry `+0x18` |
| 02012848 | InitSpriteState | void InitSpriteState(ushort * state, void * sprite, int group, int dir, ...) | blue-verified | Sets a sprite state from `sprite->group table[group*4][dir*4]`; no bounds check on `group` |
| 020020A0 | GetPokemonOverworldPalette | uchar GetPokemonOverworldPalette(short species, bool recolorShopKecleon) | blue-verified | Returns the species byte at `+8` of its 0x48-byte `monspara` entry (`overworldPalette`); returns 10 when `recolorShopKecleon` is set. Like `pmd-red` `GetPokemonOverworldPalette`. Its caller `FUN_02028628` stores the result as the monster sprite's palette at struct `+0x52D4` |
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
| ov_0000::0214FD00 | GroundStation_LoadBank | void GroundStation_LoadBank(int bank) | blue-verified | Loads the overlay of `gGroundBankTable[bank]` with `FS_LoadOverlayInfo`; called by `GroundStation_SetBank` |
| ov_0000::0214FCA4 | GroundStation_UnloadBank | void GroundStation_UnloadBank(void) | tentative | Unloads the current bank overlay |
| ov_0001::0217BD94 | CalcDamage | void CalcDamage(void * attacker, void * target, uchar moveType, int movePower, int critChance, void * dmgStruct, int arg8, ushort moveId, bool arg_10) | blue-verified | Calls `sub_806E100`; same constants as `pmd-red` (`999`, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`) and the same three `DungeonRandInt` calls in order |
| ov_0001::0217CE88 | sub_806E100 | bool sub_806E100(void * param_1, void * pokemon, void * target, uchar type, void * dmgStruct) | blue-verified | Reads `gTypeEffectivenessChart`; type is the 4th of 5 arguments, like `pmd-red` |
| ov_0001::0217CB3C | WeightWeakTypePicker | int WeightWeakTypePicker(void * user, void * target, uchar moveType) | blue-verified | Reads `gTypeEffectivenessChart`; 3 arguments, like `pmd-red` |
| ov_0001::0217BB94 | sub_806F370_Async | void sub_806F370_Async(void * pokemon, void * target, int dmg, int giveExp, bool * tookNoDamage, uchar moveType, short dungeonExitReason, int residualDmgType, int arg_10, int arg_14) | blue-verified | Same 10 arguments as `pmd-red`; Wonder Guard check through `gEffectivenessChart`, then `HandleDealingDamage_Async` |
| ov_0001::0217BA94 | sub_806F4A4 | uchar sub_806F4A4(void * pokemon, uchar type) | blue-verified | Returns 1 when the move type is not immune to either of the target's two types (chart entries in `gTypeEffectivenessChart` compared with `EFFECTIVENESS_IMMUNE` = 0); 0 when immune to one, or for the Ghost exception |
| ov_0001::0217E890 | HandleDealingDamage_Async | void HandleDealingDamage_Async(void * attacker, void * target, void * dmgStruct, bool isFalseSwipe, bool giveExp, short dungeonExitReason, bool arg8, int argC) | blue-verified | Called by `sub_806F370_Async` with 8 arguments, like `pmd-red`. Nightmare damage reaches it through `0217BCA4` (sleep class expiry, case 3); see [`re/status.md`](re/status.md#sleep-class-0x99-0x9a). |
| ov_0001::0218067C | AbilityIsActive | bool AbilityIsActive(void * entity, uchar ability) | blue-verified | Called with `0x35` (`ABILITY_WONDER_GUARD`) in `sub_806F370_Async`, like `pmd-red` |
| ov_0001::021AAEE8 | gTypeEffectivenessChart | short[18][18] | blue-verified | Same 648 bytes as `pmd-red` (V6); row stride `0x24` in its 4 readers |
| ov_0001::021AA588 | gEffectivenessChart | int[4][4] | blue-verified | Same 16 values as `pmd-red` |
| ov_0001::021AA660 | gAtkStatStageMultipliers | int[21] | blue-verified | Same 21 s24.8 values as `pmd-red`; read by `CalcDamage` |
| ov_0001::021AA6B4 | gDefStatStageMultipliers | int[21] | blue-verified | Same 21 s24.8 values as `pmd-red`; read by `CalcDamage` |
| 020E0788 | gDungeon | void * | blue-verified | Pointer to the dungeon struct; read by `RunDungeon_Async`, `IsFloorOver` and `RunFractionalTurn_Async` |
| ov_0001::021411BC | RunFractionalTurn_Async | void RunFractionalTurn_Async(bool param_1) | blue-verified | Same call sequence as `pmd-red`, each step followed by `IsFloorOver`; increments `dungeon+0x650` and wraps it at 24 |
| ov_0001::0214067C | IsFloorOver | bool IsFloorOver(void) | blue-verified | Same checks as `pmd-red`; sets `dungeon+0x648` to 1 or 2 |
| ov_0001::0214078C | TrySpawnMonsterAndActivatePlusMinus_Async | void TrySpawnMonsterAndActivatePlusMinus_Async(void) | blue-verified | `RunFractionalTurn_Async` call order |
| ov_0001::02140FAC | RunLeaderTurn_Async | bool RunLeaderTurn_Async(bool param_1) | blue-verified | `RunFractionalTurn_Async` call order; its result gates `UpdateWindTurns` |
| ov_0001::02195104 | UpdateWindTurns | void UpdateWindTurns(void) | blue-verified | `RunFractionalTurn_Async` call order |
| ov_0001::02140AC0 | sub_8044574_Async | void sub_8044574_Async(void) | blue-verified | `RunFractionalTurn_Async` call order |
| ov_0001::021408F8 | sub_8044820_Async | void sub_8044820_Async(void) | blue-verified | `RunFractionalTurn_Async` call order |
| ov_0001::021406F8 | sub_8044AB4 | void sub_8044AB4(void) | blue-verified | `RunFractionalTurn_Async` call order, last step |
| ov_0001::0219C23C | GetLeader | void * GetLeader(void) | blue-verified | Returns the cached `gLeaderPointer`, else the first valid `teamPokemon` entry (dungeon `+0x11314`) whose info has `isTeamLeader` set, like `pmd-red` |
| ov_0001::021B3F54 | gLeaderPointer | void * | blue-verified | Cache read and written by `GetLeader` |
| ov_0001::02142AE4 | EntityIsValid | bool EntityIsValid(void * entity) | blue-verified | Non-null and `type != 0`, like `pmd-red` |
| ov_0001::02181E58 | CalcSpeedStage | int CalcSpeedStage(void * pokemon) | blue-verified | Counts speed-up and speed-down counters, paralysis, species speed, Deoxys and Kecleon checks, clamps to 0–4, stores it; like `pmd-red` |
| ov_0001::02185C70 | TickStatusAndHealthRegen | void TickStatusAndHealthRegen(void * entity) | blue-verified | HP regeneration (clamped 30–500) then status countdowns, like `pmd-red`. Status chain order and the four call sites are in [`re/status.md`](re/status.md#per-turn-order). |
| ov_0001::02176B54 | TryActivateArtificialWeatherAbilities | void TryActivateArtificialWeatherAbilities(void) | blue-verified | `RunLeaderTurn_Async` call order |
| ov_0001::0218766C | DisplayActions_Async | bool DisplayActions_Async(void * a0) | blue-verified | `RunLeaderTurn_Async` call order, argument `NULL` |
| ov_0001::0213AFA8 | DungeonRunFrameActions | void DungeonRunFrameActions(uint a0) | blue-verified | `RunLeaderTurn_Async` call order, argument `12` |
| ov_0001::021ABC84 | gSpeedTurns | short[5][25] | blue-verified | Same values as `pmd-red`; indexed by speed stage and `fractionalTurn` |
| ov_0001::02151E7C | GenerateFloor | void GenerateFloor(void) | blue-verified | Only overlay-1 function with Red's 4 `DungeonRandRange` and 3 `DungeonRandInt` calls; same call order, same grid constants and layout switch |
| 0201A54C | OpenFileAndGetFileDataPtr | void * OpenFileAndGetFileDataPtr(char * filename, void * arc) | blue-verified | Opens `"fixedmap"` at the start of `GenerateFloor`, like `pmd-red` |
| 0201A22C | CloseFile | void CloseFile(void * openedFile) | blue-verified | Closes the files of `GenerateFloor` and the title screen, like `pmd-red` |
| 020817E0 | abs | int abs(int x) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::0214BDBC | ResetFloor | void ResetFloor(void) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::02150958 | ProcessFixedRoom | bool ProcessFixedRoom(int fixedRoomNumber, void * floorProps) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::02151BF4 | GenerateStandardFloor | void GenerateStandardFloor(int a0, int a1, void * a2) | blue-verified | Layout cases 0, 1, 8, 11 |
| ov_0001::0214FEB0 | GetGridPositions | void GetGridPositions(int * cellStartX, int * cellStartY, int gridSizeX, int gridSizeY) | blue-verified | `GenerateStandardFloor` step 1; writes `i * (56 / gridSizeX)` and `i * (32 / gridSizeY)`, no RNG |
| ov_0001::0214FD7C | InitDungeonGrid | void InitDungeonGrid(void * grid, int gridSizeX, int gridSizeY) | blue-verified | Step 1 of every layout generator; clears each 30-byte cell, marks columns invalid from `gStandardGridMode`, no RNG |
| ov_0001::0214FB4C | AssignRooms | void AssignRooms(void * grid, int gridSizeX, int gridSizeY, int numRooms) | blue-verified | `GenerateStandardFloor` step 3, room density as last argument; 129 RNG steps on the first floor like the retail trace (`floor_stages.py`) |
| ov_0001::0214F7A8 | CreateRoomsAndAnchors | void CreateRoomsAndAnchors(void * grid, int gridSizeX, int gridSizeY, int * cellStartX, int * cellStartY, uint roomFlags) | blue-verified | `GenerateStandardFloor` step 4; places rooms (room index on each tile) and anchors (`0xFE`); RNG budget matches the retail trace |
| ov_0001::021B4588 | gStandardGridMode | uchar | tentative | Set by `GenerateFloor` (0, 1 for the small layout, 2 for medium); `InitDungeonGrid` marks columns from `gridSizeX / 2` (1) or `gridSizeX * 3 / 4` (2) invalid |
| ov_0001::021B45BC | gNumRoomsAssigned | int | tentative | Rooms counted by `AssignRooms`' first pass |
| ov_0001::021B45C0 | gFloorPropsByteC | int | tentative | Floor-properties byte `+0x0C`, copied by `GenerateFloor` and cleared after its first attempt; gates a room flag in `CreateRoomsAndAnchors` |
| ov_0001::021506B0 | GenerateOneRoomMonsterHouseFloor | void GenerateOneRoomMonsterHouseFloor(void) | blue-verified | Layout case 2 |
| ov_0001::0215174C | GenerateOuterRingFloor | void GenerateOuterRingFloor(void * a0) | blue-verified | Layout case 3; 4 / 2 RNG calls like `pmd-red` |
| ov_0001::021512B4 | GenerateCrossroadsFloor | void GenerateCrossroadsFloor(void * a0) | blue-verified | Layout case 4; 4 / 2 RNG calls like `pmd-red` |
| ov_0001::021504F4 | GenerateTwoRoomsWithMonsterHouseFloor | void GenerateTwoRoomsWithMonsterHouseFloor(void) | blue-verified | Layout case 5; 2 / 2 RNG calls like `pmd-red` |
| ov_0001::02151160 | GenerateLineFloor | void GenerateLineFloor(void * a0) | blue-verified | Layout case 6 |
| ov_0001::02150FC0 | GenerateCrossFloor | void GenerateCrossFloor(void * a0) | blue-verified | Layout case 7 |
| ov_0001::02150DFC | GenerateBeetleFloor | void GenerateBeetleFloor(void * a0) | blue-verified | Layout case 9 |
| ov_0001::02150A10 | GenerateOuterRoomsFloor | void GenerateOuterRoomsFloor(int gridSizeX, int gridSizeY, void * floorProps) | blue-verified | Layout case 10 |
| ov_0001::02149A70 | ResetInnerBoundaryTileRows | void ResetInnerBoundaryTileRows(void) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::0214C0E0 | EnsureImpassableTilesAreWalls | void EnsureImpassableTilesAreWalls(void) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::02149A08 | GetTile | void * GetTile(int x, int y) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::0214D900 | FinalizeJunctions | void FinalizeJunctions(void) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::0214AAF8 | GenerateSecondaryTerrainFormations | void GenerateSecondaryTerrainFormations(uint flag, void * floorProps) | blue-verified | `GenerateFloor` call order and argument count; 3 / 9 RNG calls like `pmd-red` |
| ov_0001::0214B650 | SpawnNonEnemies | void SpawnNonEnemies(void * floorProps, bool isEmptyMonsterHouse) | blue-verified | `GenerateFloor` call order and argument count; 4 / 7 RNG calls like `pmd-red` |
| ov_0001::0214B338 | SpawnEnemies | void SpawnEnemies(void * floorProps, bool isEmptyMonsterHouse) | blue-verified | `GenerateFloor` call order and argument count; 2 / 2 RNG calls like `pmd-red` |
| ov_0001::0214C1A8 | ResolveInvalidSpawns | void ResolveInvalidSpawns(void) | blue-verified | `GenerateFloor` call order and argument count |
| 0206DB90 | GetFloorType | uchar GetFloorType(void) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::0214A760 | StairsAlwaysReachable | bool StairsAlwaysReachable(int stairsX, int stairsY, bool markUnreachable) | blue-verified | `GenerateFloor` call order and argument count |
| ov_0001::021498F8 | sub_80495E4 | void sub_80495E4(void) | blue-verified | Points `tilePointers[y][x]` at `tiles[y][x]` (stride 0x14) and clears `tileset`, like `pmd-red` |
| ov_0001::02149990 | GetTileMut | void * GetTileMut(int x, int y) | blue-verified | Like `GetTile`, but copies the out-of-bounds tile into `gOutOfBoundsTileData` first, like `pmd-red` |
| ov_0001::0214C0A8 | ResetTile | void ResetTile(void * tile) | blue-verified | Clears every tile field in `pmd-red`'s order, `room = 0xFF` |
| ov_0001::021B3ED0 | gCurTilesetOobTile | void * | blue-verified | Returned by `GetTile` outside the map |
| ov_0001::021B4270 | gOutOfBoundsTileData | - | blue-verified | 0x14-byte copy target in `GetTileMut` |
| ov_0001::021432E8 | HasHeldItem | bool HasHeldItem(void * pokemon, uchar id) | blue-verified | Held item exists, is not sticky and has this id, like `pmd-red` |
| ov_0001::02180644 | MonsterIsType | bool MonsterIsType(void * pokemon, uchar type) | blue-verified | Compares both types with `type`, false for type 0, like `pmd-red` |
| ov_0001::021805CC | IsTacticSet | bool IsTacticSet(void * pokemon, uchar tactic) | blue-verified | Team leader: `tactic == TACTIC_GO_THE_OTHER_WAY` (1); else compares `tactic`, like `pmd-red` |
| 02007A94 | GetItemCountInInventory | int GetItemCountInInventory(uchar id) | blue-verified | Counts existing team items with this id, 20 slots of 3 bytes, like `pmd-red` |
| 02008538 | GetNumberOfFilledInventorySlots | int GetNumberOfFilledInventorySlots(void) | blue-verified | Counts existing team items, like `pmd-red` |
| 020DECF8 | gTeamInventoryRef | void * | blue-verified | Inventory pointer read by both inventory functions |
| ov_0001::021803E0 | GetMovePower | int GetMovePower(void * pokemon, void * pokeMove) | blue-verified | Hidden Power (`0x144`) uses the info's `hiddenPower` power, else `GetMoveBasePower`; plus `ginseng`, like `pmd-red` |
| ov_0001::02180418 | GetMoveTypeForMonster | uchar GetMoveTypeForMonster(void * pokemon, void * pokeMove) | blue-verified | Hidden Power uses the info's `hiddenPower` type, else `GetMoveType`, like `pmd-red` |
| 0200B3E8 | GetMoveBasePower | int GetMoveBasePower(void * move) | blue-verified | Called by `GetMovePower` |
| 0200B4CC | GetMoveType | uchar GetMoveType(void * move) | blue-verified | Called by `GetMoveTypeForMonster` |
| 0200B3A4 | GetMoveBasePP | uint GetMoveBasePP(void * move) | blue-verified | Gives the initial PP in `InitPokemonMove` |
| 0200B568 | InitPokemonMove | void InitPokemonMove(void * move, ushort moveID) | blue-verified | Writes all five move fields, like `pmd-red` |
| 0200B510 | InitZeroedPPPokemonMove | void InitZeroedPPPokemonMove(void * move, ushort moveID) | blue-verified | Flags, id and `PP = 0`, like `pmd-red` |
| ov_0001::02141F58 | SpawnItemEntity | void * SpawnItemEntity(void * pos) | blue-verified | Takes a free slot in the 64 floor items, like `pmd-red` |
| ov_0001::021440B0 | SpawnItem | bool SpawnItem(void * pos, void * item, bool a2) | blue-verified | Same steps as `pmd-red` |
| ov_0001::02142ADC | GetEntityType | uint GetEntityType(void * entity) | blue-verified | Returns `entity->type` |
| ov_0001::02142AC4 | GetItemInfo | void * GetItemInfo(void * entity) | blue-verified | Returns the entity's info pointer (`+0x70`) |
| 02008588 | InitializeMoneyItems | void InitializeMoneyItems(void) | blue-verified | Clears 20 items, 240 storage counts, 8 shop items, money and savings, like `pmd-red` |
| 02007688 | AddToTeamMoney | void AddToTeamMoney(int amount) | blue-verified | Adds to `teamMoney` (`+0x234`) and clamps to 0–99,999, like `pmd-red` |
| 02007038 | CountKecleonShopItems | int CountKecleonShopItems(void) | blue-verified | Counts the 8 shop slots at `+0x21C`, like `pmd-red` |
| 02006D70 | CountKecleonWareItems | int CountKecleonWareItems(void) | blue-verified | Counts the 4 ware slots at `+0x22C`, like `pmd-red` |
| 02006DA4 | AddKecleonShopItem | bool AddKecleonShopItem(uchar itemIndex) | blue-verified | First empty shop slot, like `pmd-red` |
| 02006AE4 | AddKecleonWareItem | bool AddKecleonWareItem(uchar itemIndex) | blue-verified | First empty ware slot, like `pmd-red` |
| 02007014 | InitKecleonShopItem | void InitKecleonShopItem(uchar index) | blue-verified | Clears shop slot `index`, like `pmd-red` |
| 020083D4 | InitBulkItem | void InitBulkItem(void * item, uchar itemIndex) | blue-verified | Builds the `BulkItem` in both add functions, like `pmd-red` |
| 020190CC | InitFlash | void InitFlash(void) | blue-verified | Called from the boot code; `CARD_IdentifyBackup(0x1202)`, stores the size and the 0x1000 sector unit |
| 0207E598 | CARD_IdentifyBackup | bool CARD_IdentifyBackup(uint type) | sdk-hypothesis | Called by `InitFlash` with the backup type |
| 020F38B4 | gFlashSize | - | tentative | Backup size returned by the SDK in `InitFlash` |
| 020F38B8 | gFlashSectorSize | - | tentative | `0x1000`, multiplied by the sector number in `ReadFlashData` and `WriteFlashData` |
| 02018F0C | ReadFlashData | int ReadFlashData(int sector, void * dest, int size) | blue-verified | Backup read request (type 6), up to 4 attempts; called by `ReadSaveSector`, like `pmd-red` |
| 02018DC4 | WriteFlashData | int WriteFlashData(int sector, void * src, int size) | blue-verified | Backup write request (type 7), up to 4 attempts; called by `WriteSaveSector`, like `pmd-red` |
| 02029E2C | ReadSaveSector | uint ReadSaveSector(int * a, void * dest, int size) | blue-verified | Same steps as `pmd-red`: advance the sector, read, `ValidateChecksum` |
| 02029ECC | WriteSaveSector | uint WriteSaveSector(int * a, void * src, int size) | blue-verified | Same steps as `pmd-red`: advance the sector, `CalculateChecksum`, write |
| 02029F6C | sub_8011CA8 | void sub_8011CA8(uint * out, int size) | blue-verified | Advances the sector number by `(size + 0xFFF) / 0x1000`, like `pmd-red` |
| 02029FD4 | CalculateChecksum | void CalculateChecksum(void * out, uint size) | blue-verified | Word 0 = sum of the other 32-bit words, like `pmd-red` |
| 02029F98 | ValidateChecksum | bool ValidateChecksum(void * in, uint size) | blue-verified | True when word 0 differs from the sum, like `pmd-red` |
| 0202A53C | WriteSavetoPak | uint WriteSavetoPak(int * param_1, uint param_2) | blue-verified | Same header and eight sections as `pmd-red`, written twice; stores the language at `+0x424` |
| 0202A000 | GetGameInternalName | char * GetGameInternalName(void) | blue-verified | Copied to `+0x404` by `WriteSavetoPak`, like `pmd-red` |
| 0202A060 | sub_8011C34 | int sub_8011C34(void) | blue-verified | Gives header `+0x418` in `WriteSavetoPak` |
| 0202A050 | sub_8011C40 | void sub_8011C40(int x) | blue-verified | Takes header `+0x418` in `ReadSaveFromPak` |
| 0202A070 | sub_8011C28 | void sub_8011C28(uint x) | blue-verified | Takes header `+0x41C` in `ReadSaveFromPak` |
| 0202A010 | SetLanguage | void SetLanguage(int language) | tentative | Takes header `+0x424` in `ReadSaveFromPak`; also called by `GameLoop_Async` with the console's language |
| 0201F884 | SaveGlobalScriptVars | bool SaveGlobalScriptVars(void * dest) | blue-verified | `WriteSavetoPak` section order |
| 0201F81C | RestoreGlobalScriptVars | bool RestoreGlobalScriptVars(void * src) | blue-verified | `ReadSaveFromPak` section order |
| 0200409C | SaveRecruitedPokemon | int SaveRecruitedPokemon(void * a1, int a2) | blue-verified | `WriteSavetoPak` section order |
| 02003F2C | RestoreRecruitedPokemon | int RestoreRecruitedPokemon(void * a1, int a2) | blue-verified | `ReadSaveFromPak` section order |
| 02003AD0 | SavePoke2s | int SavePoke2s(void * buffer, int size) | blue-verified | `WriteSavetoPak` section order, size 600 |
| 020038AC | RestorePoke2s | int RestorePoke2s(void * a1, int size) | blue-verified | `ReadSaveFromPak` section order, size 600 |
| 020069C8 | SaveTeamInventory | int SaveTeamInventory(void * unk0, uint size) | blue-verified | `WriteSavetoPak` section order, size 0x1D8 |
| 020068AC | RestoreTeamInventory | int RestoreTeamInventory(void * unk0, uint size) | blue-verified | `ReadSaveFromPak` section order, size 0x1D8 |
| 02008758 | SaveRescueTeamInfo | uint SaveRescueTeamInfo(void * buffer, uint bufLen) | blue-verified | `WriteSavetoPak` section order, size 0x10 |
| 020086CC | ReadRescueTeamInfo | uint ReadRescueTeamInfo(void * buffer, uint bufLen) | blue-verified | `ReadSaveFromPak` section order, size 0x10 |
| 02008D30 | SaveFriendAreas | uint SaveFriendAreas(void * r0, uint size) | blue-verified | `WriteSavetoPak` section order, size 8 |
| 02008CB0 | ReadSavedFriendAreas | uint ReadSavedFriendAreas(void * r0, int size) | blue-verified | `ReadSaveFromPak` section order, size 8 |
| 02010F4C | SaveAdventureData | uint SaveAdventureData(void * buffer, uint bufLen) | blue-verified | `WriteSavetoPak` section order, size 0x100 |
| 02010EFC | RestoreAdventureData | uint RestoreAdventureData(void * buffer, uint bufLen) | blue-verified | `ReadSaveFromPak` section order, size 0x100 |
| 0200D0CC | sub_8095624 | uint sub_8095624(void * buffer, uint b) | blue-verified | `WriteSavetoPak` section order, size 0x594 |
| 0200D2B8 | sub_80954CC | uint sub_80954CC(void * buffer, uint size) | blue-verified | `ReadSaveFromPak` section order, size 0x594 |
| 0200DB80 | SaveMailInfo | uint SaveMailInfo(void * r0, uint size) | blue-verified | `WriteSavetoPak` section order, size 0x221 |
| 0200DD5C | RestoreMailInfo | uint RestoreMailInfo(void * r0, uint size) | blue-verified | `ReadSaveFromPak` section order, size 0x221 |
| 020E0490 | gUnknown_203B184 | void * | blue-verified | Debug save-state pointer tested by `ReadSaveFromPak` and `WriteSavetoPak`, like `pmd-red` |
| 0200C190 | InitBitWriter | void InitBitWriter(void * seri, void * buffer, int bufLen) | blue-verified | Sets the stream and clears the buffer, like `pmd-red` |
| 0200C1BC | InitBitReader | void InitBitReader(void * seri, void * buffer, int bufLen) | blue-verified | First call of every `Restore*` section function, like `pmd-red` |
| 0200C18C | FinishBitSerializer | void FinishBitSerializer(void * seri) | blue-verified | Empty, like `pmd-red` |
| 0200C0F4 | WriteBits | void WriteBits(void * seri, void * src, int numBits) | blue-verified | LSB-first bit writer, same loop as `pmd-red` |
| 0200C050 | ReadBits | void ReadBits(void * seri, void * dst, int numBits) | blue-verified | Used by every `Restore*` section function with the same widths |
| 020067CC | WriteItemSlotBits | void WriteItemSlotBits(void * seri, void * slot) | blue-verified | 8, 7, 8 bits, like `pmd-red` |
| 0200680C | ReadItemSlotBits | void ReadItemSlotBits(void * seri, void * slot) | blue-verified | `RestoreTeamInventory` call order |
| 0200684C | WriteHeldItemBits | void WriteHeldItemBits(void * seri, void * item) | blue-verified | 8, 7 bits, like `pmd-red` |
| 0200687C | ReadHeldItemBits | void ReadHeldItemBits(void * seri, void * item) | blue-verified | `RestoreTeamInventory` call order |
| 0200617C | WriteDungeonLocationBits | void WriteDungeonLocationBits(void * seri, void * loc) | blue-verified | 7, 7 bits, like `pmd-red` |
| 02003E1C | WritePoke1Bits | void WritePoke1Bits(void * seri, void * pokemon) | blue-verified | Same field order and widths as `pmd-red` |
| 0200388C | WritePoke1LevelBits | void WritePoke1LevelBits(void * seri, void * unkC) | blue-verified | Tail call to `WriteBits` with 7 bits |
| 020094D8 | WritePoke1MovesBits | void WritePoke1MovesBits(void * seri, void * moves) | blue-verified | 4 × `WritePoke1MoveBits` |
| 02009550 | WritePoke1MoveBits | void WritePoke1MoveBits(void * seri, void * move) | blue-verified | 4, 9, 7 bits, like `pmd-red` |
| 020093F8 | WritePoke2MovesBits | void WritePoke2MovesBits(void * seri, void * moves) | blue-verified | `SavePoke2s` call order |
| 0200B944 | WriteBellyBits | void WriteBellyBits(void * seri, void * belly) | blue-verified | 16, 16 bits, like `pmd-red` |
| 0200382C | WriteHiddenPowerBits | void WriteHiddenPowerBits(void * seri, void * hiddenPower) | blue-verified | 10, 5 bits, like `pmd-red` |
| 0200CEEC | sub_8095824 | void sub_8095824(void * a, void * b) | blue-verified | Same 13-field mail record as `pmd-red` |
| 0200D4DC | sub_809542C | void sub_809542C(void * seed) | blue-verified | Mail-record builder; body matches `pmd-red` `src/code_8094F88.c:270–283`. Called from `LoadAndRunQuickSaveDungeon_Async` (`0201E304`); seeds the Mersenne Twister from `Rand32Bit` |
| 0206C6E4 | HandleTestTrackerState | int HandleTestTrackerState(void) | blue-verified | Personality-test state machine; case 15 (`PERSONALITY_TEST_END`) matches `pmd-red` `src/personality_test1.c:96`. Reached through a dispatch table at `ov_0000::02159D00` |
| 02023B9C | OtherRandom16 | ushort OtherRandom16(void) | blue-verified | 16-bit generator: `state = state * 109 + 1021` in 16 bits, state at `020E03F4`; return loaded with `ldrh` (zero-extended). Matches `pmd-red`'s `OtherRandom16` in constants and initial value; Red's return signedness not compared |
| 02023B84 | OtherRandInt | int OtherRandInt(int n) | blue-verified | `(n * OtherRandom16()) >> 16`. Matches `pmd-red`'s `OtherRandInt` |
| 02023B6C | OtherRandRange | int OtherRandRange(int low, int high) | blue-verified | `low + OtherRandInt(high - low)`; no equal-bounds case. Matches `pmd-red`'s `OtherRandRange` |
| 020E03F4 | gOtherRngState | short | blue-verified | State of `OtherRandom16`. Static initial value `0x348C` (same as `pmd-red`'s `gOtherRngState`). The three accesses found by x-ref are all in `OtherRandom16`; aliasing not checked |
| 0201187C | GetDirectionTowardsPosition | uchar GetDirectionTowardsPosition(void * pos, void * targetPos) | red-hypothesis | Signs of the x and y deltas index a table; called by `DisplayActions_Async` (`ov_0001::0218766C`) |
| ov_0001::02188030 | RunMonsterAI | void RunMonsterAI(void * pokemon, uint unused) | red-hypothesis | Candidate. Called as `(x, 0)` and `(x, 1)` from `sub_8044574_Async`; writes `decoyAITracker` (`+0xDD`) and `aiNotNextToTarget` (`+0x71`). Decision order read in Blue (structure), see [`re/ai.md`](re/ai.md); the name is Red's. Matching this function, its `aiTarget` layout and the IQ offset is [#164](https://github.com/TorratDev/rust-mm/issues/164) |
| 0202E5A0 | DrawDialogueBoxString_Async | void DrawDialogueBoxString_Async(void) | red-hypothesis | Frame action #1 in `MainLoops_RunFrameActions`; a state machine, body not read. Order only |
| 020163B8 | CopyWindowBgBuffer | void CopyWindowBgBuffer(void * window, int win) | red-hypothesis | Frame action #2, called with `(0, 0)`; argument pattern matches Red's `CopyWindowBgBuffer(NULL, WIN0)` |
| 02012310 | nullsub_8 | void nullsub_8(uchar touchScreen) | red-hypothesis | Frame action #3; body is `return;` |
| 02012BCC | sub_8005180 | void sub_8005180(void) | red-hypothesis | Frame action #4; linked-list compaction. Order only |
| 02016320 | ToggleWindowBgBuffer | void ToggleWindowBgBuffer(void) | red-hypothesis | Frame action #6; toggles two flags, calls `FUN_02019220` and `FUN_02077098`. Order only |
| 02029BBC | UpdateSoundEffectCounters | void UpdateSoundEffectCounters(void) | red-hypothesis | Frame action #7; decrements three counters while they are above 0 |
| 02012B34 | CopySpritesToOam | void CopySpritesToOam(void) | red-hypothesis | Frame action #10; linked list to OAM |
| 02012998 | sub_8005304 | void sub_8005304(void) | red-hypothesis | Frame action #11; loop calling `FUN_01ff9fb0` and `FUN_01ff9fc4` |
| 0201A974 | DoScheduledMemCopies | void DoScheduledMemCopies(void) | red-hypothesis | Frame action #16; loops over 12-byte entries, calling `FUN_01ff9fc4(src, dst, size)` |
| 02013078 | ResetSprites | void ResetSprites(int arg) | red-hypothesis | Frame action #18, called with 0; clears a count, sets OAM base `06400000`, loops over sprite entries |
| 02018ACC | VBlankIrqHandler | void VBlankIrqHandler(void) | blue-verified | VBlank IRQ handler, registered in IRQ slot 0 by `FUN_02075C5C(1, ...)` (literal `0x02000D60`); calls `UpdateKeyState` each frame. [`re/io.md`](re/io.md#frame-path) |
| 02017B20 | UpdateKeyState | void UpdateKeyState(void) | blue-verified | Per-frame key state in two 16-byte structs at `0x020F2790` and `0x020F2780`: edges, D-pad repeat, B and R taps. Calls `ReadKeyInput`. [`re/io.md`](re/io.md#key-state) |
| 02019130 | ReadKeyInput | void ReadKeyInput(void) | blue-verified | Reads `REG_KEYINPUT` (`0x04000130`), masks with `0x2FFF`, adds the result into the sum at `0x020DF89C`. Arguments and return not checked. [`re/io.md`](re/io.md#frame-path) |
| 02017E60 | UpdateInputSnapshot | void UpdateInputSnapshot(void) | blue-verified | Copies the key accumulator to `0x020F27A0` and clears it; updates the touch struct at `0x020F27B0`. Frame action #9 (`LoadBufferedInputs` in `execution-model.md`). [`re/io.md`](re/io.md#snapshot-and-consumers) |
| 020271E0 | ResolveDungeonDirection | int ResolveDungeonDirection(void) | blue-verified | Dungeon D-pad to one of 8 directions (diagonals only with R), and entity pick within 10 px on touch. Arguments and return not checked. [`re/io.md`](re/io.md#snapshot-and-consumers) |
| 0207CA44 | CalibrateTouch | void CalibrateTouch(void) | blue-verified | Touch calibration: affine when the flag at `DAT_0207CB6C+0x30` is set, then clamp to 256 × 192. Parameter source not verified. Arguments not checked. [`re/io.md`](re/io.md#calibration) |
| 02019268 | FlushDisplayState | void FlushDisplayState(void) | blue-verified | Per-frame flush: DISPCNT layer enables, BG priorities and `BLDCNT`. Frame action #17. Flag meanings medium. [`re/io.md`](re/io.md#per-frame-flush) |
| 02074FC8 | InitDisplayRegisters | void InitDisplayRegisters(void) | blue-verified | Boot display setup: POWCNT1, DISPCNT clear, BG and affine registers on both engines, `MASTER_BRIGHT` to 0. Called from `0x02000BE0`. [`re/io.md`](re/io.md#boot-display-setup) |
| 0201A110 | SceneSwitchReset | void SceneSwitchReset(void) | blue-verified | Calls `ResetScreens` when `FUN_0207F458()` is nonzero. What that test checks is medium. [`re/io.md`](re/io.md#scene-switch-and-reset) |
| 02018B44 | ResetScreens | void ResetScreens(void) | blue-verified | Full reset: clears VRAM at `0x06800000` and three more blocks, calls `FUN_02074E90(2, 0, 0)` and `FUN_02074F04`; calls the PX decoder for the halt screen at `0x02018C20`. [`re/io.md`](re/io.md#scene-switch-and-reset) |
| 0201AFF0 | ResetSceneGraphics | void ResetSceneGraphics(void) | blue-verified | Scene load: clears BG and OBJ VRAM and palette, fills OAM A and OAM B (`0x07000400`) with `DAT_0201B214`, loads palette data. [`re/io.md`](re/io.md#oam-writers) |
| 0201ADA0 | ClearOamA | void ClearOamA(void) | blue-verified | Fills OAM A. Only caller is a computed jump from a thunk at `0x0206DC18`, not identified. [`re/io.md`](re/io.md#oam-writers) |
| 02012A6C | BlinkSpriteZero | void BlinkSpriteZero(void) | blue-verified | Writes OAM entry 0 each frame, toggling attribute 0 between `0xC0` and a value from `DAT_02012AE4` by bit 4 of a frame counter. Caller not identified. [`re/io.md`](re/io.md#oam-writers) |
| 02013388 | PxDecompress | void PxDecompress(void) | blue-verified | PX decoder for `AT4P` and `AT3P` headers, reached through the dispatcher slot `0x02013864`. Placeholder prototype: arguments not checked. [`re/io.md`](re/io.md#px-decoder) |
| 0201A6F0 | LoadArchiveTocs | void LoadArchiveTocs(void) | blue-verified | Reads each archive's 16-byte TOC records into one global table. Called from `0x02000CE8`. [`re/io.md`](re/io.md#archive-index-and-toc) |
| 0201A56C | FindArchiveEntry | void FindArchiveEntry(char * name, int archive) | blue-verified | Binary search of an archive's TOC range for a name. Return value not checked. [`re/io.md`](re/io.md#archive-index-and-toc) |
| 0201A0B8 | RelocateSir0 | void RelocateSir0(void) | blue-verified | SIR0 relocation: big-endian base-128 offsets, each added to the base. The `SIRO` flip is in `FUN_0201A134` and `FUN_0201A188`. [`re/io.md`](re/io.md#sir0-relocation) |
| 02018C98 | LoadHaltScreen | void LoadHaltScreen(int index) | blue-verified | Opens the loose `nitro_haltscreen*.sbin` file for the language index, then PX-decodes it. Reached through `FUN_02018C54`. [`re/io.md`](re/io.md#halt-screens) |
| 020160A4 | LoadKanjiFonts | void LoadKanjiFonts(void) | blue-verified | Boot: copies `kanji_a` (0xA000 bytes) and `kanji_b` (0x6400 bytes) from archive 2. Called from `0x02000CEC`. [`re/io.md`](re/io.md#consumers-at-boot-and-in-gameloop_async) |
| 0200B888 | LoadWazaParameters | void LoadWazaParameters(void) | blue-verified | Loads `wazapar{a,g,f,i,s}` (0x18000 bytes) from archive 2; suffix from `GetLanguage()`. Name as in `execution-model.md`. [`re/io.md`](re/io.md#consumers-at-boot-and-in-gameloop_async) |
| 02008614 | LoadItemParameters | void LoadItemParameters(void) | blue-verified | Loads `itempar{a,g,f,i,s}` (0xC000 bytes) from archive 2; suffix from `GetLanguage()`. Name as in `execution-model.md`. [`re/io.md`](re/io.md#consumers-at-boot-and-in-gameloop_async) |
| ov_0001::0218B140 | CannotSleep | int CannotSleep(void * entity, int unk2, int checkSafeguard) | blue-verified | Sleep immunity gate: Safeguard (only when the third argument is nonzero), IQ skill `0x11`, Insomnia `0x36`, Vital Spirit `0x49`, held item `0x15`. Returns 1 when blocked. Name from Red; argument list not checked, and the second argument is unknown. |
| ov_0001::0218B2D0 | SleepStatusTarget | int SleepStatusTarget(void * entity, int turns, int flag) | blue-verified | Sleep with message: calls the gate `CannotSleep` with flag 1, sets the message from `0x99`, then calls `SetSleepStatus`. The yawn expiry calls it as `(entity, roll + 1, 1)`. Name from Red; argument list not checked. |
| ov_0001::0218B420 | SetSleepStatus | int SetSleepStatus(void * entity, int turns) | tentative | Core sleep write: returns 2 if already nightmare, 1 if already asleep, otherwise sets `0x99 = 1` and the turns to `0x9A`. Early Bird halves the turns unless they are `0x7F`. Red counterpart `sub_8075BF4` (red-hypothesis). Argument list not checked. |
| ov_0001::0218B01C | NightmareStatusTarget | int NightmareStatusTarget(...) | blue-verified | Sets `0x99 = 3`. Blocked if already 3; status 2 gives the sleepless message. Name from Red. Argument list not checked. |
| ov_0001::0218ADA8 | YawnedStatusTarget | int YawnedStatusTarget(...) | blue-verified | Sets `0x99 = 4`, only from status 0. The write is medium confidence; another finding tags it red-hypothesis (see [`re/status.md`](re/status.md#open-questions)). Name from Red. Argument list not checked. |
| ov_0001::0218AEF0 | NappingStatusTarget | int NappingStatusTarget(...) | blue-verified | Sets `0x99 = 5` from status 0 or 4. Early Bird halves the turns. Name from Red. Argument list not checked. |
| ov_0001::0218FE64 | EndSleepClassStatus | void EndSleepClassStatus(void * entity, void * user, int arg3, int arg4) | blue-verified | Expiry switch on `0x99`: wake (1, 2); nightmare damage 8 when the fourth argument is nonzero (3); yawn to sleep for roll + 1 turns (4); napping heal 999 and cure-all (5). Switch is high confidence, targets medium. Name from Red; argument list not checked. |
| ov_0001::0218FD80 | EndBurnClassStatus | void EndBurnClassStatus(...) | blue-verified | Expiry switch on `0x9B`: burn message (1), poison message (2, 3), paralysis message (4). Clears `0x9B` and refreshes the display. Name from Red. Argument list not checked. |
| ov_0001::0218FC18 | EndFrozenClassStatus | void EndFrozenClassStatus(...) | blue-verified | Switch on `0xA0`: 1 frozen (calls `0213EA14`); 2 and 5 share a route with no release; 3 and 4 share a route that releases links through `02189910`; 6 and 7 have their own messages. Routes blue-verified, names red-hypothesis. Name from Red. Argument list not checked. |
| ov_0001::0218FA80 | EndCringeClassStatus | void EndCringeClassStatus(...) | blue-verified | Seven messages (values 1 to 7), clears `0xAC`, refreshes the display, calls `CalcSpeedStage`. The value-to-name mapping is red-hypothesis. Name from Red. Argument list not checked. |
| ov_0001::0218F814 | EndReflectClassStatus | void EndReflectClassStatus(...) | blue-verified | Switch on `+0xB1`, cases 0 to 14, each with a message; clears `+0xB1` and refreshes the display. Case 4 shares a block with case 10. The value mapping is red-hypothesis. Name from Red. Argument list not checked. |
| ov_0001::0218F6C8 | EndCurseClassStatus | void EndCurseClassStatus(void * entity, void * user, int curseClass) | red-hypothesis | The third argument selects the curse class (1 cursed, 2 decoy, 3 snatch). Case 2 clears the decoy flag at dungeon `+0x335C`; case 3 clears `+0x158C8` and `+0x158D4`. Argument list from Red, not checked. |
| ov_0001::0218A710 | BurnedStatusTarget | void BurnedStatusTarget(...) | blue-verified | Burn setter: Safeguard, Water Veil `0x42`, fire type, water terrain, already burned. Then `0x9B = 1`, `0x9C` from the (127, 127) roll plus 1, and `0x9D` and `0x9E` zeroed. Name from Red. Argument list not checked. |
| ov_0001::0218A410 | PoisonedStatusTarget | void PoisonedStatusTarget(...) | blue-verified | Poison setter: Immunity `0x45` check and the already-badly-poisoned check, then `0x9B = 2`. Name from Red. Argument list not checked. |
| ov_0001::0218A140 | BadlyPoisonedStatusTarget | void BadlyPoisonedStatusTarget(...) | blue-verified | Bad-poison setter: Pecha Scarf `0x14`, Immunity `0x45`, poison or steel type checks. Then `0x9B = 3`, `0x9C` from the (127, 127) roll plus 1, and `0x9D` and `0x9E` zeroed. The turn range is red-hypothesis (medium). Name from Red. Argument list not checked. |
| ov_0001::0218A9F8 | InfatuateStatusTarget | void InfatuateStatusTarget(...) | blue-verified | Sets `0xAC = 7` (infatuated). Name from Red. Write sites overlap #166. Argument list not checked. |
| ov_0001::0218E27C | ConfuseStatusTarget | void ConfuseStatusTarget(...) | blue-verified | Sets `0xAC = 2` (confused). Name from Red. Write sites overlap #166. Argument list not checked. |
| ov_0001::021885A0 | TryInflictParalysisStatus | void TryInflictParalysisStatus(...) | blue-verified | Writes `0x9B = 4` and the turns. Called from `0215BAF8` when bit 8 of `+0x144` is set; the other bits are not verified. Immunity check not listed. Name from Red. Argument list not checked. |
| ov_0001::02189F68 | FrozenStatusTarget | void FrozenStatusTarget(...) | blue-verified | Frozen setter: Magma Armor `0x41` check, and not already 1 or in 3 to 4. Sets `0xA0 = 1`. Medium confidence. Name from Red. Argument list not checked. |
| ov_0001::02180870 | SafeguardIsActive | int SafeguardIsActive(void * entity, int unk2, int showMessage) | blue-verified | Returns 1 when `+0xB1 == 2`. With the third argument set, shows the Safeguard message. 29 callers. Does not check Mist. Name from Red; argument list not checked, and the second argument is unknown. |
| ov_0001::02175850 | AbilityStatusCure | void AbilityStatusCure(void * entity) | tentative | Ability cures: Limber (paralysis), Water Veil (burn), Immunity (poison, bad poison), Own Tempo (cringe values 2 and 7). Insomnia and Vital Spirit call the sleep expiry when `FUN_021818D0` is nonzero or the status is yawning. Name is ours; argument list not checked. |
| ov_0001::0219CF28 | CalculateStatusTurns | int CalculateStatusTurns(void * entity, void * range, int param) | blue-verified | Rolls `DungeonRandRange` on a (min, max) pair. For monsters, checks `0x12` (halves) and `0x14` (caps at 5 when above 4) through `FUN_021805B0`; the meaning of those values is not checked against Red. Name from Red; argument list not checked. |
| ov_0001::0219BF1C | DecrementStatusTurns | void DecrementStatusTurns(uchar * turns) | tentative | Decrements a turns byte unless it is `0x7F`, which never expires. Investigator B also reports `0` as left alone; not resolved. Red's `sub_80838EC` is the same routine (red-hypothesis for the name). Argument list not checked. |
| ov_0001::021861E8 | DoEndOfTurnEffects_Async | void DoEndOfTurnEffects_Async(void * entity) | blue-verified | Per-monster end of turn: burn, poison and bad-poison damage (countdown resets 20, 10, 2), the yawning branch (calls `0213F6B0`), and the Shed Skin roll `DungeonRandInt(100) < 50`. Name from Red; argument list not checked. |
| ov_0001::02190100 | CureNegativeStatuses | void CureNegativeStatuses(void * entity) | tentative | Clears the negative statuses. Called by Shed Skin and the napping end. Runs `FUN_02181904` first and calls the class enders. Inferred from its body; argument list not checked. |
| ov_0001::0218DF38 | HealTargetHP | void HealTargetHP(void * entity, int amount) | red-hypothesis | Heal; the napping end calls it with 999. Argument order not checked against Blue. Name from Red. |
| ov_0001::02189910 | ReleaseLinkedEntities | void ReleaseLinkedEntities(int linkId) | tentative | Loops over the 20 entity slots at `gDungeon+0x11364`. For entities whose `+0x90` equals `linkId`: clears `+0xA0` when it is 3 or 4, clears `+0x90`, refreshes the sprite. Red counterpart `sub_8076CB4` is red-hypothesis; the wrap link offset differs and is not reconciled. Argument list not checked. |
| ov_0001::0219EFCC | ShouldRunMonsterAI | int ShouldRunMonsterAI(...) | blue-verified | Switch on `monsterBehavior` (`+0x98`): returns 0 for behaviours 2, 4, 10, 11, 12 and above 36, 1 otherwise. Behaviour 0 skips the check. Agrees with Red's `ShouldRunMonsterAI` (`src/dungeon_cutscene.c:1093`) for IDs 0-36. Argument list not checked. |
| ov_0001::0218163C | CheckVariousConditions | int CheckVariousConditions(...) | red-hypothesis | Called in RunMonsterAI's held-item step and by `AIDecideUseItem`. If it returns true, the held item flag is cleared and the 'couldn't be used' message is posted. Body not read. Argument list not checked. |
| ov_0001::02180048 | HasStatusThatPreventsActing | int HasStatusThatPreventsActing(...) | blue-verified | Branch-for-branch match with Red's `HasStatusThatPreventsActing` (`src/status_checks.c:18-100`). Sets the action to pass turn and returns 1 when the monster cannot act. Four RNG sites. Argument list not checked. |
| ov_0001::021918D4 | AIMovement | void AIMovement(...) | blue-verified | Movement dispatcher: clears the three movement flags on entry, tactic 7 and 9 gates, belly gate, random walk or run-away test, then avoid or item and target search, then DecideMovement. Name from Red's `AIMovement`. Argument list not checked. |
| ov_0001::02190FEC | DecideMovement | void DecideMovement(...) | blue-verified | Picks the step: the chosen direction, then +-45, +-90 and +-135 degrees, each tested with `CanAIMonsterMoveInDirection`. Turn-around and tactic 3 adjustments. Blocked-in-front and trailing branches set `+0x71`, `+0x116` and `+0x119`. Argument list not checked. |
| ov_0001::02191394 | AIFindTarget | void AIFindTarget(...) | blue-verified | Nearest valid candidate from the team or wild list; sets the chase state and target; tactics 8 and 1; leader follow through `02183FF0`. Name from Red's AIFindTarget. Return type and argument list not checked. |
| ov_0001::02183FF0 | GetLeaderIfVisible | void * GetLeaderIfVisible(...) | blue-verified | Returns 0 when the entity is not a team member (`+6` not 0), when the leader's `+0xB3` is 2, or when the leader is not visible; otherwise the leader. Return type and argument list not checked. |
| ov_0001::02180DF8 | CanAIMonsterMoveInDirection | bool CanAIMonsterMoveInDirection(...) | blue-verified | Tests in order: walls, monster house (terrain `0x40` with `gDungeon+0x661`), traps (IQ 14), lava (IQ 20), all-terrain (IQ 12), super mobile (IQ 13). Red's order is the same. Argument list not checked. |
| ov_0001::02190854 | AIWander | void AIWander(...) | tentative | Wander fallback: random direction from `DungeonRandInt(8)`, each direction tested with `02180DF8`, then a random target and state 6. The name is ours; the role is read. Argument list not checked. |
| ov_0001::02180BA0 | AIShouldRunAway | bool AIShouldRunAway(...) | tentative | Predicate: true when the ability `'+'` is active and HP is below max/2, or tactic 10 (GET_AWAY) is set, or tactic 6 (AVOID_TROUBLE) is set and HP is at or below max/2. Wrapper `02180B68` not named. Argument list not checked. |
| ov_0001::02180C6C | AIAvoidTheFirstHit | void AIAvoidTheFirstHit(...) | tentative | Tactic 3 handler called from `DecideMovement`: keeps a distance of 2 and turns 4. The name is ours. Argument list not checked. |
| ov_0001::021805B0 | HasIQSkill | bool HasIQSkill(...) | tentative | Wrapper that tail-calls `IsIQSkillSet` with the entity's IQ field (`+0x87`, offset owned by [#164](https://github.com/TorratDev/rust-mm/issues/164)) and a one-bit mask. Called with 0x15 (Dedicated Traveler) and 0x16 (Item Master). The name is ours. Argument list not checked. |
| 02004250 | IsIQSkillSet | bool IsIQSkillSet(...) | blue-verified | Tests one bit across the three IQ bytes. Reached through `HasIQSkill`. Name from Red's `IsIQSkillSet`. Argument list not checked. |
| 0200234C | GetCanMoveFlag | bool GetCanMoveFlag(...) | red-hypothesis | Returns the can-move flag. RunMonsterAI calls `AIMovement` only when it is set, in both IQ branches. Argument list not checked. |
| 0200B428 | GetMoveAIWeight | uchar GetMoveAIWeight(...) | blue-verified | Returns the move-table byte at `+0x0D` (move-table stride 0x24). Used by the weight step of `ChooseAIMove`. Argument list not checked. |
| 021473A8 | GetAIUseItemProbability | int GetAIUseItemProbability(...) | red-hypothesis | Switch on the item ID's low byte that returns percentages; flag bit 0 gates some cases. Same shape as Red's `GetAIUseItemProbability`; not compared case by case. Argument list not checked. |
| ov_0001::02185568 | GetPossibleAIArcItemTargets | int GetPossibleAIArcItemTargets(...) | blue-verified | Walks the 20 monsters, skips self, keeps visible targets within Chebyshev distance under 11, and rolls per target unless the roll is ignored. Matches Red's `GetPossibleAIArcItemTargets` (`src/dungeon_ai_items.c:256`) approximately; Red's `GetPossibleAIThrownItemDirections` (`:297`) maps here too. Argument list not checked. |
| ov_0001::0218579C | AIDecideUseItem | void AIDecideUseItem(...) | blue-verified | Item AI: held-item branch on `+0xD9`, self-use roll per eligible item, Item Master (IQ 22) gate, throw and eat actions. Name from Red's `AIDecideUseItem` (`src/dungeon_ai_items.c:46`). Argument list not checked. |
| ov_0001::02192274 | IsAITargetEligible | bool IsAITargetEligible(...) | blue-verified | Reads the `0xF00` nibble of the move's AI field. `0x100` is the random-chance case: roll `DungeonRandInt(100)` against accuracy index 2 at move `+0x10`. `0x200` to `0x600` are HP, status and type checks. Red's `IsAITargetEligible` is `src/dungeon_ai_attack.c:608`. Argument list not checked. |
| ov_0001::02192D24 | ChooseAIMove | void ChooseAIMove(...) | blue-verified | Move choice: confusion roll (70), bide and charge block, weights from `GetMoveAIWeight` and the regular-attack table, Weak-Type Picker filter, weighted pick. Name from Red's `ChooseAIMove` (`src/dungeon_ai_attack.c:53`). Argument list not checked. |
