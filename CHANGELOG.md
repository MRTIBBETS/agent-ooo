# Changelog

All notable changes to the **Agent OOO** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **AI Engineering Ledger:** This changelog explicitly tracks the AI models, orchestration runtimes, and core dependencies utilized to author milestones, providing an auditable trail of generative contributions.

## [Unreleased]

### Added
- **Phase 4 of v0.2: Guest Vitals, 4-Tier Wellness Scale, and Report Copy Alignment:**
  - Added `agent_harness_for` in `crates/core/src/checkin/discovery.rs` and extended `TranscriptMetrics` in `crates/core/src/checkin/parser.rs` to extract `model_id`, `user_turns`, `assistant_turns`, `failed_tool_calls`, bounded `tool_frequency` (capped at 64 distinct tool names), `thinking_bytes`, and `output_bytes` in a single streaming pass.
  - Defined `WellnessTier` (`Elite`, `Fit`, `Fair`, `Fatigued`), `ToolUsageStat`, and `AgentVitals` in `crates/core/src/checkin/mod.rs`, and upgraded `calculate_triage` in `crates/core/src/checkin/triage.rs` with a multi-factor Rot Index combining token load (0.0 to 5.0), repetition streak (0.0 to 3.0), and tool friction (0.0 to 2.0).
  - Updated `DischargeCertificate` (`crates/core/src/discharge/mod.rs`), `generate_spa_report` (`crates/core/src/discharge/report.rs`), and `ExportFormat::HandoffMarkdown` (`crates/core/src/detox/exporter.rs`) to share `# Agent OOO Spa Report` YAML frontmatter and H1 titles, strict sentence-case section headings anchored to the 4 treatments (`Checkin`, `Detox`, `Reset`, `Discharge`), and zero parentheses across rendered report templates.
  - *Self-Review Fix:* Capped `TranscriptMetrics.tool_calls` at 4,096 stored entries while tracking `total_tool_calls_count` separately so long-running sessions with thousands of tool invocations remain strictly bounded in RAM, and replaced manual division checks in `triage.rs` with `checked_div`.
  - *Context:* Authored by Antigravity (Gemini 4 Argon Medium) using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Phase 3 of v0.2: Constant-Memory Secret-Scrubbed Conversation Exporter (`crates/core/src/detox/exporter.rs`):**
  - Implemented `export_transcript` and `ExportFormat` (`Jsonl`, `Normalized`, `HandoffMarkdown`) with `ExportReceipt` tracking `lines_exported`, `redactions_count`, `spilled_payloads_count`, and `bytes_written`.
  - Streams transcripts in `O(1)` memory when `max_turns` is `None` (or retains up to 16 pinned Gen 0 system turns plus a bounded `VecDeque` ring buffer when `max_turns` is `Some(n)`), automatically preferring sibling `transcript_full.jsonl` files, skipping `isSidechain: true` entries, stripping ANSI codes, masking leaked credentials, and spilling payloads >512 KB into `.agent-ooo/spillover/<blake3>.bin`.
  - Added `Commands::Export` (`agent-ooo export` / `ooo export`) to `crates/cli/src/lib.rs`, suppressing the ASCII banner and status `stdout` messages when `--output` is omitted so `ooo export` pipes cleanly into downstream verification tools (`--history-loader`).
  - Registered the `export` tool in `agent-ooo-mcp` (`crates/mcp/src/lib.rs`) supporting `"jsonl"`, `"normalized"`, and `"handoff"` formats with a bounded default `max_turns` of 50 or direct file output via `output_path`.
  - *Self-Review Fix:* Clamped `VecDeque::with_capacity` pre-allocation in `export_transcript` to `1..=2048` to prevent oversized upfront allocations when large `max_turns` values are passed, sanitized constraint strings read from `.agent-ooo/checkpoint.json` before rendering the `HandoffMarkdown` header, and handled `ErrorKind::BrokenPipe` gracefully when `ooo export` is piped to truncated readers like `head`.
  - *Context:* Authored by Antigravity (Gemini 4 Argon Medium) using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Phase 2 of v0.2: Zero-LLM Clinical & Behavioral Canary Suite (`crates/core/src/discharge/canary.rs`):**
  - Introduced structured canary diagnostics (`CanaryStatus`, `CanaryFinding`, `CanaryReport`) and `evaluate_canaries(workspace_dir, checkpoint_dir)` executing 4 deterministic checks in under 5ms without an LLM:
    1. `checkpoint_structural_integrity` (`Fail` / `Pass`): Verifies packed `.agent-ooo/checkpoint.capnp` decoding, 32-byte `blake3` event digest lengths, and `session_id` parity with `.agent-ooo/checkpoint.json`.
    2. `unresolved_tool_failure` (`Warn` / `Pass`): Inspects the Cap'n Proto `EventStream` to flag sessions ending on a non-zero exit code in the final tool invocation or final test/build runner (`cargo test`, `cargo check`, `pytest`, `npm test`, `pnpm test`, `go test`, `tsc`).
    3. `unverified_late_edit` (`Warn` / `Pass`): Flags sessions where a mutating file tool (`edit_file`, `write_file`, `Edit`, `Write`, `MultiEdit`, `apply_patch`) ran after the last verification command.
    4. `stub_or_conflict_injection` (`Fail` / `Warn` / `Pass`): Streams `git diff -U0 HEAD` via a bounded `BufReader` (`take(50_000)`) to flag unresolved git merge conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) as `Fail` and newly added stub markers (`todo!(`, `unimplemented!(`, `FIXME:`, `XXX:`) in non-test files as `Warn`.
  - Wired `CanaryReport` into `DischargeCertificate`, `.agent-ooo/spa_report.md` (`## 4. Clinical & Behavioral Canaries`), `CheckpointInspection` (`reset::serializer::inspect_checkpoint`), `agent-ooo discharge`, `agent-ooo spa`, and the `agent-ooo-mcp` `discharge` tool.
  - *Self-Review Fix:* Bounded `decoded_events` allocation in `evaluate_canaries` to 4,096 records and explicitly dropped the `BufReader<ChildStdout>` pipe and terminated the child process before `child.wait()` in `evaluate_git_diff_canary` to prevent potential pipe deadlocks on repositories with diffs exceeding 50,000 lines.
  - *Context:* Authored by Antigravity (Gemini 4 Argon Medium) using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Phase 1 of v0.2: Cap'n Proto State Hydration & Deterministic Intent-Hash Loop Severing:**
  - Extended `DetoxReceipt` and `detox::spillover` (`manage_spillover_with_handle`) to track exact `evicted_bytes` from the `MemoryArena` ring buffer and up to 256 bounded `<blake3>.bin` CAS `recall_handles`, while filtering `isSidechain: true` turns in `checkin::parser` and `detox::pipeline`.
  - Created `crates/core/src/reset/analyzer.rs` to correlate Claude Code (`tool_use` / `tool_result`) and Antigravity (`tool_calls` / `tool_responses`) invocations across turns using a bounded 256-entry lookback queue, producing `ToolEventRecord` entries with normalized `blake3` argument and stdout hashes.
  - Implemented deterministic intent-hash loop severing across a 6-event sliding window (`MemoryArena::sever_loops`), collapsing intermediate duplicate retry turns while preserving the initial attempt and final resolution/failure.
  - Added user directive extraction (`ConstraintRecord`) to capture boundary rules (`never`, `do not`, `don't`, `must not`, `always`, `under no circumstances`, `stop doing`) from user turns into `SessionCheckpoint.constraintLedger` (deduplicated via `blake3` and capped at 64 entries of up to 280 chars).
  - Hydrated `EventStream`, `NegativeConstraint`, and `LossManifest` in `reset::serializer::write_checkpoints` and `inspect_checkpoint` (both `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`), and updated `discharge::report::generate_spa_report` to report severed loops, correlated tool events, preserved constraints, and CAS recall handles.
  - *Self-Review Fix:* Bounded volatile regex normalization in `analyzer::hash_normalized` to a UTF-8-safe 16 KB prefix (`bounded_utf8_prefix`) to avoid multi-hundred-kilobyte string allocations on large tool outputs, and added 32-byte `blake3` digest slice verification on `EventStream` records inside `inspect_checkpoint`.
  - *Context:* Authored by Antigravity (Gemini 4 Argon Medium) using Rust 1.99.1 and Cap'n Proto 1.5.0.

