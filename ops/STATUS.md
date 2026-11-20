# Agent OOO Operational Status

**Last updated:** 2026-11-20
**Latest release:** `v0.2.0` (with Cycle 0 Vision Alignment, Cycle 1 Multi-Harness & Detox `--all`, Cycle 2 Turnkey Skill & Lifecycle Hooks, Cycle 3 Fast-Path Byte Pre-Filter, and Cycle 4 Shareable Vitals Card on `main`)
**Active sprint:** `v0.3.0` Phase A Complete -> `v0.4.0` Phase B Active Remediation (Next up: Cycle 5 `B1` Safe In-Place Transcript Scrubbing `ooo detox --redact-in-place --backup`)
**Overall health:** Green (`53/53` tests passing, `0` clippy warnings, `39.17 ms` full spa latency, `13.09 MB` peak RSS)

## 1. Executive pulse

* **What just shipped (`Cycle 4` / Roadmap `A4`):**
  * **Shareable Terminal Vitals Card (`crates/cli/src/card.rs`, `crates/cli/src/lib.rs`):** Added `ooo checkin --card` to render a compact, screenshot-ready 70-column ANSI terminal box card (`╭─╮│╰╯├┤`) summarizing Session ID, Guest Condition + Loop Lock status, 10-cell Rot Index bar, Harness + Model, Activity, Top 3 Tools, API Token Spend (`K`/`M`/`B` compact units + USD), and Secrets Masked (`scan_transcript_secrets`). Suppresses the ASCII banner and `[CHECKIN]` preamble when `--card` is passed, and aligns all borders deterministically before applying `colored` ANSI styles.
  * **Test Coverage & Documentation (`53/53` Passing):** Added 3 unit tests in `crates/cli/src/card.rs` (ANSI-stripped width alignment, compact token scaling, truncation of long fields, `Fatigued` / leaked secrets edge case, zero em dashes) plus `test_cli_checkin_card_flag_parsing` in `crates/cli/src/lib.rs`. Synced `README.md`, `llms.txt`, and `skills/agent-ooo/SKILL.md`.
  * **Fresh Scheduled Autonomous Execution Under Budget:** Cycle 4 ran in a fresh unattended session (`95d386a8`) delegating implementation, tests, and docs sync to Flash (`103bff8e`). Reported `ooo checkin` ledger total was **`$0.7353`** (`$0.6727` subagent at fallback rate + `$0.0626` coordinator; `$0.1578` at Flash rates), within the `< $1.00` hard guardrail.
* **Next queued slice (`Cycle 5` / Roadmap `B1`):**
  * **Safe In-Place Transcript Scrubbing (`ooo detox --redact-in-place --backup`):** Allow developers to purge leaked credentials from historical `.jsonl` logs on disk using atomic temp-file replacement + `.bak` backup verification.
* **Schedule & founder action item:**
  * Recurring schedule is configured in `agent-ooo-autonomous-sprint` (`Mon / Wed / Fri at 9:00 AM PT`; next window: **Monday, Nov 23, 2026 at 9:00 AM PT**). Phase A (`A0` through `A4`) is now `100%` complete on `main` and ready for a `v0.3.0` release tag whenever desired.

## 2. Core KPIs, growth & credit ledger

| Category | Metric | Previous (`Cycle 3`) | Current (`Cycle 4`, 2026-11-20) | Target (`v0.3.0`) |
| :--- | :--- | :--- | :--- | :--- |
| **Product Quality** | Workspace unit + integration tests | `49` passing | `53` passing (`0` failing) | `50+` passing (Met) |
| **Engine Speed** | `ooo spa` release latency (current session) | `38.88 ms` | `39.17 ms` | `< 50 ms` (Met) |
| **Memory Bound** | Peak RSS (`maximum resident set size`) | `13.61 MB` | `13.09 MB` (`7.17 MB` peak footprint) | `< 24 MB` (Met) |
| **Coverage** | Supported agent harnesses | `7` + Skill, Hooks & Pre-Filter | `7` + Skill, Hooks, Pre-Filter & Vitals Card | `7+` (Met) |
| **Distribution** | GitHub release downloads (cumulative across tags) | `20` (`4` on `v0.2.0`) | `20` (`4` on `v0.2.0`) | `100+` |
| **Distribution** | GitHub stars / forks / open issues | `1` / `0` / `0` | `1` / `0` / `0` | Track weekly delta |
| **Ops Credit Spend** | Fresh autonomous cycle token & spend ledger | `$0.8428` (`$0.1983` Flash eq.) | `$0.6727` Flash (`83.1k` in / `20.7k` out, fallback rate) + `$0.0626` Coordinator (`79.8k` in / `7.4k` out) = **`$0.7353`** (`$0.1578` Flash eq.) | `< $1.00` / fresh autonomous run (Met) |

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
