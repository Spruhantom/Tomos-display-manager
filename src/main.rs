mod session;

use session::discover_sessions;
use std::env;
use std::process::ExitCode;

fn print_help() {
    println!("TomOS Display Manager (tdm) — session discovery");
    println!();
    println!("USAGE:");
    println!("    tdm <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    list-sessions    List installed Wayland and X11 desktop sessions");
    println!("    check            Check session directory access and desktop entries");
    println!("    version          Print version information");
    println!("    help             Show this help");
}

fn run() -> Result<(), String> {
    let command = env::args().nth(1).unwrap_or_else(|| "help".to_owned());
    match command.as_str() {
        "help" | "--help" | "-h" => print_help(),
        "version" | "--version" | "-V" => println!("tdm {}", env!("CARGO_PKG_VERSION")),
        "list-sessions" | "list" => {
            let (sessions, warnings) = discover_sessions();
            if sessions.is_empty() {
                println!("No desktop sessions found.");
                println!("Looked in /usr/share/wayland-sessions and /usr/share/xsessions.");
            } else {
                println!(
                    "{:<10}  {:<24}  {:<32}  DESKTOP FILE",
                    "TYPE", "SESSION", "COMMAND"
                );
                for session in &sessions {
                    println!(
                        "{:<10}  {:<24}  {:<32}  {}",
                        session.kind.label(),
                        session.name,
                        session.exec,
                        session.file.display()
                    );
                }
                println!();
                println!("{} session(s) found.", sessions.len());
            }
            for warning in warnings {
                eprintln!("warning: {warning}");
            }
        }
        "check" => {
            let (sessions, warnings) = discover_sessions();
            println!("Wayland session directory: /usr/share/wayland-sessions");
            println!("X11 session directory:     /usr/share/xsessions");
            println!("Valid visible sessions:    {}", sessions.len());
            if warnings.is_empty() {
                println!("Session metadata check:    no errors found");
            } else {
                println!("Session metadata check:    {} warning(s)", warnings.len());
                for warning in warnings {
                    eprintln!("warning: {warning}");
                }
            }
        }
        other => return Err(format!("unknown command '{other}'. Run 'tdm help'.")),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tdm: {error}");
            ExitCode::FAILURE
        }
    }
}
