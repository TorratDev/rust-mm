// Dumps the whole program to text files, so that read-only questions (what does this
// function do, who refers to this address) are answered with a text search instead of a
// running Ghidra bridge. See docs/ghidra-setup.md, section 7.
//
// Output, in the directory given as the first argument:
//   <space>/<address>_<name>.c  one decompiled function each; <space> is ram or ov_NNNN
//   functions.tsv               address, name, signature, file (empty file = see FAILED)
//   xrefs.tsv                   every reference to a memory address: from, from_function,
//                               type, to, to_name
//   FAILED                      functions that did not decompile, with the reason
//   STAMP                       date, program and counts; written last, so a missing STAMP
//                               means the export did not finish
//
// Addresses are written as Ghidra prints them: 0219d058 in the ARM9, ov_0001::0219d058
// in an overlay. Calls into an overlay from outside it land on the ram address (see
// docs/ghidra-setup.md), so search xrefs.tsv for the bare offset (0219d058) to get the
// references in every space.
//
// The output is derived from the retail ROM: the directory must be outside the
// repository, and nothing in it may be committed (docs/legal.md). The directory must be
// empty or missing, so that no file from an older export survives a rename.
//
// The script only reads the program. Rerun it after anything that renames or adds
// functions (RescueApplySymbols, RescueRngLabels, RescueOverlayDiscover).
//
// GUI: run from the Script Manager; it asks for the directory. Headless or ghidra-cli:
// pass the directory, e.g.
//   -postScript RescueExport.java /home/me/rescue-re/export
//
//@category NDS
//@author rescue project
//@runtime Java

import java.io.BufferedWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDate;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceManager;
import ghidra.program.model.symbol.Symbol;

public class RescueExport extends GhidraScript {

	private static final int TIMEOUT_SECONDS = 60;

	@Override
	protected void run() throws Exception {
		String[] args = getScriptArgs();
		Path dir = args.length > 0 ? Path.of(args[0])
				: askDirectory("Export to (outside the repository)", "Export").toPath();
		if (Files.exists(dir)) {
			try (Stream<Path> entries = Files.list(dir)) {
				if (entries.findAny().isPresent()) {
					throw new IllegalArgumentException(
						dir + " is not empty; remove it first so no stale file survives");
				}
			}
		}
		Files.createDirectories(dir);

		int functions = 0;
		List<String> failed = new ArrayList<>();
		DecompInterface decompiler = new DecompInterface();
		decompiler.openProgram(currentProgram);
		try (BufferedWriter table = Files.newBufferedWriter(dir.resolve("functions.tsv"))) {
			table.write("address\tname\tsignature\tfile\n");
			for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
				if (monitor.isCancelled()) {
					throw new IllegalStateException("cancelled; the export is incomplete");
				}
				Address entry = f.getEntryPoint();
				String file = entry.getAddressSpace().getName() + "/" +
					String.format("%08x", entry.getOffset()) + "_" + fileName(f.getName()) + ".c";
				DecompileResults r = decompiler.decompileFunction(f, TIMEOUT_SECONDS, monitor);
				if (r.decompileCompleted()) {
					Path out = dir.resolve(file);
					Files.createDirectories(out.getParent());
					Files.writeString(out, "// " + entry + " " + f.getName() + "\n" +
						r.getDecompiledFunction().getC());
				}
				else {
					failed.add(entry + "\t" + f.getName() + "\t" + r.getErrorMessage().trim());
					file = "";
				}
				table.write(entry + "\t" + f.getName() + "\t" +
					f.getSignature().getPrototypeString() + "\t" + file + "\n");
				functions++;
			}
		}
		finally {
			decompiler.dispose();
		}

		int refs = 0;
		ReferenceManager manager = currentProgram.getReferenceManager();
		try (BufferedWriter table = Files.newBufferedWriter(dir.resolve("xrefs.tsv"))) {
			table.write("from\tfrom_function\ttype\tto\tto_name\n");
			AddressIterator sources =
				manager.getReferenceSourceIterator(currentProgram.getMemory(), true);
			while (sources.hasNext()) {
				Address from = sources.next();
				Function fromFunction = getFunctionContaining(from);
				for (Reference r : manager.getReferencesFrom(from)) {
					Address to = r.getToAddress();
					if (!to.isMemoryAddress()) {
						continue; // stack and register references
					}
					Symbol label = getSymbolAt(to);
					Function toFunction = getFunctionContaining(to);
					String toName = label != null ? label.getName()
							: toFunction != null ? toFunction.getName() : "";
					table.write(from + "\t" + (fromFunction == null ? "" : fromFunction.getName()) +
						"\t" + r.getReferenceType() + "\t" + to + "\t" + toName + "\n");
					refs++;
				}
			}
		}

		Files.write(dir.resolve("FAILED"), failed);
		String summary = functions + " functions (" + failed.size() + " failed), " + refs +
			" references";
		Files.writeString(dir.resolve("STAMP"),
			LocalDate.now() + " " + currentProgram.getName() + ": " + summary + "\n");
		println("RESCUE-EXPORT: " + summary + " -> " + dir);
	}

	/** A function name made safe as a file name on Windows and Linux. */
	private static String fileName(String name) {
		String safe = name.replaceAll("[^A-Za-z0-9_.-]", "_");
		return safe.length() > 80 ? safe.substring(0, 80) : safe;
	}
}