## [0.1.7] - 2026-10-19

### Added
- **MCP Server (`crates/mcp`) and Core Inspection APIs:**
  - Added workspace member `crates/mcp` (`agent-ooo-mcp`), a zero-async-overhead stdio JSON-RPC 2.0 Model Context Protocol server exposing `spa`, `checkin`, `detox`, `reset`, and `discharge` in 1:1 parity with the CLI subcommands.
  - Added `--help` and `--version` flags plus terminal `stderr` hints to `agent-ooo-mcp`, and updated `.github/workflows/release.yml` and `Formula/agent-ooo.rb` so GitHub Releases and Homebrew package and test `agent-ooo-mcp` alongside `agent-ooo` and `ooo`.
  - Implemented `detox::sanitizer::scan_transcript_secrets` to stream transcripts line by line in constant memory, recursively inspect JSON string values, prefer sibling `transcript_full.jsonl` files on Antigravity sessions, extract JWT `role` claims (such as `service_role`) via zero-dependency base64url decoding, deduplicate matches via `blake3` digests so raw secrets are never retained in heap structures, and emit masked previews (`prefix...suffix`).
  - Implemented `reset::serializer::inspect_checkpoint` to deserialize packed `.agent-ooo/checkpoint.capnp` messages alongside `.agent-ooo/checkpoint.json` and cross-verify session IDs and structural integrity.
  - Exported `discover_recent_transcripts`, `resolve_scan_target`, and `resolve_transcript_non_interactive` in `checkin::discovery` so non-interactive tool callers never touch `stdin` or `stdout`.
  - *Self-Review Fix:* Replaced full-token `Vec<char>` allocation in `mask_secret_preview` with bounded iterator extraction (`take(7)` / `rev().take(4)`), added directory resolution to `resolve_scan_target` so passing a session directory resolves its nested transcript, clamped `checkin` session listing limits to `1..=200`, and standardized the MCP execution boundary on `anyhow::Result`.
  - *Context:* Authored by Antigravity (Gemini 4 Argon Medium) using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **AGENTS.md:** Established root operational governance and self-review protocols for future AI agents operating within the repository.
  - *Context:* Authored by Antigravity (Gemini).

