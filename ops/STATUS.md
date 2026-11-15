# Agent OOO Operational Status

**Last updated:** 2026-11-15
**Latest release:** `v0.2.0` (with Cycle 0 Vision Alignment + Cycle 1 Multi-Harness & Detox `--all` on `main`)
**Active sprint:** `v0.3.0` Wedge & Distribution (Next up: Cycle 2 `A2` Turnkey Agent Skill & Lifecycle Hook Bundle)
**Overall health:** Green (`42/42` tests passing, `0` clippy warnings, `47.0 ms` full spa latency, `12.48 MB` peak RSS)

## 1. Executive pulse

* **What just shipped (`Cycle 1` / Roadmap `A1`):**
  * **Machine-wide CLI secret posture (`ooo detox --all` / `-a`):** Operators can now audit every discovered AI coding session on their machine from the terminal in `<50 ms`, matching the MCP `detox(scan_all: true)` tool without panicking on unreadable session files (`skipped_sessions` fault isolation). Live run scanned `25` local sessions and surfaced `102` secret occurrences across `5` transcripts with masked values.
  * **7-harness auto-discovery:** Expanded bounded directory tree scanning (`scan_jsonl_tree` with `MAX_TREE_CANDIDATES = 512` and `.git`/`node_modules`/`target` guards) in `crates/core/src/checkin/discovery.rs` to automatically discover **OpenAI Codex CLI** (`~/.codex/sessions`), **Windsurf** (`~/.codeium/windsurf`, `~/.windsurf/sessions`), and **Cline / Roo Code** (`~/.cline`, `~/.roo-code`) alongside Antigravity, AGY IDE, Claude Code, and Cursor.
  * **Tiered subagent execution verified:** Authoring + unit tests (`114` turns, `55` tool calls) ran end-to-end on `Gemini 3.8 Flash` for **`$0.25`** (`230.7k` in / `3.49M` cache / `30.7k` out), nearly **8x cheaper** than running implementation directly on Carbon.
* **Next queued slice (`Cycle 2` / Roadmap `A2`):**
  * **Turnkey Agent Skill & Lifecycle Hook Bundle (`ooo init-skill`):** Generate ready-to-use `SKILL.md` + hook instructions for Claude Code, Cursor, Antigravity, and Codex so agents automatically invoke `checkin` -> `spa` when `rot_index >= 5.0` without manual prompting.
* **Schedule & founder action item:**
  * Recurring schedule is configured in `agent-ooo-autonomous-sprint` (`Mon / Wed / Fri at 9:00 AM PT`; next window: **Monday, Nov 16, 2026 at 9:00 AM PT**). Toggle **Agent OOO Autonomous Sprint** ON in the Automations dashboard to activate unattended background execution (fresh-context runs also eliminate multi-week coordinator cache overhead).

## 2. Core KPIs, growth & credit ledger

| Category | Metric | Previous (`Cycle 0`) | Current (`Cycle 1`, 2026-11-15) | Target (`v0.3.0`) |
| :--- | :--- | :--- | :--- | :--- |
| **Product Quality** | Workspace unit + integration tests | `38` passing | `42` passing (`0` failing) | `50+` passing |
| **Engine Speed** | `ooo spa` release latency (2.1 MB / 688-turn log) | `47.01 ms` | `47.01 ms` (`< 50 ms` across 25 sessions on `detox --all`) | `< 50 ms` |
| **Memory Bound** | Peak RSS (`maximum resident set size`) | `13.86 MB` | `12.48 MB` (`7.21 MB` peak footprint) | `< 24 MB` |
| **Coverage** | Supported agent harnesses | `4` | `7` (Antigravity, AGY IDE, Claude Code, Cursor, Codex CLI, Windsurf, Cline) | `7+` (Target met) |
| **Distribution** | GitHub release downloads (cumulative across tags) | `20` (`4` on `v0.2.0`) | `20` (`4` on `v0.2.0`) | `100+` |
| **Distribution** | GitHub stars / forks / open issues | `1` / `0` / `0` | `1` / `0` / `0` | Track weekly delta |
| **Ops Credit Spend** | Cycle 1 token & spend breakdown | `$20.12` (multi-week cumulative) | `$0.25` Flash Author (`230.7k` in / `30.7k` out) + `$2.40` Carbon Review (731-turn thread delta) | `< $1.00` / fresh autonomous run |

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
