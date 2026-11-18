# Agent OOO Operational & Growth Ledger

Append one row per completed autonomous or interactive sprint cycle. Measure performance with `target/release/ooo` and measure token/credit spend by running `ooo -y checkin` on the cycle's session transcript.

## 1. Sprint & token cost ledger

| Date | Cycle | Primary Model + Subagents | Shipped Deliverable | Tests | `ooo spa` Latency | Peak RSS | Uncached In Tokens | Cache Read Tokens | Output Tokens | Est. Spend (USD) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 2026-11-13 | `Cycle 0` (Baseline + Vision) | Carbon (multi-week session `a60bb690`) | Persisted Capnp Arena, User Anchors, Intent Streaks, Drift Canary, CAS Recall, API Spend Telemetry | `38` | `47.01 ms` | `13.86 MB` | `3,699,514` (cumulative since Oct 19) | `27,183,880` | `271,971` | `$20.12` (cumulative v0.1.7->v0.2.1) |
| 2026-11-15 | `Cycle 1` (`A1`: Detox `--all` + 7 Harnesses) | Flash Author (`63273270`) + Carbon Review (`a60bb690` delta) | `ooo detox --all` CLI posture scan + Codex CLI, Windsurf, Cline auto-discovery | `42` | `47.01 ms` | `12.48 MB` | `230,743` (Flash) + `549,536` (Carbon) | `3,494,850` (Flash) + `3,123,390` (Carbon) | `30,698` (Flash) + `16,173` (Carbon) | `$0.25` Flash + `$2.40` Carbon (`$2.65` total) |
| 2026-11-16 | `Cycle 2` (`A2`: Turnkey Skill & Lifecycle Hooks) | Flash Author (`c74ec949`) + Carbon Review (`f3101127`) | `ooo init-skill` / `ooo init-hooks`, `SKILL.md`, pre-push git hook, subagent model auto-detection & 4-tier pricing | `45` | `28.72 ms` | `12.25 MB` | `236,441` (Flash) + `195,260` (Carbon) | `6,392,991` (Flash) + `3,701,213` (Carbon) | `53,876` (Flash) + `28,575` (Carbon) | `$0.40` Flash + `$0.24` Carbon (`$0.64` total) |
| 2026-11-18 | `Cycle 3` (`A3`: Fast-Path Byte Pre-Filter & Benchmark Harness) | Flash Author (`797b1f56`) + Coordinator (`c2a5d909`) | Byte-slice pre-checks in `sanitizer.rs`, streaming `read_line` buffer reuse, `sanitizer_throughput.rs` release benchmark (`542 MB/s` scan / `947 MB/s` sanitize) | `49` | `38.88 ms` | `13.61 MB` | `84,792` (Flash) + `87,492` (Coordinator) | `806,822` (Flash) + `700,722` (Coordinator) | `22,434` (Flash) + `18,133` (Coordinator) | `$0.7502` (`ooo checkin` fallback) + `$0.0926` (`$0.8428` reported; `$0.1983` at Flash rates) |

## 2. Distribution & adoption snapshots

| Date | Latest Tag | Total Release Downloads | `v0.2.0` Downloads | GitHub Stars | Forks | Open Issues | Harnesses Supported |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 2026-11-13 | `v0.2.0` | `20` | `4` | `1` | `0` | `0` | `4` (Antigravity, AGY IDE, Claude Code, Cursor) |
| 2026-11-15 | `v0.2.0` | `20` | `4` | `1` | `0` | `0` | `7` (+ Codex CLI, Windsurf, Cline) |
| 2026-11-16 | `v0.2.0` | `20` | `4` | `1` | `0` | `0` | `7` + Turnkey Skill Bundle & Git Lifecycle Hooks |
| 2026-11-18 | `v0.2.0` | `20` | `4` | `1` | `0` | `0` | `7` + Turnkey Skill Bundle & Git Lifecycle Hooks |