## [0.1.0] - 2026-09-26

### Added
- **Phase 1: Workspace & State Schema:**
  - Initialized virtual Cargo workspace (`crates/core`, `crates/cli`).
  - Defined the zero-copy binary state definition (`schema/session_checkpoint.capnp`).
  - Implemented `build.rs` for automatic `capnpc-rust` code generation.
  - *Context:* Authored by Antigravity (Gemini 3.1 Pro High) using Rust 1.98.1 and Cap'n Proto 1.5.0.

- **Phase 2: Intake & Discovery:**
  - Implemented transcript auto-discovery for `.gemini`, `.claude`, and `.cursor` workspaces.
  - Built streaming JSONL parser for high-throughput, low-memory ingestion.
  - Engineered loop lock detection and token degradation scoring (Rot Index).
  - *Context:* Authored by Antigravity (Gemini 3.1 Pro High).

- **Phase 3: Detox & Memory Arena:**
  - Built regex-based syntactic sanitizer to strip ANSI escape codes and redact API keys/Bearer tokens.
  - Implemented `blake3` Content-Addressed Storage (CAS) tripwire: payloads exceeding 512KB are spilled to disk with lightweight stubs injected into memory.
  - Scaffolded Generational Memory Arena (pinned Gen 0 for system prompts, rolling ring buffer for transient context).
  - *Context:* Authored by Antigravity (Gemini).

