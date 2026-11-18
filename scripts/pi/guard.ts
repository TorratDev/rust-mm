// pi extension: run scripts/guard.sh before every bash command and block what it refuses.
// scripts/orch/spawn.sh loads it with `pi -e scripts/pi/guard.ts`. The Claude Code side is the
// PreToolUse hook in .claude/settings.json.
import { type ExtensionAPI, isToolCallEventType } from "@earendil-works/pi-coding-agent";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const guard = fileURLToPath(new URL("../guard.sh", import.meta.url));

export default function (pi: ExtensionAPI) {
  pi.on("tool_call", async (event) => {
    if (!isToolCallEventType("bash", event)) return;
    const r = spawnSync(guard, [event.input.command], { encoding: "utf8" });
    if (r.status === 0) return;
    // Exit 2 is a refusal; anything else means the guard could not decide, so fail closed.
    const reason = r.status === 2 ? r.stderr.trim() : `scripts/guard.sh failed: ${r.stderr || r.error}`;
    return { block: true, reason };
  });
}
