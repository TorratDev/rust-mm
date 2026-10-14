// Finds code in overlays that auto-analysis left empty (V8, issue #18 / #42).
//
// Auto-analysis only follows code from entry points it knows (the ARM9 entry and each
// overlay's static initialisers), so most overlays end up with no functions. For every
// overlay block (ov_NNNN) that has no functions yet, this script:
//   1. creates functions at ARM prologues: word-aligned "stmdb sp!, {..., lr}"
//      (mask 0xFFFF4000 == 0xE92D4000);
//   2. creates functions at the block start and right after each function end
//      ("bx lr" or "ldmia sp!, {..., pc}"), when that word is an unconditional ARM
//      instruction and not inside existing code; this catches leaf functions;
//   3. runs auto-analysis on the changes, so calls and references get followed;
//   4. types literal-pool words read by "ldr rX, [pc, #n]" as pointers when their value
//      lies in the same overlay or in the ARM9. Analysis leaves such pools as undefined
//      words, so nothing showed a reference to, for example, the type chart.
// Overlays smaller than MIN_SIZE are skipped (some are tables of a few words).
//
// Arguments (optional): "dry" counts candidates without changing anything; "all" also
// processes overlays that already have functions; ov_NNNN names limit the run to those.
// Output lines start with "RESCUE-DISCOVER:". Safe to run again.
//
//@category NDS
//@author rescue project
//@runtime Java

import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.Undefined;
import ghidra.program.model.listing.Bookmark;
import ghidra.program.model.listing.BookmarkType;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.RefType;
import ghidra.program.model.symbol.Reference;

public class RescueOverlayDiscover extends GhidraScript {

	private static final int MIN_SIZE = 0x40;

	private static final long PUSH_LR_MASK = 0xFFFF4000L;
	private static final long PUSH_LR = 0xE92D4000L;
	private static final long BX_LR = 0xE12FFF1EL;
	private static final long POP_PC_MASK = 0xFFFF8000L;
	private static final long POP_PC = 0xE8BD8000L;

