# Agent OOO

**Spa retreats for your AI. Reset, refresh, relax.**

Even autonomous agents need an occasional out-of-office day. After hours of grepping codebases, wrestling compiler errors, and accidentally pasting `.env` credentials into their own logs, your agent's session history gets tense.

**Agent OOO** is a fast, local Rust day spa (`agent-ooo`, alias `ooo`) and Model Context Protocol server (`agent-ooo-mcp`) that checks in tired AI agent transcripts between tasks. It scrubs leaked API keys and tokens from session logs, strips terminal ANSI residue, offloads oversized payloads into cold storage, and locks session metadata into a compact Cap'n Proto checkpoint. Everything runs locally on your machine. Nothing leaves the sanctuary.

## Installation

Agent OOO ships prebuilt binaries (`agent-ooo`, `ooo`, and `agent-ooo-mcp`) for macOS (Apple Silicon) and Linux (`x86_64`).

**Homebrew**

The formula lives in this repository, so pass the repository URL when tapping:

```bash
brew tap mrtibbets/agent-ooo https://github.com/mrtibbets/agent-ooo
brew install agent-ooo
```

**Manual download**

Download the archive for your platform from [GitHub Releases](https://github.com/mrtibbets/agent-ooo/releases), extract it, and place `agent-ooo`, `ooo`, and `agent-ooo-mcp` in your `$PATH`.

**From source** (requires Rust and Cap'n Proto):

```bash
cargo install --git https://github.com/mrtibbets/agent-ooo agent-ooo
cargo install --git https://github.com/mrtibbets/agent-ooo agent-ooo-mcp
```

From a local clone:

```bash
cargo install --path crates/cli
cargo install --path crates/mcp
```

## The spa package

The signature treatment is `spa`. Run it with no arguments to automatically discover your most recent active session across 7 supported agent harnesses (Antigravity, Antigravity IDE, Claude Code, Codex CLI, Cursor, Windsurf, or Cline), or pass a specific `.jsonl` transcript path.

```bash
agent-ooo spa
ooo -y spa
ooo spa ~/.claude/projects/<project>/<session>.jsonl
ooo detox --all
```

Pass `-y` (or `--latest`) to skip the interactive session picker and automatically select the most recent active session. Non-interactive agent shells (`ANTIGRAVITY_AGENT=1`, `CLAUDECODE=1`, `CI=1`, or `TERM=dumb`) automatically select the latest session without blocking.

You can also book individual treatments (`checkin`, `detox`, `reset`, `discharge`, `export`, `init-skill`) a la carte. Run `ooo detox --all` (or `-a`) to run a machine-wide credential posture scan across every discovered session on the host.

### The 4-step treatment

1. **Checkin (Intake, guest vitals & triage):** Admits the session to the spa, parses the transcript in a constant-memory stream (ignoring sidechain sub-queries), extracts guest vitals (agent harness, model ID, tool cadence and error rate, top tools, thinking ratio, and token pace), detects identical tool-and-argument loop locks, and assigns a 4-tier wellness condition (`Elite`, `Fit`, `Fair`, `Fatigued`) alongside a multi-factor "Rot Index."
2. **Detox (Deep-tissue cleanse & credential hygiene):** Strips terminal ANSI escape codes, detects and masks leaked secrets (Anthropic, OpenAI, Google, GitHub, AWS, Bearer tokens, PEM private keys, and JWTs including decoded `role` claims such as `service_role`), pins Generation 0 system directives and durable user anchors, and spills payloads over 512 KB into `blake3` content-addressed cold storage while recording CAS recall handles. Pass `--all` (`-a`) to run a machine-wide credential posture audit across all discovered agent sessions.
3. **Reset (Structural alignment & state compaction):** Correlates tool calls and outcomes across turns (`EventStream`), severs recursive intent-hash error loops within a 6-event sliding window, extracts durable user directives (`NegativeConstraint`) across the full session, captures `validIf`-bound host git registers, and locks both session metadata and the refreshed `gen0Arena` + `transientArena` conversation turns into a zero-copy Cap'n Proto checkpoint (`checkpoint.capnp`) alongside a portable JSON copy.
4. **Discharge (Exit exam, cold-storage recall & spa report):** Runs the 5-check zero-LLM canary suite (`checkpoint_structural_integrity`, `unresolved_tool_failure`, `unverified_late_edit`, `stub_or_conflict_injection` via streaming `git diff -U0 HEAD`, and `environment_register_freshness` for git HEAD drift), evaluates exit wellness (`Elite`, `Fit`, `Fair`, `Fatigued`), supports bounded cold-storage payload retrieval (`ooo discharge --recall <blake3>`), and issues `.agent-ooo/spa_report.md` and `spa_report.json`.

### Take-home robes (`ooo export`)

When handing a session off to another agent or piping conversation history into an external reviewer (such as `vet --history-loader`), `ooo export` streams a secret-scrubbed, ANSI-stripped transcript in constant memory:

```bash
ooo export --format normalized
ooo export --format handoff --max-turns 50 --output handoff.md
```

When `--output` is omitted, `ooo export` suppresses the ASCII banner and streams directly to `stdout` so it can be piped safely without leaking `.env` keys or multi-megabyte tool payloads.

### Turnkey agent skill & lifecycle hooks (`ooo init-skill`)

Generate the turnkey Agent OOO skill bundle (`skills/agent-ooo/SKILL.md`) and pre-push git verification hooks for your project:

```bash
# Generate skill bundle in ./skills/agent-ooo
ooo init-skill

# Install pre-push git hook running ooo discharge before git push
ooo init-skill --hooks
# Or using the alias:
ooo init-hooks

# Target a specific agent harness (antigravity, claude, cursor, codex, git, all)
ooo init-skill -H claude --target .claude/skills/agent-ooo

# Print skill markdown directly to stdout
ooo init-skill --stdout
```

The generated skill teaches agents when to book each treatment (`checkin`, `detox`, `spa`, `discharge`, `export`) and provides harness configuration examples for Antigravity, Claude Code, Cursor, Codex CLI, and Git lifecycle hooks.

## Self-care via MCP (`agent-ooo-mcp`)

Agents can also check themselves into the spa. `agent-ooo-mcp` exposes the same 6 commands over standard I/O using the Model Context Protocol (JSON-RPC 2.0), so any MCP-capable agent can audit its own transcripts for leaked credentials, inspect checkpoints, or export sanitized handoffs as a native tool call.

Add `agent-ooo-mcp` to your MCP client configuration (such as Antigravity, Claude Code, or Cursor):

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

### Treatment menu (MCP tools)

The MCP tool surface mirrors the CLI subcommands 1:1:

* **`spa`:** Runs the full 4-step retreat (`checkin`, `detox`, `reset`, `discharge`) non-interactively and writes checkpoints and `spa_report.md` to `.agent-ooo/`.
* **`checkin`:** Triages a session transcript (Rot Index, token count, tool call repetitions), or lists discovered sessions across Antigravity, Antigravity IDE, Claude Code, Codex CLI, Cursor, Windsurf, and Cline when `list_sessions: true` is passed.
* **`detox`:** Streams a session transcript (or every discovered session on the machine with `all_sessions: true`) in constant memory. Automatically prefers sibling `transcript_full.jsonl` files for Antigravity sessions so truncated tool outputs are still inspected. Hashes raw tokens with `blake3` and returns only masked previews (`first 7...last 4`), line numbers, and occurrence counts so raw credentials are never echoed back into the agent's context window.
* **`reset`:** Correlates tool events, severs recursive intent-hash loops, extracts user negative constraints and git environment registers, and writes `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`.
* **`discharge`:** Runs the 5-check Clinical & Behavioral Canary Suite and deserializes `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`, returning session metadata, `validIf`-bound git environment registers, event stream count, preserved negative constraints, CAS recall handles, persisted arena counts, and structured canary findings (`PASS` / `WARN` / `FAIL`). Pass `recall_handle` (with optional `offset` and `max_bytes`) to retrieve bounded slices of spilled >512 KB cold-storage payloads on demand.
* **`export`:** Streams a secret-scrubbed, ANSI-stripped transcript in `"jsonl"`, `"normalized"`, or `"handoff"` format (rendering `"handoff"` directly from the sanitized, loop-severed `.agent-ooo/checkpoint.capnp` arena when present for the session).

## What gets written to `.agent-ooo/`

Running a treatment leaves your source transcripts untouched and writes its artifacts to `.agent-ooo/` in the target workspace:

```text
.agent-ooo/
├── checkpoint.capnp   # Packed Cap'n Proto binary checkpoint
├── checkpoint.json    # Portable JSON companion checkpoint
├── spa_report.md      # Clinical verification & detox summary for humans
├── spa_report.json    # Machine-readable telemetry certificate
└── spillover/         # Content-addressed cold storage for payloads > 512 KB
    └── <blake3>.bin
```

## Updating

If you installed with Homebrew, upgrade with Homebrew:

```bash
brew upgrade agent-ooo
```

If you installed manually, the built-in updater pulls the latest `agent-ooo` binary from GitHub Releases:

```bash
agent-ooo update
```

`agent-ooo update` replaces the `agent-ooo` binary in place. To update `ooo` and `agent-ooo-mcp` on a manual install, extract them from the latest release archive or run `brew upgrade agent-ooo`.

## Architecture

Agent OOO is organized as a three-crate Rust workspace (`crates/core`, `crates/cli`, `crates/mcp`). It uses Cap'n Proto (`schema/session_checkpoint.capnp`) for zero-copy binary checkpoint serialization (persisting `EnvironmentRegister`, `EventStream`, `NegativeConstraint`, `LossManifest`, `gen0Arena`, and loop-severed `transientArena`), `blake3` for content-addressed payload spilling and zero-retention secret deduplication, and a generational memory arena that pins system instructions (Gen 0) and durable user anchors while cycling transient turns through a ring buffer. See [CHANGELOG.md](CHANGELOG.md) for the complete milestone ledger.

For operational guidelines and pre-commit protocols for AI agents working in this repository, see [AGENTS.md](AGENTS.md).
