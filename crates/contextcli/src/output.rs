use colored::Colorize;

pub fn success(msg: &str) {
    eprintln!("{} {}", "✓".green().bold(), msg);
}

pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

pub fn info(msg: &str) {
    eprintln!("{} {}", "→".blue().bold(), msg);
}

pub fn hint(msg: &str) {
    eprintln!("  {}", msg.dimmed());
}

pub fn header(msg: &str) {
    eprintln!("{}", msg.bold());
}

pub fn status_badge(state: &str) -> String {
    match state {
        "authenticated" => format!("{}", "authenticated".green()),
        "unauthenticated" => format!("{}", "unauthenticated".yellow()),
        "expired" => format!("{}", "expired".red()),
        "error" => format!("{}", "error".red()),
        _ => state.to_string(),
    }
}
