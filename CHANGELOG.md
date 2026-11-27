# Changelog

All notable changes to the **Agent OOO** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **AI Engineering Ledger:** This changelog explicitly tracks the AI models, orchestration runtimes, and core dependencies utilized to author milestones, providing an auditable trail of generative contributions.

## [Unreleased]

### Added
- **Cycle 7 (`B3`): Schema Drift Doctor (`ooo checkin --doctor` & MCP `doctor: true`):**
  - Refactored `parse_transcript` in `crates/core/src/checkin/parser.rs` to delegate to `parse_transcript_with_diagnostics` using a reusable `String` buffer via `BufReader::read_line`. Added `ParseDiagnostics` tracking `total_non_empty_lines`, `valid_json_lines`, `malformed_json_lines`, `sidechain_skipped_lines`, `recognized_role_turns`, `unknown_role_turns`, bounded `unknown_top_level_keys` (max 24), and bounded `suspected_tool_keys` (max 12) for unrecognized tool fields (`function_call`, `functionCall`, `toolUse`, `tool_use`, `tool_executions`, `actions`, `invocations`, `calls`, `tool_input`).
  - Added `crates/core/src/checkin/doctor.rs` (exported via `pub mod doctor;` in `crates/core/src/checkin/mod.rs`) implementing `SUPPORTED_HARNESSES`, `DoctorSeverity` (`Pass`, `Warn`, `Fail`), `DoctorFinding`, `SessionDoctorResult`, `HarnessDoctorSummary`, `DoctorReport`, `diagnose_transcript`, `diagnose_single_to_report`, `diagnose_candidates`, and `diagnose_discovered_transcripts` (bounded to 200 recent sessions). Diagnoses unreadable files, empty transcripts, malformed JSONL ratios, unrecognized schemas (with sample top-level keys), zero-tool-call anomalies (flagging suspected unrecognized tool keys), missing API or estimable token telemetry, and missing model IDs across all 7 supported harnesses.
  - Added `--doctor` to `Commands::Checkin` in `crates/cli/src/lib.rs` with `validate_checkin_flags` (rejecting `--card` + `--doctor` together) and `print_doctor_report` rendering 7-harness coverage and up to 20 non-`PASS` session diagnostics.
  - Added `"doctor": boolean` to the `checkin` tool schema and `execute_checkin` dispatch in `crates/mcp/src/lib.rs` for both explicit `transcript` targets and host-wide discovery.
  - Added 8 new tests across `crates/core/src/checkin/doctor.rs` (`6`), `crates/cli/src/lib.rs` (`1`), and `crates/mcp/src/lib.rs` (`1`), bringing total workspace tests to `78/78`, and synced `README.md`, `llms.txt`, and `skills/agent-ooo/SKILL.md`.
  - *Self-Review Fix:* Added `recognized_role_turns` to `ParseDiagnostics` (`crates/core/src/checkin/parser.rs`) and wired it to `SessionDoctorResult.recognized_turns` (`crates/core/src/checkin/doctor.rs`) so valid JSON lines with unrecognized non-`UNKNOWN` event `type` keys are not counted as recognized turns.
  - *Context:* Authored by Antigravity Tier-2 Subagent (Gemini 3.8 Flash) and reviewed by Coordinator using Rust 1.99.1 and Cap'n Proto 1.5.0.

## [0.3.0] - 2026-11-25

