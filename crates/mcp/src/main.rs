use std::env;
use std::io::{self, BufReader, IsTerminal};

fn print_help() {
    println!(
        "agent-ooo-mcp {}\n\
         Model Context Protocol (MCP) stdio server for Agent OOO.\n\n\
         USAGE:\n    \
         agent-ooo-mcp [OPTIONS]\n\n\
         OPTIONS:\n    \
         -h, --help       Print help information\n    \
         -V, --version    Print version information\n\n\
         TOOLS EXPOSED OVER JSON-RPC 2.0 (STDIO):\n    \
         spa          Run the full 4-step retreat (Checkin, Detox, Reset, Discharge)\n    \
         checkin      Parse transcripts, diagnose context rot, or list discovered sessions\n    \
         detox        Scan and mask leaked credentials and offload oversized payloads\n    \
         reset        Extract git registers and write Cap'n Proto checkpoints\n    \
         discharge    Verify checkpoint integrity and inspect checkpoint state",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() -> anyhow::Result<()> {
    if let Some(arg) = env::args().nth(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("agent-ooo-mcp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            other => {
                anyhow::bail!("Unknown option: {other}. Run 'agent-ooo-mcp --help' for usage.");
            }
        }
    }

    let stdin = io::stdin();
    if stdin.is_terminal() {
        eprintln!(
            "[agent-ooo-mcp v{}] Listening for MCP JSON-RPC 2.0 messages on stdin (Ctrl-D to exit)...",
            env!("CARGO_PKG_VERSION")
        );
    }
    let stdout = io::stdout();
    agent_ooo_mcp::serve_stdio(BufReader::new(stdin.lock()), stdout.lock())
}
