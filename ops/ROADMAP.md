# Agent OOO Product & Engineering Roadmap

All items are ranked by **user value x distribution leverage / complexity**. Autonomous cycles pull the top `[ ]` item, implement with tests, run the [`AGENTS.md`](../AGENTS.md) Pre-Commit Self-Review, dogfood `ooo -y spa`, commit to `main`, and mark `[x]`.

## Phase A: Wedge sharpening & harness reach (`v0.2.x` -> `v0.3.0`)

- [x] **A0. Founding Vision Alignment & Native Token Spend Telemetry (`Cycle 0`):**
  - Persist `gen0Arena` + `transientArena` into `checkpoint.capnp`, durable 32-turn user anchors, FNV-1a `(tool, args)` loop triage, `environment_register_freshness` git drift canary, `ooo discharge --recall <blake3>`, and API token/spend ledger in `AgentVitals`.
- [x] **A1. Machine-Wide CLI Secret Audit (`ooo detox --all`) & Multi-Harness Discovery (`Cycle 1`):**
  - Bring MCP's `all_sessions: true` machine-wide secret scan to the CLI (`ooo detox --all`) with a crisp summary table across every session on disk.
  - Expand `checkin::discovery` to auto-discover **OpenAI Codex CLI** (`~/.codex/sessions`), **Windsurf** (`~/.codeium/windsurf`), and **Cline / Roo Code** transcripts with unit tests.
- [x] **A2. Turnkey Agent Skill & Lifecycle Hook Bundle (`Cycle 2`):**
  - Author `skills/agent-ooo/SKILL.md` and `ooo init-skill` (with alias `ooo init-hooks`) so agents know *when* to call `checkin`, `detox`, `spa`, `discharge`, and `export` automatically before PRs and handoffs.
  - Subagent model auto-detection across non-standard logs and tiered API cost estimation (`$0.30` flash/lite/mini, `$15.00` opus/o1, `$3.00` sonnet/argon/gpt-4o, `$2.50` carbon/pro).
  - Pre-push git lifecycle hook installation (`.git/hooks/pre-push`) with fast `ooo discharge` clinical and behavioral canary gate.
- [x] **A3. Fast-Path Byte Pre-Filter & Benchmark Harness (`Cycle 3`):**
  - Add fast byte-slice pre-checks in `sanitizer.rs` to skip regex matching on clean ASCII lines without `\x1b` or secret prefixes, plus a deterministic `cargo test --release` throughput benchmark fixture in `crates/core/tests/`.
- [x] **A4. Shareable Terminal Vitals Card (`ooo checkin --card`) (`Cycle 4`):**
  - Render a compact, screenshot-ready ANSI terminal card summarizing Guest Condition, Rot Index, Harness + Model, Top Tools, API Token Spend, and Secrets Masked.

## Phase B: Active remediation & CI governance (`v0.4.0`)

- [ ] **B1. Safe In-Place Transcript Scrubbing (`ooo detox --redact-in-place --backup`):**
  - Allow developers to purge leaked credentials from historical `.jsonl` logs on disk using atomic temp-file replacement + `.bak` backup verification.
- [ ] **B2. Git Pre-Push Canary Gate (`ooo hook install`):**
  - Optional 5 ms `.git/hooks/pre-push` gate that warns or blocks pushes when `Discharge` reports `FAIL` canaries or unredacted workspace secrets.
- [ ] **B3. Schema Drift Doctor (`ooo checkin --doctor`):**
  - Diagnose unrecognized JSONL schemas or zero-tool-call anomalies across discovered harnesses so vendor format changes are caught immediately.
