use colored::Colorize;

pub fn print_banner() {
    println!("\n");
    println!(
        "{}",
        format!("Agent OOO v{}", env!("CARGO_PKG_VERSION")).bold()
    );
    println!(
        "{}",
        "Spa retreats for your AI. Reset, refresh, relax.".italic()
    );
    println!("{}", "https://github.com/mrtibbets/agent-ooo".dimmed());
    println!();
}
