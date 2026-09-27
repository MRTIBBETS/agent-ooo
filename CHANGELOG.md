# Changelog

All notable changes to the **Agent OOO** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **AI Engineering Ledger:** This changelog explicitly tracks the AI models, orchestration runtimes, and core dependencies utilized to author milestones, providing an auditable trail of generative contributions.

## [Unreleased]

### Added
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
