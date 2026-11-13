# Agent OOO Operational Status

**Last updated:** 2026-11-13
**Latest release:** `v0.2.0` (with Founding Vision Alignment + Cycle 0 Telemetry on `main`)
**Active sprint:** `v0.3.0` Wedge & Distribution (Cycle 1: `ooo detox --all` CLI parity + multi-harness discovery)
**Overall health:** Green (`38/38` tests passing, `0` clippy warnings, `47.0 ms` full spa latency, `13.9 MB` peak RSS)

## 1. Executive pulse

* **What just shipped (`Cycle 0`):**
  * Persisted sanitized, loop-severed `gen0Arena` and `transientArena` turns directly into Cap'n Proto (`checkpoint.capnp`) and wired `ExportFormat::HandoffMarkdown` to resume from the compacted checkpoint (`91.29%` realized footprint reduction).
  * Added durable 32-turn `user_anchors` so early user constraints survive 1,000+ turn sessions (`0` -> `11` pinned constraints on live dogfooding).
  * Upgraded `checkin` loop detection to FNV-1a `(tool_name, args)` signatures, added Antigravity `<USER_SETTINGS_CHANGE>` model detection, added the 5th `environment_register_freshness` git drift canary, and closed cold-storage CAS recall (`ooo discharge --recall <blake3>`).
  * Added native **API token & credit spend telemetry** (`api_input_tokens`, `api_cache_read_tokens`, `api_output_tokens`, `estimated_session_cost_usd`) directly into `ooo checkin`, `agent-ooo-mcp`, and `spa_report.md` so every operational cycle measures its own token burn natively.
* **Next queued slice (`Cycle 1`):**
  * CLI machine-wide secret posture (`ooo detox --all`) + expanded harness auto-discovery for **OpenAI Codex CLI (`~/.codex`), Windsurf (`~/.codeium/windsurf`), and Cline/Roo Code**, executed via tiered `flash` Author + `Carbon` Verifier subagents.
* **Blockers / founder input needed:** None (enable the **Agent OOO Autonomous Sprint** automation in the Automations dashboard if you want unattended Mon/Wed/Fri background sprints in addition to this active loop).

## 2. Core KPIs, growth & credit ledger

| Category | Metric | Current Baseline (2026-11-13) | Target (`v0.3.0`) |
| :--- | :--- | :--- | :--- |
| **Product Quality** | Workspace unit + integration tests | `38` passing (`0` failing) | `50+` passing |
| **Engine Speed** | `ooo spa` release latency (2.1 MB / 688-turn log) | `47.01 ms` (`~44.7 MB/s` full 4-step pipeline) | `< 50 ms` |
| **Memory Bound** | Peak RSS (`maximum resident set size`) | `13.86 MB` (`8.30 MB` footprint) | `< 24 MB` |
| **Coverage** | Supported agent harnesses | `4` (Antigravity, AGY IDE, Claude Code, Cursor) | `7+` (+ Codex, Windsurf, Cline) |
| **Distribution** | GitHub release downloads (cumulative across tags) | `20` (`4` on `v0.2.0`) | `100+` |
| **Distribution** | GitHub stars / forks / open issues | `1` star / `0` forks / `0` open issues | Track weekly delta |
| **Ops Credit Spend** | Cumulative multi-day build session (`a60bb690`, 688 turns) | `3.70M` in / `27.18M` cache / `272k` out (`~$20.12` est.) | `< $1.50` per incremental cycle via Flash routing |

## 3. Model routing & cost governance charter

Authorized by founder on 2026-11-13 to maximize impact per token:

1. **Tier 1: Coordinator & Architecture (`Gemini 4 Carbon` / `pro`):**
   * Reserved strictly for roadmap prioritization, Cap'n Proto schema changes, subtle Rust ownership/streaming invariants, and final adversarial verification before pushing to `main`.
2. **Tier 2: Implementation & Test Workhorse (`Gemini 3.8 Flash` / `flash`):**
   * Default subagent model (`invoke_subagent(..., Model: "flash")`) for CLI flag wiring, multi-harness path discovery, fixture synthesis, unit test authoring, and documentation sync (`README.md`, `llms.txt`, `CHANGELOG.md`). Costs ~8x less per token than Tier 1.
3. **Tier 3: High-Volume Inspection & Formatting (`flash_lite`):**
   * Used for bulk log scanning, release checksum checks, and mechanical text verification.
4. **Hard Per-Cycle Budget Guardrail:**
   * Every autonomous cycle starts fresh (avoiding 27M-token multi-week cache accumulation), executes **one bounded roadmap item**, logs exact API token counts via `ooo -y checkin` into `ops/runs/YYYY-MM-DD.md`, and targets **`< 400k` uncached input tokens (`< $1.00` estimated spend)** per cycle.