### Added
- **Cycle 6 (`B2`): Git Pre-Push Canary Gate (`ooo hook install`):**
  - Added `scan_line_for_secrets` in `crates/core/src/detox/sanitizer.rs` using the `has_secret_candidate` fast-path and `SECRET_RE` (capped at 16 matches per line) to emit `(secret_type, masked_preview)` pairs without retaining raw secret material.
  - Added `crates/core/src/discharge/gate.rs` (exported via `crates/core/src/discharge/mod.rs`) implementing `GateMode`, `GateOutcome`, `SecretLeakLocation`, `CanaryGateReport`, `parse_pre_push_ranges`, and `evaluate_pre_push_gate`. Evaluates `.agent-ooo/` checkpoint pairs via `canary::evaluate_canaries` (flagging incomplete pairs), streams `.agent-ooo/` text artifacts, unpushed git commit ranges (`git diff -U0` / `git show --format= -U0`, capped at 16 ranges and 50,000 diff lines), tracked diffs (`HEAD` / `--cached`), and non-binary untracked files (capped at 256 files, 1 MB max per file, skipping `.git/`, `target/`, `.agent-ooo/`), capping findings at 64 entries.
  - Added `crates/cli/src/hook.rs` (`HookAction::{Install, Uninstall, Status, Check}`, `build_pre_push_script`, `install_pre_push_hook`, `uninstall_pre_push_hook`, `status_pre_push_hook`, `run_hook_check`, `handle_hook_command`) and wired `Commands::Hook` (`display_order = 9`) in `crates/cli/src/lib.rs`, suppressing the ASCII banner on `ooo hook check` and reusing `hook::install_pre_push_hook` in `handle_init_skill_with_base` (`ooo init-skill --hooks` / `ooo init-hooks`).
  - Synced `README.md`, `llms.txt`, and `skills/agent-ooo/SKILL.md` to document `ooo hook install [--warn-only]`, `ooo hook check`, `ooo hook status`, and `ooo hook uninstall`.
  - *Self-Review Fix 1:* Replaced a literal dummy key inside the binary-skip test fixture (`blob.bin`) in `crates/core/src/discharge/gate.rs` with a dynamic byte slice constructed from `concat!` so no raw dummy secret literals appear in `git diff`.
  - *Self-Review Fix 2:* Added a leading-dash range guard in `evaluate_pre_push_gate` (`crates/core/src/discharge/gate.rs`) to skip empty or `-`-prefixed push ranges before passing arguments to `git diff` / `git show`.
  - *Context:* Authored by Antigravity Tier-2 Subagent (Gemini 3.8 Flash) and reviewed by Coordinator using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Cycle 5 (`B1`): Safe In-Place Transcript Scrubbing (`ooo detox --redact-in-place --backup`):**
  - Added `crates/core/src/detox/scrub.rs` implementing `redact_transcript_in_place`, `resolve_in_place_targets`, and `InPlaceRedactionReceipt` with constant-memory `BufReader::read_line` streaming, fast-path byte pre-filtering, structural JSON key preservation, non-JSON fallback redaction, and original line ending (`\r\n` / `\n`) retention.
  - Added `redact_secrets_only` in `crates/core/src/detox/sanitizer.rs` (sharing `mask_secrets_cow` with `sanitize_content`) to surgically mask leaked credentials in historical logs without stripping ANSI escape sequences or altering clean turns.
  - Added atomic same-directory `NamedTempFile` replacement, `symlink_metadata` directory/symlink guards, permission preservation, and optional `.bak` backup creation with pre-persist and on-disk `blake3` + byte-length verification (`persist_noclobber`).
  - Added `--redact-in-place` and `--backup` flags to `Commands::Detox` in `crates/cli/src/lib.rs` (supporting single-session sibling `transcript_full.jsonl` scrubbing and `--all` machine-wide remediation with target deduplication).
  - Added 6 unit tests across `crates/core/src/detox/scrub.rs` and `crates/cli/src/lib.rs` (`60` total workspace tests passing) and synced `README.md`, `llms.txt`, and `skills/agent-ooo/SKILL.md`.
  - *Self-Review Fix:* Moved the existing-`.bak` check from pre-scan into `create_verified_backup` (invoked only when `redactions_count > 0`) and added `test_in_place_redaction_idempotent_when_backup_already_exists_on_clean_file` so repeated `--all --redact-in-place --backup` runs cleanly skip already-scrubbed files even when a prior `.bak` exists.
  - *Context:* Authored by Antigravity Tier-2 Subagent (Gemini 3.8 Flash) and reviewed by Coordinator using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Cycle 4 (`A4`): Shareable Terminal Vitals Card (`ooo checkin --card`):**
  - Added `crates/cli/src/card.rs` implementing `render_vitals_card` and `format_compact_tokens` to print a compact, screenshot-ready 70-column Unicode box card (`╭─╮│╰╯├┤`) summarizing Session ID, Guest Condition, Loop Lock status, 10-cell Rot Index bar, Harness + Model, Activity (turns/calls/errors), Top 3 Tools, API Token Spend (`K`/`M`/`B` compact units + USD), and Secrets Masked (`scan_transcript_secrets` results).
  - Enforced plain-text width accounting (`truncate_chars` and `render_row`) prior to `colored` ANSI styling so every row aligns cleanly across terminals regardless of color support or long field truncation.
  - Added `--card` (`#[arg(long)] card: bool`) to `Commands::Checkin` in `crates/cli/src/lib.rs`, suppressing the ASCII banner and `[CHECKIN]` preamble when `--card` is passed.
  - Added 3 unit tests in `crates/cli/src/card.rs` and `test_cli_checkin_card_flag_parsing` in `crates/cli/src/lib.rs` (`53` total workspace tests passing).
  - Updated `README.md`, `llms.txt`, and `skills/agent-ooo/SKILL.md` to document `ooo checkin --card`.
  - *Self-Review Fix:* Added an explicit `is_finite()` guard before clamping `receipt.degradation_score` in `render_vitals_card` so non-finite scores default cleanly to `0.00/10.0`.
  - *Context:* Authored by Antigravity Tier-2 Subagent (Gemini 3.8 Flash) and reviewed by Coordinator using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Cycle 3 (`A3`): Fast-Path Byte Pre-Filter & Benchmark Harness (`crates/core/src/detox/sanitizer.rs`):**
  - Added `has_ansi_candidate` (`bytes.contains(&0x1B)`), `has_secret_candidate` (20-byte minimum length check and single-pass prefix byte scan for `sk-`, `AIza`, `AQ.`, `AKIA`, `gh[pousr]_`, `github_pat_`, `eyJ`, case-insensitive `bearer`, and `-----BEGIN `), and `contains_json_unicode_escape` (`\\u` window check) in `crates/core/src/detox/sanitizer.rs`.
  - Updated `sanitize_content` to skip both `SECRET_RE` and `ANSI_RE` on clean strings and selectively invoke only the matching regex using `Cow::Borrowed`, and guarded `scan_str_into` with `has_secret_candidate`.
  - Refactored `scan_transcript_secrets` to stream with a single reusable `String` buffer via `reader.read_line(&mut line)?` while preserving 1-based physical line numbers and skipping `serde_json::from_str::<Value>` DOM parsing and regex execution on lines lacking both secret candidates and `\u` JSON escapes.
  - Added `crates/core/tests/sanitizer_throughput.rs` with equivalence tests (empty/short inputs, near-misses, all secret families, ANSI CSI/OSC sequences, `\u0073` JSON-escaped secrets) and a deterministic ~3.70 MB clean / ~1.98 MB mixed throughput benchmark (`542.24 MB/s` clean scan, `946.94 MB/s` clean sanitize, `612.21 MB/s` mixed scan, `233.42 MB/s` mixed sanitize in release mode).
  - Updated `README.md` and `llms.txt` to document the fast-path byte-slice pre-filter.
  - *Self-Review Fix 1:* Replaced JSON-escaped `\u001b` in the mixed `sanitize_content` benchmark slice with raw `0x1B` bytes and expanded the 10,000-turn clean fixture to ~3.70 MB so the benchmark genuinely exercises ANSI stripping and multi-megabyte fast-path throughput.
  - *Self-Review Fix 2:* Added concise docstrings (`///`) and `#[inline]` attributes to `has_ansi_candidate`, `has_secret_candidate`, and `contains_json_unicode_escape` in `crates/core/src/detox/sanitizer.rs` during coordinator review.
  - *Context:* Authored by Antigravity Tier-2 Subagent (Gemini 3.8 Flash) and reviewed by Coordinator using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Cycle 2 (`A2`): Turnkey Agent Skill Bundle & Pre-Push Lifecycle Hook (`ooo init-skill` / `ooo init-hooks`):**
  - Added turnkey agent skill bundle `skills/agent-ooo/SKILL.md` specifying operational triggers (`checkin`, `detox`, `spa`, `discharge`, `export`), harness setup guides (Antigravity, Claude Code, Cursor, OpenAI Codex CLI, and Git lifecycle hooks), and zero em dash compliance.
  - Implemented `ooo init-skill` CLI command (with alias `ooo init-hooks`) in `crates/cli/src/lib.rs` supporting `-t` / `--target`, `-H` / `--harness`, `--hooks`, `-f` / `--force`, and `--stdout` flags. Automatically embeds the skill template, generates `.git/hooks/pre-push` with a fast `ooo discharge` canary gate, and marks hook scripts executable on Unix.
  - Enhanced model extraction in `crates/core/src/checkin/parser.rs` to detect models across `value.get("model")`, `model_id`, `model_name`, `message.get("model")`, and content markers (`Model: "flash"`, `Model: "flash_lite"`, `Model: "pro"`, `Tier-2 (`flash`)`, `gemini-2.5-flash`).
  - Refined `estimate_api_cost_usd` in `crates/core/src/checkin/triage.rs` to handle tiered token pricing across flash/lite/haiku/mini (`$0.30` / `$0.03` / `$2.50`), opus/o1 (`$15.00` / `$1.50` / `$75.00`), sonnet/argon/gpt-4o (`$3.00` / `$0.30` / `$15.00`), and default pro/carbon (`$2.50` / `$0.25` / `$15.00`).
  - Added unit tests for `InitSkill` argument parsing, file generation, `--force` overwrites, git hook permissions, subagent transcript model detection, and pricing tiers (`45` total workspace tests).
  - *Self-Review Fix 1:* Replaced dynamic executable alias inside the git pre-push template with concrete `ooo discharge` and `agent-ooo discharge` invocations so the generated script strictly adheres to the published skill documentation and passes content assertion tests.
  - *Self-Review Fix 2:* Guarded subagent model marker extraction in `crates/core/src/checkin/parser.rs` with `is_valid_model_name` and `!is_antigravity_tool_result` to prevent code diffs and punctuation in tool result turns from corrupting transcript model identification.
  - *Context:* Authored by Antigravity Tier-2 Subagent (Gemini 3.8 Flash) and reviewed by Coordinator (Gemini 4 Carbon) using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Cycle 1 (`A1`): Machine-Wide Secret Posture (`ooo detox --all`) & 7-Harness Auto-Discovery:**
  - Added bounded recursive session discovery (`scan_jsonl_tree`, depth-limited and entry-capped) in `crates/core/src/checkin/discovery.rs` to auto-discover **Codex CLI** (`~/.codex/sessions`), **Windsurf** (`~/.codeium/windsurf`, `~/.windsurf/sessions`), and **Cline / Roo Code** (`~/.cline`, `~/.roo-code`) alongside Antigravity, Antigravity IDE, Claude Code, and Cursor (`7` harnesses total).
  - Added `-a` / `--all` to `ooo detox` in `crates/cli/src/lib.rs` (bringing 1:1 CLI parity with MCP `all_sessions: true`) to stream-scan every discovered session across all local harnesses and print a masked credential posture summary.
  - Added 4 unit tests covering nested directory discovery, depth-cap enforcement, harness classification, and `--all` CLI flag parsing (`42` total workspace tests).
  - *Self-Review Fix:* Added per-directory entry caps (`512`) and skipped `node_modules`/`.git`/`target` directories inside `scan_jsonl_tree` to maintain bounded memory and `<5ms` discovery latency, and made `ooo detox --all` resilient to transient unreadable files instead of aborting mid-scan.
  - *Context:* Authored by Antigravity Tier-2 Subagent (Gemini 3.8 Flash) and reviewed by Coordinator (Gemini 4 Carbon) using Rust 1.99.1 and Cap'n Proto 1.5.0.
