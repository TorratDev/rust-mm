// Prints the callers of named functions, walking up the call graph to a given depth.
// Used to find the RNG wrappers and the code that seeds and drives them (issue #47).
//
// A name matches a function called NAME or ending in "__NAME" (e.g. FUN_ov_0001__NAME).
// Calls into an overlay from outside it do not resolve to the overlay (see
// docs/ghidra-setup.md), so references are collected at the function's offset in every
// memory space, and marked "via SPACE" when they land outside the function's own space.
// For the named functions it also scans initialised memory for aligned 4-byte pointers
// to the entry point (function tables, literal pools Ghidra did not mark).
//
// GUI: run from the Script Manager; it asks for comma-separated function names and a
// depth. Headless: pass names then depth, e.g.
//   -postScript RescueCallers.java DungeonRand_Next,Rand32 2
// Output lines start with "RESCUE-CALLERS:" and are also appended to
// ~/rescue-re/ghidra-script.log. Only names and addresses are printed.
//
//@category NDS
//@author rescue project
//@runtime Java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolType;

public class RescueCallers extends GhidraScript {

	private static final String DEFAULT_NAMES =
		"DungeonRand_Next,DungeonRand_SetSeed,Rand32,Rand_SetSeed";

	@Override
	protected void run() throws Exception {
		String[] args = getScriptArgs();
		String names;
		int depth;
		if (args.length > 0) {
			// analyzeHeadless.bat splits "a,b" into separate arguments; a trailing number
			// is the depth.
			int n = args.length;
			depth = 2;
			if (n > 1 && args[n - 1].matches("\\d+")) {
				depth = Integer.parseInt(args[--n]);
			}
			names = String.join(",", java.util.Arrays.copyOf(args, n));
		}
		else {
			names = askString("Callers of", "comma-separated function names", DEFAULT_NAMES);
			depth = askInt("Depth", "levels of callers to walk up (1 = direct callers)");
		}
		for (String raw : names.split(",")) {
			String name = raw.trim();
			if (name.isEmpty()) {
				continue;
			}
			List<Function> matches = functionsNamed(name);
			if (matches.isEmpty()) {
				say("no function named " + name + " or *__" + name);
			}
			for (Function f : matches) {
				say(f.getName() + " @ " + f.getEntryPoint() + "  " + f.getSignature());
				walk(f, 1, depth, new HashSet<>());
				pointers(f);
			}
		}
	}

	private void walk(Function f, int level, int depth, Set<Function> seen) {
		if (level > depth || !seen.add(f)) {
			return;
		}
		String indent = "  ".repeat(level);
		int unknown = 0;
		Set<Function> callers = new HashSet<>();
		AddressSpace own = f.getEntryPoint().getAddressSpace();
		for (Address target : aliases(f.getEntryPoint())) {
			String via = target.getAddressSpace().equals(own) ? ""
					: "  via " + target.getAddressSpace().getName();
			for (Reference r : getReferencesTo(target)) {
				Function c = getFunctionContaining(r.getFromAddress());
				if (c == null) {
					unknown++;
					say(indent + "<no function> " + r.getFromAddress() + " " +
						r.getReferenceType() + via);
					continue;
				}
				if (callers.add(c)) {
					say(indent + c.getName() + " @ " + c.getEntryPoint() + "  (" +
						r.getReferenceType() + " from " + r.getFromAddress() + ")" + via);
					walk(c, level + 1, depth, seen);
				}
			}
		}
		if (callers.isEmpty() && unknown == 0) {
			say(indent + "(no references)");
		}
	}

	private List<Function> functionsNamed(String name) {
		List<Function> out = new ArrayList<>();
		for (Symbol s : currentProgram.getSymbolTable().getAllSymbols(false)) {
			if (s.getSymbolType() != SymbolType.FUNCTION) {
				continue;
			}
			String n = s.getName();
			if (n.equals(name) || n.endsWith("__" + name)) {
				Function f = getFunctionAt(s.getAddress());
				if (f != null) {
					out.add(f);
				}
			}
		}
		return out;
	}

	/** The entry offset in every memory space: the real address first, then the aliases. */
	private Set<Address> aliases(Address entry) {
		Set<Address> out = new LinkedHashSet<>();
		out.add(entry);
		for (AddressSpace sp : currentProgram.getAddressFactory().getAllAddressSpaces()) {
			if (!sp.isMemorySpace()) {
				continue;
			}
			try {
				out.add(sp.getAddress(entry.getOffset()));
			}
			catch (RuntimeException e) {
				// offset outside this space
			}
		}
		return out;
	}

	private void pointers(Function f) throws Exception {
		long off = f.getEntryPoint().getOffset();
		byte[] le = { (byte) off, (byte) (off >> 8), (byte) (off >> 16), (byte) (off >> 24) };
		Memory mem = currentProgram.getMemory();
		int hits = 0;
		for (MemoryBlock b : mem.getBlocks()) {
			if (!b.isInitialized()) {
				continue;
			}
			Address p = b.getStart();
			while (true) {
				Address hit = mem.findBytes(p, b.getEnd(), le, null, true, monitor);
				if (hit == null) {
					break;
				}
				if (hit.getOffset() % 4 == 0) {
					hits++;
					Function c = getFunctionContaining(hit);
					Symbol l = getSymbolAt(hit);
					say("  pointer at " + hit + (l == null ? "" : " " + l.getName()) +
						(c == null ? "" : " in " + c.getName()));
				}
				if (hit.compareTo(b.getEnd()) >= 0) {
					break;
				}
				p = hit.add(1);
			}
		}
		if (hits == 0) {
			say("  (no pointers to " + f.getName() + " in memory)");
		}
	}

	private void say(String s) {
		String line = "RESCUE-CALLERS: " + s;
		println(line);
		Path log = Path.of(System.getProperty("user.home"), "rescue-re", "ghidra-script.log");
		try {
			Files.createDirectories(log.getParent());
			Files.writeString(log, line + System.lineSeparator(), StandardOpenOption.CREATE,
				StandardOpenOption.APPEND);
		}
		catch (java.io.IOException e) {
			println("RESCUE-CALLERS: cannot write " + log + ": " + e.getMessage());
		}
	}
}
