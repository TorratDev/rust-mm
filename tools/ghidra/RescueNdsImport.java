// Maps an NDS ARM9 binary and its overlays at their real addresses, from the
// manifest.json written by tools/nds_extract_code.py (Phase 0 check V8, issue #42).
//
// Use on a program whose language is ARM:LE:32:v5t (import arm9.bin as a "Raw Binary"
// with that language). The script REPLACES all memory blocks of the program with:
//   arm9                 ARM9 static code/data at its RAM address
//   arm9_bss             ARM9 static BSS (uninitialised)
//   autoload_N_XXXXXXXX  each autoload section (ITCM, DTCM, ...) at its destination,
//                        plus autoload_N_bss when it has BSS
//   ov_NNNN              each ARM9 overlay as an overlay block (overlays share RAM
//                        ranges, so each gets its own overlay address space)
//   ov_NNNN_bss          the overlay's BSS, in the same overlay space
//   io                   ARM9 I/O registers at 0x04000000, with labels
// and labels the entry point, module params and overlay static initialisers.
//
// Headless: tools/ghidra/import.sh or import.ps1 (runs this as a -preScript, so auto
// analysis runs on the mapped program). Script argument: path to manifest.json.
// GUI: import arm9.bin as Raw Binary, language ARM:LE:32:v5t, do NOT analyse yet,
// run this script from the Script Manager (it asks for manifest.json), then analyse.
//
// Never commit the Ghidra project or the extracted binaries (docs/legal.md).
// See docs/ghidra-setup.md.
//
//@category NDS
//@author rescue project
//@runtime Java

import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import ghidra.app.util.headless.HeadlessScript;
import ghidra.program.database.mem.FileBytes;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.address.OverlayAddressSpace;
import ghidra.program.model.data.DWordDataType;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.lang.Register;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.SymbolTable;

public class RescueNdsImport extends HeadlessScript {

	private static final String FORMAT = "rescue-nds-code-manifest";
	private static final int VERSION = 1;
	private static final String LANGUAGE = "ARM:LE:32:v5t";

	private Memory mem;
	private AddressSpace ram;
	private File dir;
	private int created = 0;

	@Override
	protected void run() throws Exception {
		try {
			map();
		}
		catch (Exception e) {
			if (isRunningHeadless()) {
				// Do not save a half-mapped program.
				setHeadlessContinuationOption(HeadlessContinuationOption.ABORT_AND_DELETE);
			}
			printerr("RESCUE: FAILED: " + e.getMessage());
			throw e;
		}
	}