- **Founding Vision Alignment: Persisted Cap'n Proto Arena, Durable User Anchors, Intent-Signature Triage, Register Drift Canary, & CAS Recall:**
  - Extended `schema/session_checkpoint.capnp` with `ArenaTurn` (`gen0Arena`, `transientArena`) and updated `reset::serializer::write_checkpoints` (`crates/core/src/reset/serializer.rs`) to persist sanitized, loop-severed Generation 0 and transient conversation turns (bounded to 16 Gen 0 turns, 256 transient turns, and 32 KB per turn) into `.agent-ooo/checkpoint.capnp` and `.agent-ooo/checkpoint.json`.
  - Wired `ExportFormat::HandoffMarkdown` in `crates/core/src/detox/exporter.rs` to render directly from the persisted Cap'n Proto arena when `.agent-ooo/checkpoint.capnp` matches the target session, ensuring severed error loops stay pruned upon session resumption, and updated `discharge::report::generate_spa_report` (`crates/core/src/discharge/report.rs`) to report realized end-to-end state footprint reduction.
  - Added a bounded 32-turn `user_anchors` ledger in `MemoryArena` (`crates/core/src/detox/arena.rs`) populated during `detox::pipeline::execute_detox` and scoped `reset::analyzer::extract_constraints_from_text` to `<USER_REQUEST>` payloads so durable user constraints from turn 1 survive transient ring eviction without matching harness metadata boilerplate.
  - Upgraded `checkin::parser::parse_transcript` and `checkin::triage::calculate_triage` to hash `(tool_name, args)` FNV-1a intent signatures (`tool_call_signatures`) so legitimate parallel file reads/edits no longer trigger false `loop_lock_detected` streaks, and added Antigravity `<USER_SETTINGS_CHANGE>` model selection extraction.
  - Hydrated `EnvironmentRegister.validIf` (`HEAD == <commit>`) and `sourceEventId` in `reset::serializer::write_checkpoints`, and added a 5th zero-LLM canary (`environment_register_freshness`) in `crates/core/src/discharge/canary.rs` that warns when live workspace git HEAD or branch has drifted from the checkpointed state.
  - Implemented `detox::spillover::recall_spilled_payload` (`crates/core/src/detox/spillover.rs`) with strict hex-handle validation and bounded 32 KB slice seeking, exposed via `ooo discharge --recall <blake3>` and the `agent-ooo-mcp` `discharge` tool (`recall_handle`, `offset`, `max_bytes`), plus set `default-run = "ooo"` in `crates/cli/Cargo.toml`.
  - *Self-Review Fix:* Sanitized dynamic `model_id`, `agent_harness`, `session_id`, and `tool_name` strings in `discharge::report::generate_spa_report` to strip parenthetical annotations (such as `(Medium)`) so `spa_report.md` preserves its strict zero-parentheses invariant on all harnesses, and rejected non-hex path characters in `sanitize_handle_filename` to prevent directory traversal on CAS recall.
  - *Context:* Authored by Antigravity (Gemini 4 Carbon) using Rust 1.99.1 and Cap'n Proto 1.5.0.

