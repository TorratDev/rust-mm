# {{Subsystem}}

**Date:** {{date}}
**Dump:** {{dump, by name and SHA-1}}
**Issue:** {{#n}}
**Workflow:** [workflow.md](workflow.md)

Each claim below carries one status: **blue-verified**, **red-hypothesis** or **lead**. A claim with no evidence is not a fact. Move a row to **blue-verified** only when its evidence cell is filled in.

## Claims

| Claim | Evidence | Status |
|-------|----------|--------|
| {{what is stated}} | {{Blue address, e.g. `ov_0001::021B3F58`; routine reference: function name, address and paraphrase; or trace reference: script, output hash, command}} | {{blue-verified / red-hypothesis / lead}} |
|       |          |        |

## Addresses

| Address | Name | Source |
|---------|------|--------|
|         |      |        |

## Routines

Name each routine the claims depend on, with its address and a one-line paraphrase. Do not paste decompiler output.

| Function | Address | What it does (paraphrase) |
|----------|---------|---------------------------|
|          |         |                           |

## Trace checks

| Script | Output hash | Command | Claims checked |
|--------|-------------|---------|----------------|
|        |             |         |                |

Outputs live in an ignored scratch directory. Record only the hash and the command here.

## Red comparison

What `pmd-red` does here, cited by file and function name only. Mark it **red-hypothesis** until Blue confirms it.

## Open questions

-

## Links

- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