	private void map() throws Exception {
		File manifestFile;
		String[] args = getScriptArgs();
		if (args.length > 0) {
			manifestFile = new File(args[0]);
		}
		else {
			manifestFile = askFile("Select manifest.json from nds_extract_code.py", "Map");
		}
		if (!manifestFile.isFile()) {
			throw new IllegalArgumentException("manifest not found: " + manifestFile);
		}
		dir = manifestFile.getAbsoluteFile().getParentFile();
		JsonObject m = JsonParser.parseString(
			new String(Files.readAllBytes(manifestFile.toPath()), StandardCharsets.UTF_8))
				.getAsJsonObject();
		if (!FORMAT.equals(str(m, "format")) || m.get("version").getAsInt() != VERSION) {
			throw new IllegalArgumentException(
				"not a " + FORMAT + " v" + VERSION + " manifest: " + manifestFile);
		}
		String lang = currentProgram.getLanguageID().getIdAsString();
		if (!LANGUAGE.equals(lang)) {
			throw new IllegalStateException("program language is " + lang + ", expected " +
				LANGUAGE + ". Re-import arm9.bin with language " + LANGUAGE + ".");
		}
		JsonObject rom = m.getAsJsonObject("rom");
		if (!rom.get("supported").getAsBoolean()) {
			println("RESCUE: WARNING: manifest comes from an UNSUPPORTED/SYNTHETIC ROM " +
				str(rom, "sha1"));
		}

		mem = currentProgram.getMemory();
		ram = currentProgram.getAddressFactory().getDefaultAddressSpace();

		// Start from a clean memory map (drops the raw-binary block of the importer).
		for (MemoryBlock b : mem.getBlocks()) {
			mem.removeBlock(b, monitor);
		}
		// The raw-binary importer marks the ARM exception vectors (relative to its base
		// address) as entry points; on an NDS ARM9 these are not code entry points.
		SymbolTable symbols = currentProgram.getSymbolTable();
		List<Address> stale = new ArrayList<>();
		symbols.getExternalEntryPointIterator().forEachRemaining(stale::add);
		for (Address a : stale) {
			symbols.removeExternalEntryPoint(a);
		}

		JsonObject a9 = m.getAsJsonObject("arm9");
		FileBytes a9fb = fileBytes(str(a9, "file"), a9.get("size").getAsLong());
		long a9ram = num(a9, "ram_address");
		currentProgram.setImageBase(addr(a9ram), true);

		JsonObject st = a9.getAsJsonObject("static");
		MemoryBlock arm9 = mem.createInitializedBlock("arm9", addr(num(st, "ram_address")), a9fb,
			st.get("file_offset").getAsLong(), st.get("size").getAsLong(), false);
		perms(arm9, true, true, true, "ARM9 static, " + str(a9, "file") +
			(a9.get("compressed").getAsBoolean() ? " (BLZ-decoded)" : ""));
		report(arm9);

		if (a9.has("bss") && !a9.get("bss").isJsonNull()) {
			JsonObject bss = a9.getAsJsonObject("bss");
			uninit("arm9_bss", num(bss, "ram_address"), bss.get("size").getAsLong(),
				"ARM9 static BSS");
		}

		for (JsonElement e : a9.getAsJsonArray("autoloads")) {
			JsonObject al = e.getAsJsonObject();
			long dest = num(al, "ram_address");
			String name = String.format("autoload_%d_%08X", al.get("index").getAsInt(), dest);
			long size = al.get("size").getAsLong();
			if (size > 0) {
				Address a = addr(dest);
				if (overlaps(a, size)) {
					println("RESCUE: WARNING: " + name + " overlaps an existing block; " +
						"mapped as an overlay block instead");
				}
				MemoryBlock b = mem.createInitializedBlock(name, a, a9fb,
					al.get("file_offset").getAsLong(), size, overlaps(a, size));
				perms(b, true, true, true, "ARM9 autoload section " + al.get("index") +
					" (copied here by crt0 at boot), from " + str(a9, "file") + " +0x" +
					Long.toHexString(al.get("file_offset").getAsLong()));
				report(b);
			}
			long bssSize = al.get("bss_size").getAsLong();
			if (bssSize > 0) {
				uninit(name + "_bss", num(al, "bss_address"), bssSize,
					"BSS of autoload section " + al.get("index"));
			}
		}

		// Entry point and module params.
		Address entry = addr(num(a9, "entry"));
		createLabel(entry, "_start", true, SourceType.IMPORTED);
		addEntryPoint(entry);
		setThumb(entry, false);
		if (a9.has("module_params") && !a9.get("module_params").isJsonNull()) {
			Address mp = addr(num(a9.getAsJsonObject("module_params"), "ram_address"));
			if (mem.contains(mp)) {
				createLabel(mp, "ModuleParams", true, SourceType.IMPORTED);
				for (int i = 0; i < 9; i++) {
					clearListing(mp.add(i * 4));
					createData(mp.add(i * 4), DWordDataType.dataType);
				}
			}
		}

		// Overlays: one overlay address space per overlay.
		int nOv = 0;
		for (JsonElement e : m.getAsJsonArray("overlays")) {
			monitor.checkCancelled();
			JsonObject ov = e.getAsJsonObject();
			int id = ov.get("id").getAsInt();
			String name = String.format("ov_%04d", id);
			long size = ov.get("size").getAsLong();
			FileBytes fb = fileBytes(str(ov, "file"), size);
			MemoryBlock b;
			if (size > 0) {
				b = mem.createInitializedBlock(name, addr(num(ov, "ram_address")), fb, 0, size,
					true);
			}
			else {
				b = mem.createUninitializedBlock(name, addr(num(ov, "ram_address")),
					Math.max(1, ov.get("bss_size").getAsLong()), true);
			}
			perms(b, true, true, true, "ARM9 overlay " + id + ", file id " + ov.get("file_id") +
				(ov.get("compressed").getAsBoolean() ? ", BLZ-decoded" : "") + ", " +
				str(ov, "file"));
			report(b);
			AddressSpace space = b.getStart().getAddressSpace();
			long bssSize = ov.get("bss_size").getAsLong();
			if (bssSize > 0 && size > 0) {
				MemoryBlock bb = mem.createUninitializedBlock(name + "_bss",
					inSpace(space, num(ov, "bss_address")), bssSize, true);
				perms(bb, true, true, false, "BSS of ARM9 overlay " + id);
				report(bb);
			}
			createLabel(b.getStart(), name + "_start", true, SourceType.IMPORTED);
			sinit(ov, space, name);
			nOv++;
		}

		ioRegisters();
		println("RESCUE: mapped arm9 + " + nOv + " overlays, " + created +
			" blocks; ROM SHA-1 " + str(rom, "sha1"));
	}

