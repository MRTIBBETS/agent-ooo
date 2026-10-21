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

The signature treatment is `spa`. Run it with no arguments to automatically discover your most recent active session across Antigravity, Antigravity IDE, Claude Code, or Cursor, or pass a specific `.jsonl` transcript path.

```bash
agent-ooo spa
ooo spa ~/.claude/projects/<project>/<session>.jsonl
```

You can also book individual treatments (`checkin`, `detox`, `reset`, `discharge`) a la carte.

### The 4-step treatment

1. **Checkin (Intake & triage):** Admits the session to the spa, parses the transcript in a constant-memory stream (ignoring sidechain sub-queries), and calculates a context degradation "Rot Index."
2. **Detox (Deep-tissue cleanse & credential hygiene):** Strips terminal ANSI escape codes, detects and masks leaked secrets (Anthropic, OpenAI, Google, GitHub, AWS, Bearer tokens, PEM private keys, and JWTs including decoded `role` claims such as `service_role`), and spills payloads over 512 KB into `blake3` content-addressed cold storage while recording CAS recall handles.
3. **Reset (Structural alignment & loop severing):** Correlates tool calls and outcomes across turns (`EventStream`), severs recursive intent-hash error loops within a 6-event sliding window, extracts durable user directives (`NegativeConstraint`), captures host git registers, and locks session state into a zero-copy Cap'n Proto checkpoint (`LossManifest` included) alongside a portable JSON copy.
4. **Discharge (Exit exam & clean bill of health):** Verifies structural checkpoint integrity (including 32-byte `blake3` event stream hashes) and issues a signed `.agent-ooo/spa_report.md` and `spa_report.json` summarizing detox results, severed loops, preserved constraints, and an arena compaction preview.

## Self-care via MCP (`agent-ooo-mcp`)

Agents can also check themselves into the spa. `agent-ooo-mcp` exposes the exact same 5 treatments over standard I/O using the Model Context Protocol (JSON-RPC 2.0), so any MCP-capable agent can audit its own transcripts for leaked credentials or inspect checkpoints as a native tool call.

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
* **`checkin`:** Triages a session transcript (Rot Index, token count, tool call repetitions), or lists discovered sessions across Antigravity, Antigravity IDE, Claude Code, and Cursor when `list_sessions: true` is passed.
* **`detox`:** Streams a session transcript (or every discovered session on the machine with `all_sessions: true`) in constant memory. Automatically prefers sibling `transcript_full.jsonl` files for Antigravity sessions so truncated tool outputs are still inspected. Hashes raw tokens with `blake3` and returns only masked previews (`first 7...last 4`), line numbers, and occurrence counts so raw credentials are never echoed back into the agent's context window.
* **`reset`:** Correlates tool events, severs recursive intent-hash loops, extracts user negative constraints and git environment registers, and writes `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`.
* **`discharge`:** Verifies structural checkpoint integrity and deserializes `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`, returning session metadata, git environment registers, event stream count, preserved negative constraints, CAS recall handles, and arena counts.

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

Agent OOO is organized as a three-crate Rust workspace (`crates/core`, `crates/cli`, `crates/mcp`). It uses Cap'n Proto (`schema/session_checkpoint.capnp`) for binary checkpoint serialization, `blake3` for content-addressed payload spilling and zero-retention secret deduplication, and a generational memory arena that pins system instructions (Gen 0) while cycling transient turns through a ring buffer. The arena is not yet persisted to the checkpoint. See [CHANGELOG.md](CHANGELOG.md) for what is built today and what is planned next.

For operational guidelines and pre-commit protocols for AI agents working in this repository, see [AGENTS.md](AGENTS.md).
