---
name: agent-ooo
description: Autonomous context rot diagnostics, secret detox, loop severing, state compaction, and workspace drift verification for AI coding agents.
---

# Agent OOO operational skill guide

Agent OOO is a high-performance local day spa, state compaction engine, and verification harness for AI coding agents. It detects context rot, masks leaked credentials, strips ANSI terminal escape noise, severs repetitive error loops, offloads oversized payloads into cold storage, and locks session state into zero-copy Cap'n Proto checkpoints.

All treatments run locally in bounded memory. Source transcripts are never modified unless `--redact-in-place` is explicitly passed. Treatment receipts and artifacts are saved to `.agent-ooo/` in the workspace.

## 1. Operational triggers and treatment menu

Call the appropriate Agent OOO command based on session vitals, lifecycle stage, and risk profile.

### When to call checkin

Run `ooo checkin` (or MCP `checkin`) when:
* Starting a new phase in a long-running session to inspect current context health.
* You suspect context rot, cognitive drift, or high token consumption.
* You want to audit tool call cadence, tool error rates, and API spend ledger without modifying any state.

What it returns:
* **Session ID and harness detection:** Identifies the active session and host harness across Antigravity, Claude Code, Cursor, Codex CLI, Windsurf, or Cline.
* **Guest condition and rot index:** Computes a multi-factor degradation score from 0.0 to 10.0 across 4 wellness tiers (`Elite`, `Fit`, `Fair`, `Fatigued`).
* **Cognitive loop lock detection:** Flags identical consecutive `(tool_name, arguments)` intent signatures repeating 3 or more times.
* **Token and spend ledger:** Reports exact API input, cache read, and output tokens alongside an estimated session cost in USD.

```bash
# Triage the most recent active session
ooo checkin

# Render a compact, screenshot-ready ANSI terminal vitals card
ooo checkin --card

# Non-interactive triage (skips interactive picker)
ooo -y checkin

# Triage a specific session transcript
ooo checkin /path/to/transcript.jsonl
```

### When to call detox

Run `ooo detox` (or MCP `detox`) when:
* Inspecting or sanitizing session logs where credentials, secrets, or bearer tokens may have leaked.
* Output logs contain excessive terminal ANSI color or cursor escape noise.
* Tool outputs exceed 512 KB and should be evicted into content-addressed cold storage.
* Conducting a host-wide credential posture audit via `ooo detox --all` (or `ooo detox -a`).
* Scrubbing leaked credentials directly out of historical transcript files with `ooo detox --redact-in-place --backup`.

What it executes:
* **Secret sanitization:** Masks Anthropic, OpenAI, Google (`AIza`, `AQ.`), GitHub, AWS, JWT (including decoded `role` claims), Bearer, and PEM keys. Credentials are masked as `prefix...suffix` and deduplicated with Blake3 digests so raw secrets are never echoed into context.
* **Syntactic cleanse:** Strips ANSI escape sequences and normalizes whitespace in constant memory.
* **CAS spillover:** Offloads payloads over 512 KB into `.agent-ooo/spillover/<blake3>.bin` and records recall handles in `LossManifest`.
* **Machine-wide audit:** Scans every discovered session on the host across all 7 supported agent harnesses when `--all` is supplied.
* **Safe in-place scrubbing:** When `--redact-in-place` is passed, streams each target file (including sibling `transcript_full.jsonl` files) line by line into a same-directory temp file, redacts secrets without altering clean lines or stripping ANSI sequences, optionally writes and `blake3`-verifies a `.bak` copy (`--backup`) without clobbering existing backups, and atomically replaces the target file.

```bash
# Detox current session
ooo detox

# Machine-wide credential scan across all sessions
ooo detox --all

# Scrub leaked secrets in place with a verified .bak backup
ooo detox --redact-in-place --backup

# Scrub all discovered sessions in place with verified backups
ooo detox --all --redact-in-place --backup
```

### When to call spa

Run `ooo spa` (or MCP `spa`) when:
* The session Rot Index is 5.0 or higher (`Fair` or `Fatigued`).
* A cognitive loop lock is detected (3 or more consecutive identical tool failures).
* Preparing for a major context reset, turn compaction, or cross-agent handoff.

What it executes:
1. **Checkin:** Parses transcripts in constant memory, calculates guest vitals, and evaluates degradation.
2. **Detox:** Strips ANSI noise, redacts leaked keys, and spills oversized payloads into Blake3 CAS.
3. **Reset:** Correlates tool events into an `EventStream`, severs recursive retry loops within a 6-event sliding window, extracts durable user negative constraints, captures live git environment registers, and serializes state to `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`.
4. **Discharge:** Runs the 5-check zero-LLM canary suite, computes footprint reduction, and generates `.agent-ooo/spa_report.md` and `.agent-ooo/spa_report.json`.

```bash
# Run full spa treatment on latest session
ooo -y spa

# Run spa treatment on a specific transcript
ooo spa /path/to/transcript.jsonl
```

### When to call discharge