	@Override
	protected void run() throws Exception {
		Set<String> args = new TreeSet<>(List.of(getScriptArgs()));
		boolean dry = args.remove("dry");
		boolean all = args.remove("all");

		int blocks = 0;
		int totalBefore = 0;
		int totalAfter = 0;
		AddressSet changed = new AddressSet();
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (!b.getName().matches("ov_\\d{4}") || !b.isInitialized()) {
				continue;
			}
			if (!args.isEmpty() && !args.contains(b.getName())) {
				continue;
			}
			int before = countFunctions(b);
			if ((before > 0 && !all) || b.getSize() < MIN_SIZE) {
				continue;
			}
			monitor.checkCancelled();
			blocks++;
			totalBefore += before;
			List<Address> prologues = prologues(b);
			if (dry) {
				say(String.format("%s size 0x%X functions %d, prologue candidates %d",
					b.getName(), b.getSize(), before, prologues.size()));
				continue;
			}
			int made = 0;
			made += create(b.getStart(), true);
			for (Address a : prologues) {
				made += create(a, false);
			}
			made += leafFunctions(b);
			changed.add(b.getStart(), b.getEnd());
			int after = countFunctions(b);
			totalAfter += after;
			say(String.format("%s size 0x%X functions %d -> %d (created %d), bad instructions %d",
				b.getName(), b.getSize(), before, after, made, badInstructions(b)));
		}
		if (dry) {
			say(blocks + " overlays would be processed");
			return;
		}
		if (!changed.isEmpty()) {
			say("running auto-analysis on " + blocks + " overlays ...");
			analyzeChanges(currentProgram);
		}
		int finalTotal = 0;
		int pointers = 0;
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (changed.contains(b.getStart())) {
				finalTotal += countFunctions(b);
				int n = poolPointers(b);
				pointers += n;
				say(b.getName() + ": " + n + " literal-pool words typed as pointers");
			}
		}
		say(String.format("%d overlays processed: functions %d before, %d after discovery, " +
			"%d after analysis; %d pool pointers", blocks, totalBefore, totalAfter, finalTotal,
			pointers));
	}

	/**
	 * Types each undefined pool word read by an instruction in this block as a pointer,
	 * when its value is inside the block (resolved in the overlay's space) or inside the
	 * ARM9 block (resolved in RAM).
	 */
	private int poolPointers(MemoryBlock b) throws Exception {
		MemoryBlock arm9 = currentProgram.getMemory().getBlock("arm9");
		int n = 0;
		for (Instruction i : currentProgram.getListing()
				.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true)) {
			monitor.checkCancelled();
			if (!i.getMnemonicString().startsWith("ldr")) {
				continue;
			}
			for (Reference r : i.getReferencesFrom()) {
				Address pool = r.getToAddress();
				if (!r.getReferenceType().isRead() || !b.contains(pool) ||
					(pool.getOffset() & 3) != 0 || b.getEnd().subtract(pool) < 3) {
					continue;
				}
				Data d = getDataAt(pool);
				if (d != null && !Undefined.isUndefined(d.getDataType())) {
					continue;
				}
				long v = word(pool);
				Address target = null;
				if (v >= b.getStart().getOffset() && v <= b.getEnd().getOffset()) {
					target = b.getStart().getAddressSpace().getAddress(v);
				}
				else if (arm9 != null && v >= arm9.getStart().getOffset() &&
					v <= arm9.getEnd().getOffset()) {
					target = arm9.getStart().getAddressSpace().getAddress(v);
				}
				if (target == null || getInstructionContaining(pool) != null) {
					continue;
				}
				if (d != null) {
					clearListing(pool, pool.add(3));
				}
				createData(pool, PointerDataType.dataType);
				boolean linked = false;
				for (Reference pr : getReferencesFrom(pool)) {
					linked |= pr.getToAddress().equals(target);
				}
				if (!linked) {
					for (Reference pr : getReferencesFrom(pool)) {
						removeReference(pr);
					}
					createMemoryReference(getDataAt(pool), target, RefType.DATA);
				}
				n++;
			}
		}
		return n;
	}

	/** Word-aligned "stmdb sp!, {..., lr}" in the block, in address order. */
	private List<Address> prologues(MemoryBlock b) throws Exception {
		List<Address> out = new ArrayList<>();
		Address a = align4(b.getStart());
		while (a != null && a.compareTo(b.getEnd()) < 0) {
			if (b.getEnd().subtract(a) < 3) {
				break;
			}
			if ((word(a) & PUSH_LR_MASK) == PUSH_LR) {
				out.add(a);
			}
			a = a.add(4);
		}
		return out;
	}

	/**
	 * Creates a function after each function's last return, until no new one appears.
	 * The word must be an unconditional ARM instruction (top nibble 0xE) so literal pools,
	 * which hold addresses (0x02......) or small constants, are not taken for code.
	 */
	private int leafFunctions(MemoryBlock b) throws Exception {
		int made = 0;
		boolean again = true;
		while (again) {
			monitor.checkCancelled();
			again = false;
			List<Address> next = new ArrayList<>();
			for (Instruction i : currentProgram.getListing()
					.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true)) {
				long w = word(i.getAddress());
				if (w == BX_LR || (w & POP_PC_MASK) == POP_PC) {
					Address n = i.getAddress().add(4);
					if (n.compareTo(b.getEnd()) < 0 && b.getEnd().subtract(n) >= 3) {
						next.add(n);
					}
				}
			}
			for (Address n : next) {
				if ((word(n) >>> 28) == 0xE && create(n, false) > 0) {
					made++;
					again = true;
				}
			}
		}
		return made;
	}

	/** Disassembles and creates a function at a, unless a is already code or data. */
	private int create(Address a, boolean blockStart) throws Exception {
		if (getFunctionContaining(a) != null || getDataContaining(a) != null) {
			return 0;
		}
		Instruction existing = getInstructionContaining(a);
		if (existing != null && !existing.getAddress().equals(a)) {
			return 0;
		}
		if (blockStart && (word(a) >>> 28) != 0xE) {
			return 0;
		}
		if (existing == null && !disassemble(a)) {
			return 0;
		}
		return createFunction(a, null) != null ? 1 : 0;
	}

	private int countFunctions(MemoryBlock b) {
		int n = 0;
		FunctionIterator it = currentProgram.getFunctionManager()
				.getFunctions(new AddressSet(b.getStart(), b.getEnd()), true);
		while (it.hasNext()) {
			it.next();
			n++;
		}
		return n;
	}

	private int badInstructions(MemoryBlock b) {
		int n = 0;
		var it = currentProgram.getBookmarkManager().getBookmarksIterator(BookmarkType.ERROR);
		while (it.hasNext()) {
			Bookmark bm = it.next();
			if (b.contains(bm.getAddress())) {
				n++;
			}
		}
		return n;
	}

	private long word(Address a) throws Exception {
		return getInt(a) & 0xFFFFFFFFL;
	}

	private Address align4(Address a) {
		long r = a.getOffset() & 3;
		return r == 0 ? a : a.add(4 - r);
	}

	private void say(String s) {
		println("RESCUE-DISCOVER: " + s);
	}
}
