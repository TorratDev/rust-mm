# Symbols

Names for Blue's functions and globals, one row each. This file is the machine-readable side of the glossary (#43): [`tools/ghidra/RescueApplySymbols.java`](../tools/ghidra/RescueApplySymbols.java) reads the table below and applies every row to a Ghidra project built as in [`ghidra-setup.md`](ghidra-setup.md), then checks it.

Dump: `BlueRescueTeamEur`. Rules for each column:

- **Address:** RAM address in hex. Prefix `ov_NNNN::` for code or data in an overlay; no prefix means the ARM9 binary.
- **Type:** a C prototype for a function (its name must match the Name column), a data type for a global (`int`, `uint[5]`, ...), or `-` for a plain label.
- **Tag:** `blue-verified` (role read in Blue's code), `red-hypothesis` (role taken from `pmd-red`, not yet checked in Blue), `sdk-hypothesis` (NitroSDK function recognised by its behaviour), `tentative` (our own name, role only partly known).
- **Evidence:** why the name fits. Names that come from `pmd-red` keep its spelling.

Topic pages: [`re/rng.md`](re/rng.md), [`re/execution-model.md`](re/execution-model.md), [`re/combat.md`](re/combat.md), [`re/turns.md`](re/turns.md), [`re/structs.md`](re/structs.md), [`re/dungeon-generation.md`](re/dungeon-generation.md).

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
| ov_0001::0217BD94 | CalcDamage | void CalcDamage(void * attacker, void * target, uchar moveType, int movePower, int critChance, void * dmgStruct, int arg8, ushort moveId, bool arg_10) | blue-verified | Calls `sub_806E100`; same constants as `pmd-red` (`999`, `0xAAAA`, `0xCCC`, `0xE000`, `0x4000`) and the same three `DungeonRandInt` calls in order |
| ov_0001::0217CE88 | sub_806E100 | bool sub_806E100(void * param_1, void * pokemon, void * target, uchar type, void * dmgStruct) | blue-verified | Reads `gTypeEffectivenessChart`; type is the 4th of 5 arguments, like `pmd-red` |
| ov_0001::0217CB3C | WeightWeakTypePicker | int WeightWeakTypePicker(void * user, void * target, uchar moveType) | blue-verified | Reads `gTypeEffectivenessChart`; 3 arguments, like `pmd-red` |
| ov_0001::0217BB94 | sub_806F370_Async | void sub_806F370_Async(void * pokemon, void * target, int dmg, int giveExp, bool * tookNoDamage, uchar moveType, short dungeonExitReason, int residualDmgType, int arg_10, int arg_14) | blue-verified | Same 10 arguments as `pmd-red`; Wonder Guard check through `gEffectivenessChart`, then `HandleDealingDamage_Async` |
| ov_0001::0217BA94 | sub_806F4A4 | uchar sub_806F4A4(void * pokemon, uchar type) | blue-verified | Tests the target's two types for `EFFECTIVENESS_IMMUNE` in `gTypeEffectivenessChart`, like `pmd-red` |
| ov_0001::0217E890 | HandleDealingDamage_Async | void HandleDealingDamage_Async(void * attacker, void * target, void * dmgStruct, bool isFalseSwipe, bool giveExp, short dungeonExitReason, bool arg8, int argC) | blue-verified | Called by `sub_806F370_Async` with 8 arguments, like `pmd-red` |
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
| ov_0001::02185C70 | TickStatusAndHealthRegen | void TickStatusAndHealthRegen(void * entity) | blue-verified | HP regeneration (clamped 30–500) then status countdowns, like `pmd-red` |
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