## [0.2.0] - 2026-11-06

### Added
- **Empirical Code & Product Audit Polish (Antigravity `GENERIC` Tool Correlation, Non-Interactive PTY Guard, & Full-Transcript Resolution):**
  - Updated `checkin::parser::parse_transcript`, `reset::analyzer::analyze_and_sever_arena`, and `detox::exporter::export_transcript` to correlate Antigravity `type: "PLANNER_RESPONSE"` `tool_calls` with subsequent `type: "GENERIC"` tool output steps (`status: "DONE" | "ERROR" | "RUNNING"`), accurately detecting tool failures (`status == "ERROR"` or `"Command exited with code N"`) and mapping `GENERIC` outputs to `"tool"` turns rather than assistant turns.
  - Added `is_interactive_terminal` and `resolve_transcript_path_with_mode` in `crates/core/src/checkin/discovery.rs` plus a global `-y` / `--latest` CLI flag in `crates/cli/src/lib.rs` so PTY-backed agent shells (`ANTIGRAVITY_AGENT`, `CLAUDECODE`, `CI`, `TERM=dumb`, `AGENT_OOO_NON_INTERACTIVE`) never hang on `dialoguer::Select`, and enriched the interactive picker with short session IDs (`[a60bb690-2c5]`) and file sizes (`KB` / `MB`).
  - Wired `discovery::resolve_scan_target` at the entry of `checkin::parser::parse_transcript` and `detox::pipeline::execute_detox` so both CLI and MCP automatically read untruncated sibling `transcript_full.jsonl` files when present, added `tool_calls` argument byte accounting to `parse_transcript`, extended `execute_detox` to sanitize `thinking` and `tool_calls` arguments, and added Google OAuth/Vertex `AQ.` token detection to `SECRET_RE` in `crates/core/src/detox/sanitizer.rs`.
  - Replaced naive period splitting in `reset::analyzer::extract_constraints_from_text` with `split_sentences_preserving_tokens` so file paths (`analyzer.rs`, `spa_report.md`) and version strings (`v0.2`) remain intact inside extracted `NegativeConstraint` directives, and excluded `SYSTEM_MESSAGE` and `CHECKPOINT` metadata turns from Generation 0 system prompt pinning in `detox::pipeline` and `detox::exporter`.
  - Upgraded `Commands::Detox` in `crates/cli/src/lib.rs` to run `scan_transcript_secrets` and print masked credential findings (`prefix...suffix`, line numbers, and occurrence counts) on the terminal, removed the no-op `Commands::Watch` stub, removed dead helper `compute_structural_hash` in `loop_normalizer.rs`, and pruned unused workspace dependencies (`memmap2`, `tracing`, `tracing-subscriber`).
  - *Self-Review Fix:* Flushed unpopped `pending_calls` in `reset::analyzer::analyze_and_sever_arena` across user/assistant turn boundaries and at end-of-stream so cancelled, backgrounded, or in-flight trailing tool calls do not misalign FIFO correlation or trigger false `unverified_late_edit` canary warnings.
  - *Context:* Authored by Antigravity (Gemini 4 Argon Medium) using Rust 1.99.1 and Cap'n Proto 1.5.0.
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
