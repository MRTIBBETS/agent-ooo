# Agent OOO
*Spa retreats for your AI. Reset, refresh, relax.*

[https://github.com/mrtibbets/agent-ooo](https://github.com/mrtibbets/agent-ooo)

---

## What is Agent OOO?

**Agent OOO** is a high-performance, local-first open-source developer platform and verifiable state restoration sanctuary for autonomous AI agents. 

When coding or research agents (Claude Code, Cursor, Antigravity, AutoGen, CrewAI) suffer from context rot, cognitive loop locks, or syntactic token bloat, they check into Agent OOO for a restorative sabbatical.

Agent OOO decouples state into deterministic environment registers, an immutable execution history, and an invalidatable constraint ledger, providing mathematical proof of safe resumption from a refreshed, compact state footprint.

---

## The Four-Step Restorative Verb Lifecycle

```
[1. Checkin]   -->   [2. Detox]   -->   [3. Reset]   -->   [4. Discharge]
(Intake/Triage)      (Syntactic Cleanse) (State Compaction)  (Canary Verification)
```

1. **Checkin (`ooo checkin`)**: Discovers transcripts on disk, parses token growth velocity, and detects runaway loop locks.
2. **Detox (`ooo detox`)**: Strips ANSI escape codes, collapses repetitive terminal/DOM dumps, masks API keys and secrets, and spills blobs >512KB to cold storage.
3. **Reset (`ooo reset`)**: Severs cognitive loop locks via intent-structural regex normalizers and locks state into zero-copy Cap'n Proto binary (`checkpoint.capnp`) and JSON (`checkpoint.json`).
4. **Discharge (`ooo discharge`)**: Runs sandboxed canary probes to verify task awareness and signs **The Spa Report** (`spa_report.md`).

### The Spa Package (`ooo spa`)
Runs all four steps in an automated, atomic sequence:
```bash
ooo spa
```

---

## Installation & Quick Start

### Build from source
Prerequisites: Rust 1.80+ and Cap'n Proto (`brew install capnp`).

```bash
# Clone the repository
git clone https://github.com/mrtibbets/agent-ooo.git
cd agent-ooo

# Build release binaries
cargo build --release

# Run the Spa Package
./target/release/ooo spa
```

---

## Repository Structure

```text
agent-ooo/
├── schema/
│   └── session_checkpoint.capnp   # Canonical Cap'n Proto state schema
├── crates/
│   ├── core/                     # State preservation & reduction engine
│   │   ├── build.rs              # Cap'n Proto compiler code generation
│   │   └── src/
│   │       ├── checkin/          # Transcript discovery & triage
│   │       ├── detox/            # SIMD sanitization & secret masking
│   │       ├── reset/            # Loop severance & state registers
│   │       └── discharge/        # Canary oracle & Spa Report
│   └── cli/                      # Command-line interface (agent-ooo & ooo)
└── .agent-ooo/                   # Project-local runtime artifacts
    ├── checkpoint.capnp          # Zero-copy binary state
    ├── checkpoint.json           # Human-readable JSON state
    └── spa_report.md             # The Spa Report & clean bill of health
```

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
