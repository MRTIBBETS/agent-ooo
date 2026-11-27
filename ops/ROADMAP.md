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

- [x] **B1. Safe In-Place Transcript Scrubbing (`ooo detox --redact-in-place --backup`):**
  - Allow developers to purge leaked credentials from historical `.jsonl` logs on disk using atomic temp-file replacement + `.bak` backup verification.
- [x] **B2. Git Pre-Push Canary Gate (`ooo hook install`):**
  - Optional 5 ms `.git/hooks/pre-push` gate that warns or blocks pushes when `Discharge` reports `FAIL` canaries or unredacted workspace secrets.
- [x] **B3. Schema Drift Doctor (`ooo checkin --doctor`):**
  - Diagnose unrecognized JSONL schemas, missing token/tool telemetry, or zero-tool-call anomalies across all 7 discovered harnesses (CLI `--doctor` + MCP `doctor: true`) so upstream vendor format changes are caught immediately.

## Phase C: Distribution, CI gate, & fleet observability (`v0.3.x` -> `v0.4.0`)

- [ ] **C1. GitHub Actions CI Workflow & Reusable PR Action (`.github/workflows/ci.yml` + `action.yml`) (`Cycle 8`):**
  - Add automated PR and `main` push CI (`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, em-dash gate) plus a composite `action.yml` (`ooo hook check`) so external repositories can enforce agent canary and secret gates in CI with two lines of YAML.
- [ ] **C2. Machine-Wide Fleet Vitals & Token FinOps Rollup (`ooo checkin --all [--json]`) (`Cycle 9`):**
  - Aggregate session counts, Guest Condition distribution, average/max Rot Index, model tier mix, and cumulative API token + estimated USD spend across all 7 harnesses on the host in `< 100 ms` (with CLI `--all` / `--json` and MCP `all_sessions: true` parity).
- [ ] **C3. MCP Registry Manifest & Crates.io Package Readiness (`server.json` + Workspace Metadata) (`Cycle 10`):**
  - Author an official MCP Registry `server.json` manifest, populate `repository`, `homepage`, `readme`, `keywords`, and `categories` across `crates/core`, `crates/cli`, and `crates/mcp`, and validate `cargo test --workspace` + packaging metadata for registry distribution.
- [ ] **C4. Checksummed Curl Installer (`install.sh`) & Linux ARM64 Release Target (`Cycle 11`):**
  - Ship a POSIX `install.sh` (`shasum -a 256` / `sha256sum` verified installer for `agent-ooo`, `ooo`, and `agent-ooo-mcp`) and extend `.github/workflows/release.yml` + `Formula/agent-ooo.rb` to build `aarch64-unknown-linux-gnu` alongside `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`.
- [ ] **C5. Release Cut `v0.4.0` (`Cycle 12`):**
  - Bump workspace version to `0.4.0`, roll `CHANGELOG.md` `[Unreleased]` into `[0.4.0]`, verify full release test and throughput benchmarks, push `chore(release): v0.4.0` to `origin/main`, and publish tag `refs/tags/v0.4.0` via `gh api` so GitHub Actions builds release artifacts and updates Homebrew automatically.
