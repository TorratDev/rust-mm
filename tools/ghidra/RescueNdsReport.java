// Prints a short report of a program mapped by RescueNdsImport.java, for recording
// V8 evidence in docs/verification.md: memory blocks, function counts per block, and
// the references to given addresses.
//
// Arguments (optional, any number): addresses to check references to, written as
//   BLOCK+0xOFFSET   offset from the start of a memory block, e.g. ov_0001+0x72688
//   SPACE::ADDRESS   e.g. ov_0001::02172688, or a plain RAM address 02000000
//
// Headless: pass after -postScript RescueNdsReport.java (import.sh/import.ps1 do this
// when given --check ARGS). GUI: run from the Script Manager; it asks for one address.
// Output lines start with "RESCUE-REPORT:".
//
//@category NDS
//@author rescue project
//@runtime Java

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;

public class RescueNdsReport extends GhidraScript {

	@Override
	protected void run() throws Exception {
		String[] args = getScriptArgs();
		if (args.length == 0 && !isRunningHeadless()) {
			String a = askString("References to", "BLOCK+0xOFFSET or address (blank: none)", "");
			args = a.isBlank() ? new String[0] : new String[] { a.trim() };
		}
		say("program " + currentProgram.getName() + ", language " +
			currentProgram.getLanguageID() + ", image base " + currentProgram.getImageBase());
		MemoryBlock[] blocks = currentProgram.getMemory().getBlocks();
		say(blocks.length + " memory blocks");
		int total = 0;
		for (MemoryBlock b : blocks) {
			int n = 0;
			FunctionIterator it = currentProgram.getFunctionManager()
					.getFunctions(new ghidra.program.model.address.AddressSet(b.getStart(),
						b.getEnd()), true);
			while (it.hasNext()) {
				it.next();
				n++;
			}
			total += n;
			say(String.format("block %-22s %s - %s  size 0x%X  %s%s  functions %d", b.getName(),
				b.getStart(), b.getEnd(), b.getSize(), b.isOverlay() ? "overlay" : "base",
				b.isInitialized() ? "" : " uninitialised", n));
		}
		say("functions total " + total);
		for (String a : args) {
			Address addr = parse(a);
			if (addr == null) {
				say("cannot resolve address " + a);
				continue;
			}
			Reference[] refs = getReferencesTo(addr);
			say("references to " + addr + " (" + a + "): " + refs.length);
			for (Reference r : refs) {
				Function f = getFunctionContaining(r.getFromAddress());
				say("  from " + r.getFromAddress() + " " + r.getReferenceType() +
					(f == null ? "" : " in " + f.getName() + " @ " + f.getEntryPoint()));
			}
		}
	}

	private Address parse(String a) {
		int plus = a.indexOf('+');
		if (plus > 0) {
			MemoryBlock b = currentProgram.getMemory().getBlock(a.substring(0, plus));
			if (b == null) {
				return null;
			}
			return b.getStart().add(Long.decode(a.substring(plus + 1)));
		}
		return currentProgram.getAddressFactory().getAddress(a);
	}

	private void say(String s) {
		println("RESCUE-REPORT: " + s);
	}
}