- **Phase 4: Reset & State Compaction:**
  - Implemented 5 structural normalizer regexes (ISO 8601, Epoch, UUIDv4, Hex, Temp paths) to identify recursive intent despite volatile string mutations.
  - Integrated Git environment register extraction (branch, commit, working directory, diffs).
  - Serialized active memory arena into portable `.agent-ooo/checkpoint.json` and zero-copy `.agent-ooo/checkpoint.capnp`.
  - *Context:* Authored by Antigravity (Gemini).

- **Phase 5: Discharge & Spa Pipeline:**
  - Designed behavioral canary verification interface.
  - Engineered automated economic impact calculator (tokens reduced, dollars saved, developer hours reclaimed).
  - Generated markdown-formatted `spa_report.md` artifact.
  - Wired the universal `agent-ooo spa` CLI command to execute the atomic four-step pipeline.
  - *Context:* Authored by Antigravity (Gemini).

## [0.1.1] - 2026-09-26

### Fixed
- **Pre-Commit Self-Review Executed:**
  - *Memory Safety & Panics:* Refactored `crates/core/src/detox/pipeline.rs` to replace `.unwrap()` on mutable JSON tool response payloads with safe reference rebinding, preventing potential panics on malformed transcripts.
  - *Context:* Authored by Antigravity (Gemini).

## [0.1.2] - 2026-09-26

### Changed
- **CLI Ergonomics & UX Polish:**
  - Reordered the `clap` help menu to elevate `agent-ooo spa` as the primary entry point (display order 1), followed by the individual steps.
  - Refined one-liner descriptions for all commands for punchier readability.
  - Added visual "breathing room" (`\n\n`) before the plain-text banner to reduce terminal clutter.
  - Upgraded the transcript discovery engine (`discovery.rs`) to use `dialoguer`. When multiple active sessions are found across Antigravity, Claude, or Cursor, it now presents a beautiful, interactive arrow-key selection menu with "last active" timestamps instead of silently guessing.
  - *Context:* Authored by Antigravity (Gemini).

## [0.1.3] - 2026-09-28

### Added
- **GitHub Actions CI/CD Pipeline:**
  - Engineered `.github/workflows/release.yml` to automate cross-compilation and artifact generation.
  - Automatically compiles `agent-ooo` and `ooo` release binaries for macOS (Apple Silicon `aarch64-apple-darwin`) and Linux (`x86_64-unknown-linux-gnu`) whenever a version tag (e.g., `v0.1.3`) is pushed.
  - Injects Homebrew and APT dependencies for `capnproto` prior to the Cargo build layer.
  - Bundles the binaries, `README.md`, and `CHANGELOG.md` into `.tar.gz` archives and automatically attaches them to GitHub Releases.
  - *Context:* Authored by Antigravity (Gemini).

## [0.1.4] - 2026-09-28

### Added
- **Built-in `update` Command:**
  - Integrated the `self_update` crate with `rustls` (avoiding OpenSSL dependencies).
  - The CLI can now pull the latest pre-compiled binaries natively via `agent-ooo update`.
- **Public `README.md` Polish:**
  - Rewrote the main repository README to document the 4-step Spa architecture, the zero-copy performance guarantees, and the new `update` command.
  - *Context:* Authored by Antigravity (Gemini).

## [0.1.6] - 2026-10-14