	// ----------------------------------------------------------------- helpers

	private FileBytes fileBytes(String name, long size) throws Exception {
		File f = new File(dir, name);
		if (f.length() != size) {
			throw new IllegalStateException(f + " is " + f.length() + " bytes, manifest says " +
				size + ". Re-run nds_extract_code.py.");
		}
		try (InputStream in = new FileInputStream(f)) {
			return mem.createFileBytes(name, 0, size, in, monitor);
		}
	}

	private void uninit(String name, long start, long size, String comment) throws Exception {
		Address a = addr(start);
		// The ARM9 BSS usually starts where the autoload data sits in arm9.bin; that data is
		// not mapped there, but trim against anything already mapped to be safe.
		MemoryBlock clash = mem.getBlock(a);
		if (clash != null) {
			long skip = clash.getEnd().getOffset() + 1 - start;
			if (skip >= size) {
				println("RESCUE: WARNING: " + name + " lies inside " + clash.getName() + "; skipped");
				return;
			}
			a = addr(start + skip);
			size -= skip;
		}
		boolean ov = overlaps(a, size);
		if (ov) {
			println("RESCUE: WARNING: " + name + " overlaps a block; mapped as an overlay block");
		}
		MemoryBlock b = mem.createUninitializedBlock(name, a, size, ov);
		perms(b, true, true, false, comment);
		report(b);
	}

	private boolean overlaps(Address a, long size) {
		Address end = a.add(size - 1);
		for (MemoryBlock b : mem.getBlocks()) {
			if (b.getStart().getAddressSpace() != a.getAddressSpace()) {
				continue;
			}
			if (b.getStart().compareTo(end) <= 0 && b.getEnd().compareTo(a) >= 0) {
				return true;
			}
		}
		return false;
	}

	private void sinit(JsonObject ov, AddressSpace space, String name) throws Exception {
		JsonObject si = ov.getAsJsonObject("static_init");
		long s = num(si, "start"), e = num(si, "end");
		if (s == 0 || e <= s || (e - s) % 4 != 0) {
			return;
		}
		Address start = inSpace(space, s);
		if (!mem.getBlock(start).isInitialized() || !mem.contains(inSpace(space, e - 1))) {
			return;
		}
		createLabel(start, name + "_sinit", true, SourceType.IMPORTED);
		for (long p = s; p < e; p += 4) {
			Address pa = inSpace(space, p);
			clearListing(pa);
			createData(pa, PointerDataType.dataType);
			long target = mem.getInt(pa) & 0xFFFFFFFFL;
			if (target == 0) {
				continue;
			}
			Address t = space.getAddress(target & ~1L);
			if (mem.contains(t)) {
				setThumb(t, (target & 1) != 0);
				addEntryPoint(t);
			}
		}
	}

	private void setThumb(Address a, boolean thumb) throws Exception {
		Register tmode = currentProgram.getRegister("TMode");
		if (tmode != null) {
			currentProgram.getProgramContext().setValue(tmode, a, a,
				thumb ? BigInteger.ONE : BigInteger.ZERO);
		}
	}

