# ROM-dependent tests

Most parsers can only be checked properly against real game data, but the ROM is never committed or put in CI ([`legal.md`](legal.md)). Tests that need it are **ROM-gated**: they run locally when a supported dump is configured and skip everywhere else.

## Running them locally

1. Point the tools at your dump: set `RESCUE_ROM=/path/to/dump.nds`, or put `rom_path = "..."` in a `rescue.toml` at the workspace root (git-ignored). The dump must be on the [supported list](supported-dumps.md).
2. Run the tests as usual:

   ```sh
   RESCUE_ROM=~/roms/my-dump.nds cargo nextest run --workspace
   ```

3. To make sure nothing was skipped, add `RESCUE_REQUIRE_ROM=1`: a ROM-gated test then fails if no ROM is configured.

| Situation | ROM-gated test |
|-----------|----------------|
| No ROM configured (CI) | prints `SKIPPED <test>: no ROM configured ...` and passes |
| No ROM, `RESCUE_REQUIRE_ROM=1` | fails |
| ROM configured but missing, unreadable or not a supported dump | fails, with the computed SHA-1 and the supported list |
| Supported ROM configured | runs |

The skip line goes to stderr; `cargo nextest run --no-capture` or `cargo test -- --nocapture` shows it.

## Writing one

Use the `rom_test!` macro from `rescue-tests` (`tests/src/rom.rs`):

```rust
rescue_tests::rom_test! {
    /// Doc comments and attributes are kept.
    fn header_parses(rom) {
        let bytes = rom.read();             // whole dump
        let out = rom.fixture_dir("header"); // target/rom-fixtures/<sha1>/header
        // ...
    }
}
```

`rom` is a `TestRom` with the path, the typed `RomId` and the SHA-1. Rules:

- **Fixtures are generated at test time** from the dump, into `rom.fixture_dir(..)` (under `target/`, git-ignored). Never copy them into the repository.
- **Snapshots** (`insta`) store hashes, sizes or decoded metadata, never decoded asset bytes.
- The helper itself is tested without a ROM, using synthetic files and an injected supported-dumps table (`tests/tests/rom_gate.rs`). Do the same for new parsers: unit-test them on synthetic data and keep ROM-gated tests as an extra check.
