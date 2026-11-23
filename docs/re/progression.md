# Progression: level-up, moves, recruiting, IQ, evolution, quiz

**Date:** 2026-11-22
**Status:** blue-verified (per section tag below; names are hypotheses unless said)
**Subsystem:** dungeon progression, party, ARM9 data tables

Issues: [#361](https://github.com/TorratDev/rust-mm/issues/361) (runtime checks, P1 to P3) and [#75](https://github.com/TorratDev/rust-mm/issues/75) (static P1 to P7). This note merges the investigators' comments on both. Addresses are `ov_0001::` unless plain (ARM9). Static facts come from the Ghidra text export (STAMP 2026-11-14). Runtime facts come from py-desmume on the dump with SHA-1 `6f01887356268de39085df01490347bbcc3aa330`; the probes are scratch files, not in the repo. "Forced" means a probe wrote a register or byte to pass a gate; each forced step is named. Tags: **blue-verified**, **red-hypothesis**, **lead** ([workflow](workflow.md#status-tags)). Table contents are not copied ([`legal.md`](../legal.md)): the notes give addresses, shapes and rules.

The note is split into one file per topic, so parallel runs on different topics do not edit the same file. The address table and the open questions are one-line-per-entry lists ([workflow](workflow.md#shared-lists)).

## Topics

- [P1. Level-up [blue-verified; runtime with forced EXP]](progression/p1-level-up.md)
- [P2. Move learning [blue-verified; flow at runtime]](progression/p2-move-learning.md)
- [P3. Recruiting [blue-verified; gates forced at runtime; natural recruit not observed]](progression/p3-recruiting.md)
- [P4. Friend areas [blue-verified, static; runtime results tagged blue-verified runtime, the rest unverified]](progression/p4-friend-areas.md)
- [P5. IQ [blue-verified, static]](progression/p5-iq.md)
- [P6. Evolution [blue-verified static; dungeon-row and apply runs emulator-verified (forced); town runtime not run]](progression/p6-evolution.md)
- [P7. Personality quiz and starter [blue-verified, static]](progression/p7-quiz-and-starter.md)
- [Red comparison](progression/red-comparison.md)

## Shared lists

- [Addresses](progression/addresses.md)
- [Open questions (follow-ups)](progression/open-questions.md)

## Links
- Symbols: [symbols.md](../symbols.md)
- Verification: [verification.md](../verification.md)
