// Names, types and checks Blue's random number generators (issue #47).
//
// Three generators, named after their pmd-red counterparts (src/random.c,
// src/dungeon_random.c, src/random_mersenne_twister.c), which match Blue by constant and
// behaviour; see docs/re/rng.md:
//   global LCG (ARM9)    sPRNGState, SeedRng, Rand16Bit, Rand32Bit, RandInt, RandRange, ...
//   dungeon LCG (ov_0001) gDungeonRngState, DungeonRand16Bit, DungeonRandInt, ...
//   Mersenne Twister     sStateArray, MersenneTwister_InitializeState, Random32MersenneTwister
// plus the two callers that seed the dungeon RNG.
//
// Before changing anything it checks literal pools and tables for the expected constants
// (LCG multiplier 0x5D588B65, SeedRng constant 0xD305, MT 0x6C078965 / 0x9908B0DF). If any
// check fails it stops. Then it applies every name, type and signature (default calling
// convention, dynamic storage, not noreturn), verifies them and prints OK/FAIL lines.
// Safe to run again. Output lines start with "RESCUE-RNG:".
//
//@category NDS
//@author rescue project
//@runtime Java

import ghidra.app.cmd.function.ApplyFunctionSignatureCmd;
import ghidra.app.script.GhidraScript;
import ghidra.app.util.parser.FunctionSignatureParser;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.data.ArrayDataType;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.FunctionDefinitionDataType;
import ghidra.program.model.data.IntegerDataType;
import ghidra.program.model.data.UnsignedIntegerDataType;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;

public class RescueRngLabels extends GhidraScript {

	private static final String ARM9 = "ram";
	private static final String OV1 = "ov_0001";

	/** Words that must hold the given values before anything is changed. */
	private static final Object[][] POOLS = {
		{ ARM9, 0x020119d4L, 0x5D588B65L },   // Rand16Bit multiplier
		{ ARM9, 0x02011a18L, 0x0000D305L },   // SeedRng constant (54021)
		{ ARM9, 0x020119d0L, 0x020DF704L },   // Rand16Bit -> sPRNGState
		{ ARM9, 0x0200ccf4L, 0x6C078965L },   // MT init multiplier
		{ ARM9, 0x0200ccf0L, 0x020DED24L },   // MT init -> sStateArray
		{ ARM9, 0x020ded20L, 0x9908B0DFL },   // sTwistMatrix[1]
		{ OV1, 0x0219d0a8L, 0x5D588B65L },    // DungeonRand16Bit multiplier
		{ OV1, 0x0219d0a4L, 0x021B3F58L },    // DungeonRand16Bit -> gDungeonRngState
		{ OV1, 0x0219d0c0L, 0x021B3F58L },    // InitDungeonRNG -> gDungeonRngState
		{ OV1, 0x0219d11cL, 0x5D588B65L },    // GenerateDungeonRNGSeed multiplier
		{ OV1, 0x0219d118L, 0x021B3F5CL },    // GenerateDungeonRNGSeed -> preseed
		{ OV1, 0x0219d13cL, 0x021B3F5CL },    // SetDungeonRNGPreseed23Bit -> preseed
	};

	/** space, address, name, C prototype (the name in the prototype is replaced). */
	private static final Object[][] FUNCTIONS = {
		{ ARM9, 0x020119d8L, "SeedRng", "void f(byte *seed)" },
		{ ARM9, 0x020119a4L, "Rand16Bit", "short f(void)" },
		{ ARM9, 0x0201198cL, "Rand32Bit", "int f(void)" },
		{ ARM9, 0x02011968L, "RandInt", "int f(int maxExclusive)" },
		{ ARM9, 0x02011924L, "RandRange", "int f(int minInclusive, int maxExclusive)" },
		{ ARM9, 0x02011914L, "GetRNGState", "int f(void)" },
		{ ARM9, 0x020118ecL, "SetRNGSeed", "void f(int state)" },
		{ ARM9, 0x0200ccb0L, "MersenneTwister_InitializeState", "void f(uint seed)" },
		{ ARM9, 0x0200cb40L, "Random32MersenneTwister", "int f(void)" },
		{ ARM9, 0x0206dc20L, "RunDungeon_Async", null },
		{ ARM9, 0x0201db70L, "LoadAndRunDungeon_Async", null },
		{ OV1, 0x0219d07cL, "DungeonRand16Bit", "int f(void)" },
		{ OV1, 0x0219d058L, "DungeonRandInt", "int f(int cap)" },
		{ OV1, 0x0219d014L, "DungeonRandRange", "int f(int low, int high)" },
		{ OV1, 0x0219cfccL, "DungeonRandOutcome", "bool f(int percentChance)" },
		{ OV1, 0x0219cff0L, "DungeonRandOutcome_2", "bool f(int percentChance)" },
		{ OV1, 0x0219d0b0L, "InitDungeonRNG", "void f(uint value)" },
		{ OV1, 0x0219d0c4L, "GenerateDungeonRNGSeed", "uint f(void)" },
		{ OV1, 0x0219d124L, "SetDungeonRNGPreseed23Bit", "void f(uint preseed23)" },
	};

	private static final String RUN_DUNGEON_NOTE =
		"Matches pmd-red RunDungeon_Async by its RNG seeding.";

	private AddressSpace arm9;
	private AddressSpace ov1;
	private int failures;