	// ARM9 I/O registers (names as in GBATEK). Cheap labels so analysis output reads well.
	private static final Object[][] IO = {
		{0x000, "REG_DISPCNT"}, {0x004, "REG_DISPSTAT"}, {0x006, "REG_VCOUNT"},
		{0x008, "REG_BG0CNT"}, {0x00A, "REG_BG1CNT"}, {0x00C, "REG_BG2CNT"},
		{0x00E, "REG_BG3CNT"}, {0x010, "REG_BG0HOFS"}, {0x012, "REG_BG0VOFS"},
		{0x040, "REG_WIN0H"}, {0x048, "REG_WININ"}, {0x04A, "REG_WINOUT"},
		{0x04C, "REG_MOSAIC"}, {0x050, "REG_BLDCNT"}, {0x052, "REG_BLDALPHA"},
		{0x054, "REG_BLDY"}, {0x060, "REG_DISP3DCNT"}, {0x064, "REG_DISPCAPCNT"},
		{0x06C, "REG_MASTER_BRIGHT"},
		{0x0B0, "REG_DMA0SAD"}, {0x0B4, "REG_DMA0DAD"}, {0x0B8, "REG_DMA0CNT"},
		{0x0BC, "REG_DMA1SAD"}, {0x0C0, "REG_DMA1DAD"}, {0x0C4, "REG_DMA1CNT"},
		{0x0C8, "REG_DMA2SAD"}, {0x0CC, "REG_DMA2DAD"}, {0x0D0, "REG_DMA2CNT"},
		{0x0D4, "REG_DMA3SAD"}, {0x0D8, "REG_DMA3DAD"}, {0x0DC, "REG_DMA3CNT"},
		{0x0E0, "REG_DMA0FILL"}, {0x0E4, "REG_DMA1FILL"}, {0x0E8, "REG_DMA2FILL"},
		{0x0EC, "REG_DMA3FILL"},
		{0x100, "REG_TM0CNT_L"}, {0x102, "REG_TM0CNT_H"}, {0x104, "REG_TM1CNT_L"},
		{0x106, "REG_TM1CNT_H"}, {0x108, "REG_TM2CNT_L"}, {0x10A, "REG_TM2CNT_H"},
		{0x10C, "REG_TM3CNT_L"}, {0x10E, "REG_TM3CNT_H"},
		{0x130, "REG_KEYINPUT"}, {0x132, "REG_KEYCNT"},
		{0x180, "REG_IPCSYNC"}, {0x184, "REG_IPCFIFOCNT"}, {0x188, "REG_IPCFIFOSEND"},
		{0x1A0, "REG_AUXSPICNT"}, {0x1A2, "REG_AUXSPIDATA"}, {0x1A4, "REG_ROMCTRL"},
		{0x204, "REG_EXMEMCNT"}, {0x208, "REG_IME"}, {0x210, "REG_IE"}, {0x214, "REG_IF"},
		{0x240, "REG_VRAMCNT_A"}, {0x241, "REG_VRAMCNT_B"}, {0x242, "REG_VRAMCNT_C"},
		{0x243, "REG_VRAMCNT_D"}, {0x244, "REG_VRAMCNT_E"}, {0x245, "REG_VRAMCNT_F"},
		{0x246, "REG_VRAMCNT_G"}, {0x247, "REG_WRAMCNT"}, {0x248, "REG_VRAMCNT_H"},
		{0x249, "REG_VRAMCNT_I"},
		{0x280, "REG_DIVCNT"}, {0x290, "REG_DIV_NUMER"}, {0x298, "REG_DIV_DENOM"},
		{0x2A0, "REG_DIV_RESULT"}, {0x2A8, "REG_DIVREM_RESULT"},
		{0x2B0, "REG_SQRTCNT"}, {0x2B4, "REG_SQRT_RESULT"}, {0x2B8, "REG_SQRT_PARAM"},
		{0x300, "REG_POSTFLG"}, {0x304, "REG_POWCNT1"},
		{0x1000, "REG_DISPCNT_SUB"},
		{0x100000, "REG_IPCFIFORECV"}, {0x100010, "REG_CARD_DATA_RD"},
	};

	private void ioRegisters() throws Exception {
		long base = 0x04000000L;
		MemoryBlock io = mem.createUninitializedBlock("io", addr(base), 0x2000, false);
		perms(io, true, true, false, "ARM9 I/O registers");
		io.setVolatile(true);
		report(io);
		MemoryBlock io2 = mem.createUninitializedBlock("io_ipc", addr(base + 0x100000), 0x20,
			false);
		perms(io2, true, true, false, "IPC FIFO receive / game card data");
		io2.setVolatile(true);
		report(io2);
		for (Object[] r : IO) {
			createLabel(addr(base + (Integer) r[0]), (String) r[1], true, SourceType.IMPORTED);
		}
	}

	private void perms(MemoryBlock b, boolean r, boolean w, boolean x, String comment) {
		b.setRead(r);
		b.setWrite(w);
		b.setExecute(x);
		b.setComment(comment);
	}

	private void report(MemoryBlock b) {
		created++;
		println(String.format("RESCUE: block %-22s %s - %s  size 0x%X%s%s", b.getName(),
			b.getStart(), b.getEnd(), b.getSize(), b.isOverlay() ? "  overlay" : "",
			b.isInitialized() ? "" : "  uninitialised"));
	}

	// OverlayAddressSpace.getAddress() falls back to the base space for offsets the
	// overlay does not cover yet; this keeps the address in the overlay space.
	private static Address inSpace(AddressSpace space, long offset) {
		if (space instanceof OverlayAddressSpace os) {
			return os.getAddressInThisSpaceOnly(offset);
		}
		return space.getAddress(offset);
	}

	private Address addr(long a) {
		return ram.getAddress(a);
	}

	private static String str(JsonObject o, String k) {
		return o.get(k).getAsString();
	}

	private static long num(JsonObject o, String k) {
		JsonElement e = o.get(k);
		if (e.getAsJsonPrimitive().isString()) {
			return Long.decode(e.getAsString());
		}
		return e.getAsLong();
	}
}
