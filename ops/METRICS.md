# Agent OOO Operational & Growth Ledger

Append one row per completed autonomous or interactive sprint cycle. Measure performance with `target/release/ooo` and measure token/credit spend by running `ooo -y checkin` on the cycle's session transcript.

## 1. Sprint & token cost ledger

| Date | Cycle | Primary Model + Subagents | Shipped Deliverable | Tests | `ooo spa` Latency | Peak RSS | Uncached In Tokens | Cache Read Tokens | Output Tokens | Est. Spend (USD) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 2026-11-13 | `Cycle 0` (Baseline + Vision) | Carbon (multi-week session `a60bb690`) | Persisted Capnp Arena, User Anchors, Intent Streaks, Drift Canary, CAS Recall, API Spend Telemetry | `38` | `47.01 ms` | `13.86 MB` | `3,699,514` (cumulative since Oct 19) | `27,183,880` | `271,971` | `$20.12` (cumulative v0.1.7->v0.2.1) |

## 2. Distribution & adoption snapshots

| Date | Latest Tag | Total Release Downloads | `v0.2.0` Downloads | GitHub Stars | Forks | Open Issues | Harnesses Supported |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 2026-11-13 | `v0.2.0` | `20` | `4` | `1` | `0` | `0` | `4` (Antigravity, AGY IDE, Claude Code, Cursor) |