### Fixed
- **Correctness release driven by the Local Machine Context Audit:**
  - *Discovery:* `checkin/discovery.rs` now scans the layouts agents actually write: `~/.gemini/antigravity/brain/<id>/.system_generated/logs/transcript.jsonl`, the same layout under `~/.gemini/antigravity-ide/`, and `~/.claude/projects/<project>/<session>.jsonl`. Legacy paths remain as fallbacks. Previously `agent-ooo spa` found no sessions on a real workstation.
  - *Claude Code parsing:* `checkin/parser.rs` reads the nested `message.content[]` schema (text, thinking, `tool_use`, `tool_result`). Claude sessions previously scored 0.00.
  - *Session IDs:* New `discovery::session_id_for` returns the Antigravity session directory or the Claude session UUID instead of `logs` or `transcript.jsonl`.
  - *Savings math:* `discharge/report.rs` subtracted the entire sanitized footprint from the token total, reporting close to 100% reduction for every session. Before and after are now both derived from detox byte counts.
  - *Byte accounting:* `detox/pipeline.rs` measures sanitized size as raw line length plus the real change in field lengths. Re-serializing silently unescaped `\u003c` sequences and reported them as savings. Unparseable lines are excluded from both totals.
  - *Secret detection:* `detox/sanitizer.rs` adds Anthropic, OpenAI project, Google, GitHub, AWS, JWT, and PEM private key patterns. Bearer tokens require 20+ token characters, so prose such as "Bearer tokens" is no longer redacted.
  - *ANSI stripping:* The old regex contained a literal `|` in its character class and missed most CSI and all OSC sequences.
  - *Versioning:* Workspace version was stuck at 0.1.0 through tags v0.1.3 to v0.1.5, which broke `agent-ooo update`. CLI and banner now read `CARGO_PKG_VERSION`.
  - *Reset receipt:* Environment register count reflects captured registers instead of a hardcoded 4. Loop severing still reports 0 and is scheduled for v0.2.
  - Removed stray `AGENTS.md.orig`.
  - *Honest reporting:* Arena eviction is no longer counted as savings. The checkpoint stores only session metadata and git registers, not the retained turns, so the spa report now shows eviction as an "Arena Compaction Preview" line. Realized savings come from detox only.
  - *Checkpoint integrity:* `discharge/canary.rs` returned a hardcoded `Ok(true)`. It now verifies that both checkpoint files exist, the Cap'n Proto file is non-empty, and the JSON parses with a session ID. Labelled "Checkpoint Integrity" because it is structural, not behavioral.
  - *Non-interactive discovery:* When stdin is not a terminal, auto-discovery selects the most recent session instead of failing with "not a terminal".
  - Test suite grew from 3 to 14 tests during the audit pass.
  - *Self-Review Fix:* Arena eviction accounting now uses a bounded size ledger that mirrors the 100 item ring buffer, rather than re-serializing evicted values.
  - *Self-Review Fix:* Audit v2 surfaced `sk-` false positives on hyphenated words (`task-before-startup`) and anchors (`#ask-multiple-agents`). Added `\b` word boundaries.
  - *Self-update asset naming:* Release archives are now named with the Rust target triple (`agent-ooo-aarch64-apple-darwin.tar.gz`, `agent-ooo-x86_64-unknown-linux-gnu.tar.gz`). `self_update` selects assets by target triple, so the old `macos-aarch64` names could never match.
  - *Homebrew job hardening:* The formula updater uses `curl -f` with `pipefail`, so a missing asset fails the job instead of writing the hash of a 404 page into the formula.
  - *Homebrew instructions:* The README tap command was wrong. A formula in the main repository needs the explicit URL: `brew tap mrtibbets/agent-ooo https://github.com/mrtibbets/agent-ooo`.
  - *Execution telemetry:* `spa_report.md` and `spa_report.json` always recorded 0.00 ms because timing was captured after the report was written. Elapsed time is now passed into `execute_discharge`.
  - *Positioning:* README and `llms.txt` no longer claim loop severing or a "pristine context window". They lead with what the audit measured: local credential masking and checkpointing.
  - Added spillover tests (the only untested detox path) and resolved the two remaining clippy warnings. Test suite: 16 tests.
  - *Context:* Authored by Antigravity (Claude Opus 5.5) using Rust 1.98.1.
