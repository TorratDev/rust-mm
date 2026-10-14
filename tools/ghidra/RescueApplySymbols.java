// Applies the symbol table in docs/symbols.md to the program, then checks it.
//
// Each table row gives an address (ov_NNNN::ADDR for overlays), a name, a type and a tag.
// The type is a C prototype for a function, a data type for a global, or "-" for a plain
// label. Functions are created when missing; signatures use the compiler's default calling
// convention with dynamic storage and are never noreturn. Then every row is checked and
// OK/FAIL lines are printed. Safe to run again.
//
// Argument (optional): path to the symbols file. Default: docs/symbols.md next to this
// script's tools/ghidra folder. Output lines start with "RESCUE-SYMBOLS:".
//
//@category NDS
//@author rescue project
//@runtime Java

import java.io.File;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

import ghidra.app.cmd.function.ApplyFunctionSignatureCmd;
import ghidra.app.script.GhidraScript;
import ghidra.app.util.parser.FunctionSignatureParser;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.FunctionDefinitionDataType;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.util.data.DataTypeParser;
import ghidra.util.data.DataTypeParser.AllowedDataTypes;

public class RescueApplySymbols extends GhidraScript {

	private record Row(int line, String address, String name, String type, String tag) {
		boolean isLabel() {
			return type.equals("-");
		}

		boolean isFunction() {
			return type.contains("(");
		}
	}

	private int failures;

	@Override
	protected void run() throws Exception {
		File file = getScriptArgs().length > 0 ? new File(getScriptArgs()[0])
				: new File(getSourceFile().getParentFile().getParentFile().getParentFile()
						.getFile(false), "docs/symbols.md");
		List<Row> rows = read(file);
		say("read " + rows.size() + " rows from " + file);

		say("--- apply");
		for (Row r : rows) {
			Address a = parse(r);
			if (a == null) {
				continue;
			}
			try {
				if (r.isFunction()) {
					function(a, r);
				}
				else {
					data(a, r);
				}
			}
			catch (Exception e) {
				say("line " + r.line() + ": " + r.name() + ": " + e.getMessage());
			}
		}

		say("--- check");
		for (Row r : rows) {
			Address a = parse(r);
			if (a != null) {
				check(a, r);
			}
		}
		say(failures == 0 ? "all checks passed" : failures + " check(s) failed");
	}

	/** Table rows: lines starting with "|", minus the header and separator lines. */
	private List<Row> read(File file) throws Exception {
		List<Row> rows = new ArrayList<>();
		List<String> lines = Files.readAllLines(file.toPath());
		for (int i = 0; i < lines.size(); i++) {
			String l = lines.get(i).trim();
			if (!l.startsWith("|") || l.startsWith("| Address") || l.startsWith("|--")) {
				continue;
			}
			String[] c = l.substring(1).split("\\|");
			if (c.length < 4) {
				say("line " + (i + 1) + ": expected at least 4 columns");
				continue;
			}
			rows.add(new Row(i + 1, c[0].trim(), c[1].trim(), c[2].trim(), c[3].trim()));
		}
		return rows;
	}

	private Address parse(Row r) {
		Address a = currentProgram.getAddressFactory().getAddress(r.address());
		if (a == null) {
			say("line " + r.line() + ": cannot resolve address " + r.address());
		}
		return a;
	}

	// --- apply

	private void function(Address a, Row r) throws Exception {
		FunctionDefinitionDataType def = prototype(r);
		Function f = getFunctionAt(a);
		if (f == null) {
			if (getInstructionAt(a) == null) {
				disassemble(a);
			}
			f = createFunction(a, r.name());
			if (f == null) {
				throw new Exception("cannot create a function");
			}
		}
		f.setName(r.name(), SourceType.USER_DEFINED);
		f.setNoReturn(false);
		f.setCustomVariableStorage(false);
		ApplyFunctionSignatureCmd cmd = new ApplyFunctionSignatureCmd(a, def, SourceType.USER_DEFINED);
		if (!cmd.applyTo(currentProgram, monitor)) {
			throw new Exception("signature: " + cmd.getStatusMsg());
		}
		f.setCallingConvention(defaultConvention());
	}

	private void data(Address a, Row r) throws Exception {
		if (!r.isLabel()) {
			DataType t = dataType(r);
			Data d = getDataAt(a);
			if (d == null || !d.getDataType().isEquivalent(t)) {
				clearListing(a, a.add(t.getLength() - 1));
				createData(a, t);
			}
		}
		Symbol s = getSymbolAt(a);
		Function f = getFunctionAt(a);
		if (f != null) {
			f.setName(r.name(), SourceType.USER_DEFINED);
		}
		else if (s == null || !s.getName().equals(r.name())) {
			createLabel(a, r.name(), true, SourceType.USER_DEFINED);
		}
	}

	// --- check

	private void check(Address a, Row r) throws Exception {
		String where = r.name() + " @ " + a;
		if (r.isFunction()) {
			Function f = getFunctionAt(a);
			if (!check(f != null, where + " is a function")) {
				return;
			}
			FunctionDefinitionDataType want = prototype(r);
			check(f.getName().equals(r.name()) && !f.hasNoReturn() &&
				!f.hasCustomVariableStorage() &&
				defaultConvention().equals(f.getCallingConventionName()) &&
				f.getReturnType().isEquivalent(want.getReturnType()) &&
				f.getParameterCount() == want.getArguments().length,
				where + " is " + want.getPrototypeString());
			return;
		}
		Symbol s = getSymbolAt(a);
		boolean named = (s != null && s.getName().equals(r.name())) ||
			(getFunctionAt(a) != null && getFunctionAt(a).getName().equals(r.name()));
		if (r.isLabel()) {
			check(named, where + " is labelled");
			return;
		}
		Data d = getDataAt(a);
		check(named && d != null && d.getDataType().isEquivalent(dataType(r)),
			where + " is " + r.type());
	}

	private boolean check(boolean ok, String what) {
		if (!ok) {
			failures++;
		}
		say((ok ? "OK   " : "FAIL ") + what);
		return ok;
	}

	// --- helpers

	private FunctionDefinitionDataType prototype(Row r) throws Exception {
		FunctionDefinitionDataType def =
			new FunctionSignatureParser(currentProgram.getDataTypeManager(), null)
					.parse(null, r.type());
		if (!def.getName().equals(r.name())) {
			throw new Exception("line " + r.line() + ": prototype names " + def.getName());
		}
		return def;
	}

	private DataType dataType(Row r) throws Exception {
		return new DataTypeParser(currentProgram.getDataTypeManager(),
			currentProgram.getDataTypeManager(), null, AllowedDataTypes.ALL).parse(r.type());
	}

	private String defaultConvention() {
		return currentProgram.getCompilerSpec().getDefaultCallingConvention().getName();
	}

	private void say(String s) {
		println("RESCUE-SYMBOLS: " + s);
	}
}
