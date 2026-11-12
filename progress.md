Phase: 3 (Reverse engineer the original game), Epic: #5

| sub-issue | status | attempts | executor | worktree | PR | follow-up |
|-----------|--------|----------|----------|----------|----|-----------|
| #42 Ghidra project | done (closed) | — | — | — | — | — |
| #43 glossary | done (closed) | — | — | — | — | #151 |
| #44 RE main loop | done (closed) | — | — | — | — | #157–#161 |
| #45 RE input/rendering | done (closed) | — | — | — | — | #175–#178 |
| #46 RE dungeon state | done (closed) | — | — | — | — | #163–#166 |
| #47 RE RNG (priority) | done (closed) | — | — | — | — | #168–#171, #173 |
| #48 RE floor generation | done (closed) | — | — | — | — | #179 |
| #49 RE turn scheduler | done (closed) | — | — | — | — | #181, #182, #184 |
| #50 RE combat | done (closed) | — | — | — | — | #185–#187 |
| #51 RE status effects | done (closed) | — | — | — | — | #189–#191 |
| #52 RE AI | done (closed) | — | — | — | — | #193, #194 |
| #53 RE save format | done (closed) | — | — | — | — | #197 |
| #54 golden-trace spec | waiting: spec merged (#153); done when #56 uses it | 0 | — | — | #153 | — |
| #55 trace capture scripts | done (closed) | — | — | — | — | — |
| #56 trace-harness CLI | in progress: split into #331, #332, #333 (#68 sim core is closed) | 0 | — | — | — | — |
| #331 harness CLI, loader, diff engine | done (closed); reviewer PASS; PR #364 squash-merged (no CI checks on the repo) | 0 | executor-1 | removed | #364 | — |
| #332 starting state from first row | partial: PR #365 merged (Refs); reviewer PASS; #332 stays open | 0 | executor-1 | removed | #365 | #366 (row-0 ROM test, tilesSha256 equality) |
| #366 row-0 equality and tilesSha256 (follow-up of #332) | queued: needs captured trace and sim work (#111, entity model) | 0 | — | — | — | — |
| #367 schema v2: record decoded leader action (follow-up of #333; user approved) | done (reviewer PASS); PR #368 squash-merged; #367 stays open (runtime capture unverified: no emulator) | 0 | executor-1 | removed | #368 | — |
| #333 key inputs to sim commands | partial: option A (static). PR #369 squash-merged (Refs); reviewer PASS; #333 stays open until a captured trace runs | 0 | executor-1 | removed | #369 | captured-trace run (needs emulator) |
| #57 REA workflow | done (closed) | — | — | — | — | — |
| #58 blue-vs-red log | done (closed) | 1 | executor-1 | removed | #237 | — |
| #151 glossary immune | done (closed) | — | — | — | #155 | — |
| #157 IRQ handler | done (closed) | — | — | — | #208 | — |
| #158 boot entry | done (closed) | — | — | — | — | — |
| #159 per-frame RNG | done (closed) | 1 | executor-2 | removed | #238 | — |
| #160 ground bank map | done (closed) | 0 | executor-2 | removed | #241, #259 | #258 |
| #161 scene-runner names | done (closed) | 1 | executor-1 | removed | #244 | — |
| #163–#166 entity/floor/status | done (closed) | — | executor-2 | removed | #245–#248 | — |
| #168–#171, #173 RNG | done (closed) | — | — | removed | #249–#253, #212 | — |
| #175 input bindings | done (closed) | 0 | executor-3 | removed | #254 | — |
| #176 touch | done (closed) | 0 | executor-2 | stale 176-touch (clean, superseded by #255); to remove | #229, #235, #255 | — |
| #177 screens and fades | done (closed) | — | — | — | #227, #230, #269 | — |
| #178 file loading | done (closed) | — | — | — | #279 | #277 |
| #179 floor gen tail | done (closed) | — | — | — | #226, #256 | — |
| #181 running/swapping | done (closed) | — | — | — | #257 | — |
| #182 per-turn effects | done (closed) | — | — | — | #265 | — |
| #184 movement liquids | done (closed) | — | — | — | #273 | — |
| #185–#187 combat | done (closed) | — | — | — | #200–#205 | — |
| #189–#191 status | done (closed) | — | — | — | #206 | — |
| #193 AI runtime RNG | done (closed) | — | — | — | #272 | #270 |
| #194 AI helpers | done (closed) | — | — | — | #276 | #275 |
| #197 save runtime | done (closed) | 0 | executor-1 | removed | #240 | #239 |
| #239 save follow-up | done (closed) | — | — | — | #264 | #260, #262 |
| #258 scene entry scripts | done (closed) | — | — | — | #271 | #267 |
| #262 roster cell | done (closed) | — | — | — | #274 | — |
| #275 AI indirect calls | done (closed) | — | — | — | #281 | #278 |
| #277 file loading 2 | done (closed) | — | — | — | #282 | #280 |
| #278 AI runtime targets | done (closed) | — | — | — | #287 | — |
| #280 FS dispatch | done (closed) | — | — | — | #284 | #283 |
| #283 FS hook/AT3P | done (closed) | — | — | — | #286 | — |
| #260 valid-save load | blocked: needs a real Blue save image (maintainer) | 0 | — | — | — | — |
| #267 scene entry runtime | partial (#288 Refs); points posted; partial: P1 merged (#290, Refs); P2 natural reach blocked on save (#260); verification.md:506 fixed in #293 | 0 | executor-3 | removed | #288, #290, #293 | — |
| #270 message LCG check | done (closed; P1 refuted, P2 rng.md, P3 probes committed) | 1 | executor-1 | removed | #291 | — |
| #289 docs leftovers | done (closed) | 1 | executor-2 | removed | #292, #293 | — |

Platform: Windows, hc + herdr-app. Space epic-5 (sp58d5182bab871b6a812ae2d56bd07eb2), orchestrator tab tb2be6a5e9bff2db29f1adbaa5b26acd11 (t2c). Restarted 2026-11-07; resumed 2026-11-12: #56 was stale (#68 closed), now split into #331–#333. Tabs: executors (tbf330bdbaf7bfacdd7cd1d7ae8ad8f782), investigators (tbed9917e3456682367777a42a57305ab3). Ghidra copies a/b/c/r in rust-mm/rescue-re/ghidra (gitignored). Blocked on a human: #260 and #267 P2 (save image, maintainer). #54 closes with #56.