	@Override
	protected void run() throws Exception {
		arm9 = currentProgram.getAddressFactory().getDefaultAddressSpace();
		ov1 = currentProgram.getAddressFactory().getAddressSpace(OV1);
		if (ov1 == null) {
			say("no address space " + OV1 + "; was the program built with RescueNdsImport.java?");
			return;
		}
		boolean poolsOk = true;
		for (Object[] p : POOLS) {
			poolsOk &= expectWord(at((String) p[0], (Long) p[1]), (Long) p[2]);
		}
		if (!poolsOk) {
			say("literal pool check failed; nothing changed");
			return;
		}

		say("--- apply");
		DataType s32 = IntegerDataType.dataType;
		DataType u32 = UnsignedIntegerDataType.dataType;
		global(at(ARM9, 0x020df704L), "sPRNGState", s32);
		global(at(ARM9, 0x020ded18L), "sStateIndex", s32);
		global(at(ARM9, 0x020ded1cL), "sTwistMatrix", new ArrayDataType(u32, 2, 4));
		global(at(ARM9, 0x020ded24L), "sStateArray", new ArrayDataType(u32, 624, 4));
		global(at(OV1, 0x021b3f58L), "gDungeonRngState", u32);
		global(at(OV1, 0x021b3f5cL), "gDungeonRngPreseedState", u32);
		for (Object[] f : FUNCTIONS) {
			apply(at((String) f[0], (Long) f[1]), (String) f[2], (String) f[3]);
		}
		setPlateComment(at(ARM9, 0x0206dc20L), RUN_DUNGEON_NOTE + " Seeds: dungeon+0x668 missionKind " +
			"(OUTONRESCUE: preseed from setup+0x14, else Rand32Bit() & 0xFFFFFF), " +
			"+0x66c preseed, +0x670 floor seed = GenerateDungeonRNGSeed().");
		setPlateComment(at(ARM9, 0x0201db70L),
			"Matches pmd-red LoadAndRunDungeon_Async: swaps overlay 0 for the language text overlay and overlay 1, runs RunDungeon_Async, swaps back.");

		say("--- check");
		checkGlobal(at(ARM9, 0x020df704L), "sPRNGState");
		checkGlobal(at(ARM9, 0x020ded24L), "sStateArray");
		checkGlobal(at(OV1, 0x021b3f58L), "gDungeonRngState");
		checkGlobal(at(OV1, 0x021b3f5cL), "gDungeonRngPreseedState");
		for (Object[] f : FUNCTIONS) {
			checkFunction(at((String) f[0], (Long) f[1]), (String) f[2], (String) f[3]);
		}
		say(failures == 0 ? "all checks passed" : failures + " check(s) failed");
	}

	// --- apply

	private void global(Address a, String name, DataType type) throws Exception {
		Data d = getDataAt(a);
		if (d == null || !d.getDataType().isEquivalent(type)) {
			clearListing(a, a.add(type.getLength() - 1));
			createData(a, type);
		}
		Symbol s = getSymbolAt(a);
		if (s == null || !s.getName().equals(name)) {
			createLabel(a, name, true, SourceType.USER_DEFINED);
		}
	}

	private void apply(Address a, String name, String prototype) throws Exception {
		Function f = getFunctionAt(a);
		if (f == null) {
			if (getInstructionAt(a) == null) {
				disassemble(a);
			}
			f = createFunction(a, name);
			if (f == null) {
				say("createFunction failed at " + a + "; skipped " + name);
				return;
			}
		}
		f.setName(name, SourceType.USER_DEFINED);
		f.setNoReturn(false);
		if (prototype == null) {
			return;
		}
		FunctionDefinitionDataType def = parse(name, prototype);
		f.setCustomVariableStorage(false);
		ApplyFunctionSignatureCmd cmd =
			new ApplyFunctionSignatureCmd(a, def, SourceType.USER_DEFINED);
		if (!cmd.applyTo(currentProgram, monitor)) {
			say("signature failed for " + name + ": " + cmd.getStatusMsg());
			return;
		}
		f.setCallingConvention(defaultConvention());
	}

	private FunctionDefinitionDataType parse(String name, String prototype) throws Exception {
		FunctionSignatureParser parser =
			new FunctionSignatureParser(currentProgram.getDataTypeManager(), null);
		return parser.parse(null, prototype.replaceFirst("\\bf\\(", name + "("));
	}

	// --- check

	private void checkGlobal(Address a, String name) {
		Symbol s = getSymbolAt(a);
		check(s != null && s.getName().equals(name), a + " is labelled " + name);
	}

	private void checkFunction(Address a, String name, String prototype) throws Exception {
		Function f = getFunctionAt(a);
		if (!check(f != null, "function at " + a)) {
			return;
		}
		check(f.getName().equals(name), a + " is named " + name);
		check(!f.hasNoReturn(), name + " returns (not noreturn)");
		if (prototype == null) {
			return;
		}
		FunctionDefinitionDataType want = parse(name, prototype);
		check(f.getReturnType().isEquivalent(want.getReturnType()) &&
			f.getParameterCount() == want.getArguments().length,
			name + " is " + want.getPrototypeString());
		check(defaultConvention().equals(f.getCallingConventionName()),
			name + " uses calling convention " + defaultConvention());
		check(!f.hasCustomVariableStorage(), name + " has no custom storage");
	}

	private boolean check(boolean ok, String what) {
		say((ok ? "OK   " : "FAIL ") + what);
		if (!ok) {
			failures++;
		}
		return ok;
	}

	// --- helpers

	private String defaultConvention() {
		return currentProgram.getCompilerSpec().getDefaultCallingConvention().getName();
	}

	private boolean expectWord(Address a, long expected) throws Exception {
		long got = getInt(a) & 0xFFFFFFFFL;
		if (got != expected) {
			say(String.format("expected %08X at %s, found %08X", expected, a, got));
			return false;
		}
		return true;
	}

	private Address at(String space, long offset) {
		return (ARM9.equals(space) ? arm9 : ov1).getAddress(offset);
	}

	private void say(String s) {
		println("RESCUE-RNG: " + s);
	}
}