Run `ooo discharge` (or MCP `discharge`) when:
* Performing a pre-commit, pre-push, or pre-PR health check.
* Verifying checkpoint integrity and ensuring no unresolved tool failures linger.
* Detecting unverified late file modifications or accidental stub injections.
* Recalling spilled cold-storage CAS payloads via `--recall <blake3>`.

What it verifies:
* **Checkpoint structural integrity:** Validates `.agent-ooo/checkpoint.capnp` decoding and Blake3 event digests.
* **Unresolved tool failure:** Flags sessions terminating on non-zero tool exit codes or broken test suites.
* **Unverified late edit:** Flags file edits made after the last test or verification command.
* **Stub or conflict injection:** Streams `git diff -U0 HEAD` to detect merge conflict markers or unresolved `todo!` stubs.
* **Environment register freshness:** Checks for live git HEAD or branch drift against the checkpointed state.

```bash
# Run verification exit exam
ooo discharge

# Recall a cold-storage CAS payload (bounded 32 KB slice)
ooo discharge --recall 8a3f9c4b1e2d3...

# Recall with specific offset and byte limit
ooo discharge --recall 8a3f9c4b1e2d3... --offset 1024 --max-bytes 4096
```

### When to call export

Run `ooo export` (or MCP `export`) when:
* Handing off context to another agent, subagent, or human reviewer.
* Feeding scrubbed session transcripts into external security or audit tools.
* Generating a clean, secret-redacted summary of recent conversation turns.

```bash
# Export normalized sanitized JSONL to stdout
ooo export --format normalized

# Export loop-severed handoff markdown for another agent
ooo export --format handoff --max-turns 50 --output handoff.md
```

## 2. Harness integration guides

Agent OOO integrates directly with popular coding agent harnesses and version control.

### Antigravity

Antigravity natively discovers skills located in `skills/` within the workspace or under `~/.gemini/antigravity/skills/`.

1. Generate the skill bundle at the project root:
   ```bash
   ooo init-skill --target ./skills/agent-ooo
   ```
2. For MCP tool access, register `agent-ooo-mcp` in your Antigravity configuration.
3. Antigravity subagents can trigger `checkin`, `detox`, `spa`, `discharge`, and `export` via native tool calls.

### Claude Code

Claude Code reads project guidelines from `CLAUDE.md` and custom skills from `.claude/skills/`.

1. Install the skill:
   ```bash
   ooo init-skill --target .claude/skills/agent-ooo -H claude
   ```
2. Add reference in `CLAUDE.md`:
   ```markdown
   ## Agent OOO protocols
   Before completing a task, run `ooo discharge` to verify workspace integrity.
   If loop lock occurs or Rot Index exceeds 5.0, execute `ooo spa`.
   ```
3. To enable MCP tools, add `agent-ooo-mcp` to `claude.json`:
   ```json
   {
     "mcpServers": {
       "agent-ooo": {
         "command": "agent-ooo-mcp",
         "args": []
       }
     }
   }
   ```

### Cursor

Cursor uses `.cursor/rules/` for repository-level operational rules.

1. Generate the skill bundle or export rules:
   ```bash
   ooo init-skill --target .cursor/rules/agent-ooo -H cursor
   ```
2. Add `agent-ooo-mcp` in Cursor MCP Server settings (stdio command: `agent-ooo-mcp`).
3. Use `ooo discharge` as the final step before concluding code edits.

### OpenAI Codex CLI

For OpenAI Codex CLI environments:

1. Generate the skill bundle:
   ```bash
   ooo init-skill -H codex
   ```
2. Add instructions to `CODEX.md`:
   ```markdown
   ## Operational guidelines
   Always run `ooo discharge` before reporting task completion.
   Sanitize session credentials with `ooo detox --all` if credentials were exposed.
   ```

### Git lifecycle hooks

Install a fast pre-push canary gate that prevents pushing broken or unverified code to remotes.

1. Install the pre-push hook:
   ```bash
   ooo init-skill --hooks
   # Or using the alias:
   ooo init-hooks
   ```
2. This creates `.git/hooks/pre-push` with executable permissions:
   ```bash
   #!/usr/bin/env bash
   set -euo pipefail
   if command -v ooo >/dev/null 2>&1; then
       ooo discharge
   elif command -v agent-ooo >/dev/null 2>&1; then
       agent-ooo discharge
   fi
   ```
3. Any git push will automatically execute the 5-point canary suite prior to pushing commits.

## 3. Best practices for autonomous agents

1. **Check vitals early:** On long-running tasks, invoke `checkin` every 15 to 20 turns to monitor token pace and tool friction.
2. **Never push with failing canaries:** If `ooo discharge` reports `WARN` or `FAIL` on late edits or unresolved tool errors, run verification commands (`cargo test`, `npm test`, `pytest`) before finishing.
3. **Respect CAS offloading:** When tool outputs are spilled, reference them by Blake3 handle rather than re-requesting large dumps.
4. **Clean handoffs:** Always generate handoff markdown with `ooo export --format handoff --output handoff.md` when transferring ownership to a secondary agent.
