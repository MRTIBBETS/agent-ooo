# Agent OOO Operational Status

**Last updated:** 2026-11-16
**Latest release:** `v0.2.0` (with Cycle 0 Vision Alignment, Cycle 1 Multi-Harness & Detox `--all`, and Cycle 2 Turnkey Skill & Lifecycle Hooks on `main`)
**Active sprint:** `v0.3.0` Wedge & Distribution (Next up: Cycle 3 `A3` Fast-Path Byte Pre-Filter & Benchmark Harness)
**Overall health:** Green (`45/45` tests passing, `0` clippy warnings, `28.72 ms` full spa latency, `12.25 MB` peak RSS)

## 1. Executive pulse

* **What just shipped (`Cycle 2` / Roadmap `A2`):**
  * **Turnkey Agent Skill Bundle (`skills/agent-ooo/SKILL.md`):** Published complete operational skill specification detailing when agents must trigger `checkin` (context rot, cognitive loops, API spend), `detox` (secret hygiene, ANSI stripping, 512 KB CAS spillover, machine-wide posture scans), `spa` (full 4-step reset when Rot Index >= 5.0 or 3x error loop lock), `discharge` (clinical & behavioral canary exit exam before commit or push, bounded CAS payload recall), and `export` (secret-scrubbed JSONL, normalized, or loop-severed handoff markdown).
  * **CLI Skill & Lifecycle Hook Generator (`ooo init-skill` / `ooo init-hooks`):** Added display order 8 CLI command with `-t` / `--target`, `-H` / `--harness`, `--hooks`, `-f` / `--force`, and `--stdout` flags. Automatically embeds the skill template, writes `.git/hooks/pre-push` with executable permissions on Unix, and wires the fast `ooo discharge` canary gate so broken code or unverified late edits cannot be pushed to remotes.
  * **Subagent Model Auto-Detection & 4-Tier Token Pricing:** Hardened transcript parser to auto-detect subagent models from `model`/`model_id`/`model_name` keys and prompt markers (`Model: "flash"`, `Model: "pro"`, `Tier-2 (`flash`)`). Enhanced `estimate_api_cost_usd` with accurate reference rates across flash/lite/haiku/mini (`$0.30` in / `$0.03` cache_read / `$2.50` out), opus/o1 (`$15.00` / `$1.50` / `$75.00`), sonnet/argon/gpt-4o (`$3.00` / `$0.30` / `$15.00`), and carbon/pro (`$2.50` / `$0.25` / `$15.00`).
  * **Fresh Scheduled Autonomous Execution Under Budget:** Cycle 2 ran in a fresh unattended session. Total sprint cost was **`$0.64`** (`$0.40` Flash Author across 154 turns + `$0.24` Carbon Review across 126 turns), well below the `< $1.00` hard guardrail.
* **Next queued slice (`Cycle 3` / Roadmap `A3`):**
  * **Fast-Path Byte Pre-Filter & Benchmark Harness:** Add fast byte-slice pre-checks in `sanitizer.rs` to skip regex matching on clean ASCII lines without `\x1b` or secret prefixes, plus a deterministic `cargo test --release` throughput benchmark fixture in `crates/core/tests/`.
* **Schedule & founder action item:**
  * Recurring schedule is configured in `agent-ooo-autonomous-sprint` (`Mon / Wed / Fri at 9:00 AM PT`; next window: **Wednesday, Nov 18, 2026 at 9:00 AM PT**).

## 2. Core KPIs, growth & credit ledger

| Category | Metric | Previous (`Cycle 1`) | Current (`Cycle 2`, 2026-11-16) | Target (`v0.3.0`) |
| :--- | :--- | :--- | :--- | :--- |
| **Product Quality** | Workspace unit + integration tests | `42` passing | `45` passing (`0` failing) | `50+` passing |
| **Engine Speed** | `ooo spa` release latency (2.1 MB / 688-turn log) | `47.01 ms` | `28.72 ms` (sub-30ms full retreat) | `< 50 ms` |
| **Memory Bound** | Peak RSS (`maximum resident set size`) | `12.48 MB` | `12.25 MB` (`6.59 MB` peak footprint) | `< 24 MB` |
| **Coverage** | Supported agent harnesses | `7` | `7` + Turnkey Skill Bundle & Git Pre-Push Hook | `7+` (Target met) |
| **Distribution** | GitHub release downloads (cumulative across tags) | `20` (`4` on `v0.2.0`) | `20` (`4` on `v0.2.0`) | `100+` |
| **Distribution** | GitHub stars / forks / open issues | `1` / `0` / `0` | `1` / `0` / `0` | Track weekly delta |
| **Ops Credit Spend** | Fresh autonomous cycle token & spend ledger | `$2.65` (historical coordinator delta) | `$0.40` Flash Author (`236.4k` in / `53.8k` out) + `$0.24` Carbon Review (`195.2k` in / `28.5k` out) = **`$0.64`** | `< $1.00` / fresh autonomous run |

## 3. Model routing & cost governance charter

Authorized by founder on 2026-11-13 to maximize impact per token:

1. **Tier 1: Coordinator & Architecture (`Gemini 4 Carbon` / `pro`):**
   * Reserved strictly for roadmap prioritization, Cap'n Proto schema changes, subtle Rust ownership/streaming invariants, and final adversarial verification before pushing to `main`.
2. **Tier 2: Implementation & Test Workhorse (`Gemini 3.8 Flash` / `flash`):**
   * Default subagent model (`invoke_subagent(..., Model: "flash")`) for CLI flag wiring, multi-harness path discovery, fixture synthesis, unit test authoring, and documentation sync (`README.md`, `llms.txt`, `CHANGELOG.md`). Verified in Cycle 1 at `$0.25` for a full multi-crate feature + 4 unit tests.
3. **Tier 3: High-Volume Inspection & Formatting (`flash_lite`):**
   * Used for bulk log scanning, release checksum checks, and mechanical text verification.
4. **Hard Per-Cycle Budget Guardrail:**
   * Every unattended scheduled cycle starts in a **fresh conversation** (eliminating the `30M`-token historical cache from `a60bb690`), executes **one bounded roadmap item**, logs exact API token counts via `ooo -y checkin` into `ops/runs/YYYY-MM-DD.md`, and targets **`< 400k` uncached input tokens (`< $1.00` estimated spend)** per cycle.
