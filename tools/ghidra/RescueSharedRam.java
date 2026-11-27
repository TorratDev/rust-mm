// Maps the top 4 KiB of main RAM (0x027FF000..0x027FFFFF), which the ARM9 and the ARM7
// share, as an uninitialised, volatile block named "shared". Both CPUs keep words there
// that the other one writes, such as the key shadow word 0x027FFFA8 (docs/re/io.md), so
// the area has no file contents and its values exist only at run time.
//
// Run it on arm9.bin and on arm7.bin. tools/ghidra/import.sh and import.ps1 run it after
// RescueNdsImport.java on a fresh import. On an existing project, headless:
//   analyzeHeadless <project dir> rescue-arm9 -process arm9.bin -noanalysis \
//     -scriptPath tools/ghidra -postScript RescueSharedRam.java
// It does nothing when the block is already there, and fails if another block overlaps
// the range. See docs/ghidra-setup.md.
//
//@category NDS
//@author rescue project
//@runtime Java

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;

public class RescueSharedRam extends GhidraScript {

	private static final long START = 0x027FF000L;
	private static final long SIZE = 0x1000L;
	private static final String NAME = "shared";

	@Override
	protected void run() throws Exception {
		Memory mem = currentProgram.getMemory();
		Address start = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(START);
		Address end = start.add(SIZE - 1);
		MemoryBlock existing = mem.getBlock(NAME);
		if (existing != null) {
			if (!existing.getStart().equals(start) || existing.getSize() != SIZE) {
				throw new IllegalStateException("block " + NAME + " exists at " +
					existing.getStart() + " size 0x" + Long.toHexString(existing.getSize()) +
					", expected " + start + " size 0x" + Long.toHexString(SIZE));
			}
			println("RESCUE: shared already mapped in " + currentProgram.getName());
			return;
		}
		for (MemoryBlock b : mem.getBlocks()) {
			if (b.getStart().getAddressSpace() == start.getAddressSpace() &&
				b.getStart().compareTo(end) <= 0 && b.getEnd().compareTo(start) >= 0) {
				throw new IllegalStateException(b.getName() + " (" + b.getStart() + " - " +
					b.getEnd() + ") overlaps " + start + " - " + end);
			}
		}
		MemoryBlock b = mem.createUninitializedBlock(NAME, start, SIZE, false);
		b.setRead(true);
		b.setWrite(true);
		b.setExecute(false);
		b.setVolatile(true);
		b.setComment("Main RAM shared by the ARM9 and the ARM7 (SDK work area); run-time values only");
		println("RESCUE: block shared " + b.getStart() + " - " + b.getEnd() + " size 0x" +
			Long.toHexString(SIZE) + "  uninitialised, in " + currentProgram.getName());
	}
}
