# Agent OOO

**Spa retreats for your AI. Reset, refresh, relax.**

Agent OOO is a fast, local Rust CLI that inspects and cleans AI agent transcripts between tasks. It masks credentials that leak into session logs, strips ANSI noise, offloads oversized payloads, and writes a compact Cap'n Proto checkpoint of session metadata. Everything runs locally. Nothing is sent anywhere.

## Installation

Agent OOO ships as a single binary for macOS (Apple Silicon) and Linux (x86_64).

**Homebrew**

The formula lives in this repository, so the tap needs the repository URL:

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

The primary command is `spa`. It automatically discovers your most recent active session (Antigravity, Antigravity IDE, Claude Code, or Cursor) and runs the full 4-step treatment process. Pass a transcript path to target a specific session.

```bash
agent-ooo spa
agent-ooo spa ~/.claude/projects/<project>/<session>.jsonl
```

### The 4-step treatment:
1. **Checkin:** Parses your transcript history and calculates a degradation "Rot Index."
2. **Detox:** Strips terminal ANSI codes, masks leaked API secrets (Anthropic, OpenAI, Google, GitHub, AWS, JWT, Bearer, PEM), and spills payloads larger than 512KB to disk.
3. **Reset:** Captures git environment registers and locks session state into a zero-copy Cap'n Proto checkpoint. Intent-hash loop severing is planned for v0.2.
4. **Discharge:** Verifies checkpoint integrity and generates a `spa_report.md` detailing detox savings and an arena compaction preview.

## MCP server

`agent-ooo-mcp` exposes Agent OOO over standard I/O using the Model Context Protocol (JSON-RPC 2.0), allowing agents to audit sessions and inspect checkpoints directly as tool calls.

Build and run from a local checkout:

```bash
cargo build --release -p agent-ooo-mcp
./target/release/agent-ooo-mcp
```

Add `agent-ooo-mcp` to your MCP client configuration (such as Claude Code, Antigravity, or Cursor):

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

### Exposed MCP tools

The MCP tool names match the CLI subcommands 1:1:

* **`spa`:** Executes the full 4-step pipeline (`checkin`, `detox`, `reset`, `discharge`) non-interactively and writes checkpoints and `spa_report.md` to `.agent-ooo/`.
* **`checkin`:** Triages a session transcript (Rot Index, token count, tool call repetitions), or lists discovered sessions across Antigravity, Antigravity IDE, Claude Code, and Cursor when `list_sessions: true` is passed.
* **`detox`:** Streams a session transcript (or all discovered sessions with `all_sessions: true`) in constant memory, preferring sibling `transcript_full.jsonl` files for Antigravity sessions so truncated tool outputs are still scanned. Returns the detox receipt along with deduplicated secret classifications, line ranges, occurrence counts, and masked previews (`first 7...last 4`) without echoing raw credentials into the agent context.
* **`reset`:** Extracts git environment registers and writes zero-copy `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json` files.
* **`discharge`:** Verifies structural checkpoint integrity and deserializes `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`, returning session metadata, git environment registers, and arena counts.

## Updating

If you installed with Homebrew, update with Homebrew:

```bash
brew upgrade agent-ooo
```

If you installed manually, the built-in updater downloads the latest release binary from GitHub:

```bash
agent-ooo update
```

`agent-ooo update` replaces the `agent-ooo` binary only. If you also use the `ooo` alias, reinstall it from the same release archive.

## Architecture

Agent OOO uses Cap'n Proto for the checkpoint format, `blake3` for content-addressed payload spilling, and a generational memory arena that pins system instructions while cycling older turns. The arena is not yet persisted to the checkpoint. See [CHANGELOG.md](CHANGELOG.md) for what is built and what is planned.

For full operational agent guidelines, see [AGENTS.md](AGENTS.md).
