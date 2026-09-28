# Agent OOO

**Spa retreats for your AI. Reset, refresh, relax.**

Agent OOO is a high-performance, zero-copy Rust engine that treats context rot in autonomous AI agents. By running Agent OOO between tasks, you can surgically strip out ANSI noise, mask API keys, offload massive payloads, and sever recursive error loops—ensuring your agent always maintains a pristine, healthy context window.

## Installation

Agent OOO compiles down to a single binary with zero external dependencies.

**macOS & Linux (Coming Soon to Homebrew)**
Currently, download the latest binary from the [GitHub Releases](https://github.com/mrtibbets/agent-ooo/releases) page and place it in your `$PATH`.

Alternatively, if you have Rust installed, compile it from source:
```bash
cargo install --git https://github.com/mrtibbets/agent-ooo
```

## The Spa Package

The primary command is `spa`. It automatically discovers your most recent active session (from `.gemini`, `.claude`, or `.cursor`) and runs the full 4-step treatment process.

```bash
agent-ooo spa
```

### The 4-Step Treatment:
1. **Checkin:** Parses your transcript history and calculates a degradation "Rot Index."
2. **Detox:** Strips terminal ANSI codes, masks leaked API secrets, and spills payloads larger than 512KB to disk.
3. **Reset:** Computes an intent-hash to detect and sever recursive LLM error loops, then locks the state into a zero-copy Cap'n Proto checkpoint.
4. **Discharge:** Generates a clinical `spa_report.md` detailing token reductions and economic savings.

## Updating

Agent OOO features a built-in update engine. To instantly pull down the latest optimized binary from GitHub without needing to recompile:

```bash
agent-ooo update
```

## Architecture

Agent OOO is engineered with ruthless mechanical sympathy. It uses `capnproto` for zero-copy state definition, `blake3` for content-addressed payload spilling, and a Generational Memory Arena to pin system instructions while cycling transient tool logs. 

For full operational agent guidelines, see [AGENTS.md](AGENTS.md).
