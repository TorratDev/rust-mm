# Contributing

Plans and decisions live in [`docs/roadmap.md`](docs/roadmap.md). Assumption checks are logged in [`docs/verification.md`](docs/verification.md). External sources and their trust levels are in [`docs/references.md`](docs/references.md).

## Legal hygiene policy

The full policy is [`docs/legal.md`](docs/legal.md). Every contribution must follow it. In short:

- **Dumps are identified by SHA-1.** Tools accept only supported dumps and refuse others.
- **Fixtures are generated at test time** from the user's dump. Tests skip cleanly when no dump is configured.
  Configure your dump as described in [`docs/supported-dumps.md`](docs/supported-dumps.md); writing and running ROM-gated tests is in [`docs/rom-tests.md`](docs/rom-tests.md).
- **Release binaries and CI artifacts contain no ROM data.**
- **Use the neutral name.** Crates, binaries and packages use the `rescue` prefix. No "Pokémon" or Nintendo trademarks in names.
- **`pmd-red` is read-only.** Read and cite it; do not copy its code or data. Fetch it with `tools/fetch_pmd_red.sh`.

Before committing, check `git status` and the diff for anything the `.gitignore` rules might miss. If ROM data was committed by mistake, tell the maintainer instead of deleting it in a follow-up commit.

## Evidence

Mark facts as verified in Blue or as hypotheses (for example, anything taken from Red or Explorers). Record how a fact was confirmed: address, file, trace, or command.
