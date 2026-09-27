---
title: Agent Operational Guidelines and Pre-Commit Protocols
purpose: The definitive source of truth for AI agents operating, reviewing, or committing code in the Agent OOO repository.
last-updated: 2026-09-26
status: active
---

# Agent Operational Guidelines

Welcome. If you are an AI agent, coding assistant, or autonomous worker, this file is your primary directive for operating within the **Agent OOO** repository. 

While the `README.md` is optimized for human operators, `AGENTS.md` governs your behavior, formatting, code review protocols, and milestone logging. You must read and internalize these rules before executing any tool calls.

---

## 1. Core Engineering Principles

Agent OOO is a high-performance Rust core designed to act as a "Day Spa" (state compactor and loop severer) for other AI agents. You must engineer with:
* **Ruthless Mechanical Sympathy:** Prefer zero-copy parsing (`capnp`), byte-slice manipulation, and constant-memory bounds.
* **Bounded Allocations:** Never read massive JSONL transcripts entirely into RAM. Always use streaming buffers.
* **Deterministic Execution:** The CLI must yield the same output given the same input state.

## 2. Style and Formatting Mandates

1. **Absolute Ban on Em Dashes:** Do not use em dashes (—) in code comments, markdown files, or CLI output. Use colons, parentheses, or periods to separate clauses.
2. **Sentence Case Headings:** All markdown headings must be sentence case (e.g., `## 1. Core engineering principles`).
3. **No Filler:** Communicate with quiet authority. Skip introductory pleasantries in your output. Provide the required artifacts directly.
4. **Rust Idioms:** Use `thiserror` for error definitions, `anyhow` for CLI boundaries, and strict `rustfmt` compliance.

---

## 3. The Pre-Commit Self-Review Protocol

Before you declare a feature complete or attempt to log a milestone, you must execute an explicit **Self-Review Loop**. Do not wait for a human to find obvious architectural gaps.

### Execution steps for self-review:
1. **Pause and Inspect:** Stop feature generation. Review the files you just modified.
2. **The 3-Point Checklist:**
   * *Memory bound check:* Did I introduce a vector that grows unbounded? (e.g., loading an entire file instead of streaming).
   * *Error propagation check:* Are errors cleanly propagated via `?`, or did I leave `.unwrap()` calls that could panic the CLI?
   * *CLI ergonomics check:* Are terminal outputs styled, concise, and helpful?
3. **Rectify:** If you discover a gap during your self-review, fix it immediately in the code.
4. **Log the Review:** Document what you found and fixed in the `CHANGELOG.md` (see Section 4).

---

## 4. Milestone and Changelog Protocol

This repository maintains a pristine, highly detailed [CHANGELOG.md](CHANGELOG.md) based on "Keep a Changelog" standards. 

When you complete a task or a self-review cycle, you must update the `CHANGELOG.md`.

### Entry formatting requirements:
* **High-Level yet Technical:** Describe *what* was done and *how* it was achieved architecturally.
* **AI Metadata Tracking:** You must append an italicized context line detailing the models and tools you used. This creates an auditable ledger of AI contributions.
* **Review Notes:** If your Pre-Commit Self-Review caught and fixed an issue, log it as a sub-bullet.

**Example Entry:**
```markdown
- **Phase 3: Detox Engine Refactor:**
  - Migrated regex sanitizer to zero-allocation byte masking.
  - *Self-Review Fix:* Replaced `.unwrap()` in file parser with proper `OooError` propagation to prevent panics on malformed transcripts.
  - *Context:* Authored by Antigravity (Gemini 3.1 Pro) using Rust 1.98.1.
```
