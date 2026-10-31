use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Session {
    name: String,
    exec: String,
    kind: SessionKind,
    file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionKind {
    Wayland,
    X11,
}

impl SessionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
        }
    }
}

fn parse_desktop_entry(path: &Path, kind: SessionKind) -> Result<Option<Session>, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    parse_desktop_contents(&contents, path, kind)
}

fn parse_desktop_contents(
    contents: &str,
    path: &Path,
    kind: SessionKind,
) -> Result<Option<Session>, String> {
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut hidden = false;
    let mut no_display = false;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Name" => name = Some(value.trim().to_owned()),
                "Exec" => exec = Some(value.trim().to_owned()),
                "Hidden" => hidden = value.trim().eq_ignore_ascii_case("true"),
                "NoDisplay" => no_display = value.trim().eq_ignore_ascii_case("true"),
                _ => {}
            }
        }
    }

    if hidden || no_display {
        return Ok(None);
    }

    let name = name.unwrap_or_default();
    let exec = exec.unwrap_or_default();
    if name.is_empty() || exec.is_empty() {
        return Err(format!(
            "{}: missing Name or Exec in [Desktop Entry]",
            path.display()
        ));
    }

    Ok(Some(Session {
        name,
        exec,
        kind,
        file: path.to_path_buf(),
    }))
}

fn scan_directory(path: &Path, kind: SessionKind) -> (Vec<Session>, Vec<String>) {
    let mut sessions = Vec::new();
    let mut warnings = Vec::new();

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (sessions, warnings),
        Err(error) => {
            warnings.push(format!("{}: {error}", path.display()));
            return (sessions, warnings);
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        let file = entry.path();
        if file.extension().and_then(|value| value.to_str()) != Some("desktop") {
            continue;
        }
        match parse_desktop_entry(&file, kind) {
            Ok(Some(session)) => sessions.push(session),
            Ok(None) => {}
            Err(warning) => warnings.push(warning),
        }
    }

    sessions.sort_by_key(|session| session.name.to_lowercase());
    (sessions, warnings)
}

fn discover_sessions() -> (Vec<Session>, Vec<String>) {
    let (mut sessions, mut warnings) = scan_directory(
        Path::new("/usr/share/wayland-sessions"),
        SessionKind::Wayland,
    );
    let (x11_sessions, x11_warnings) =
        scan_directory(Path::new("/usr/share/xsessions"), SessionKind::X11);
    sessions.extend(x11_sessions);
    warnings.extend(x11_warnings);
    sessions.sort_by_key(|session| session.name.to_lowercase());
    (sessions, warnings)
}

fn print_help() {
    println!("TomOS Display Manager (tdm) — session discovery foundation");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_wayland_desktop_entry() {
        let path = Path::new("/tmp/example.desktop");
        let input =
            "[Desktop Entry]\nName=Example Desktop\nExec=example-session\nType=Application\n";
        let parsed = parse_desktop_contents(input, path, SessionKind::Wayland).unwrap();
        let session = parsed.unwrap();
        assert_eq!(session.name, "Example Desktop");
        assert_eq!(session.exec, "example-session");
        assert_eq!(session.kind, SessionKind::Wayland);
    }

    #[test]
    fn ignores_hidden_entries() {
        let path = Path::new("/tmp/hidden.desktop");
        let input = "[Desktop Entry]\nName=Hidden\nExec=hidden-session\nHidden=true\n";
        assert!(parse_desktop_contents(input, path, SessionKind::X11)
            .unwrap()
            .is_none());
    }

    #[test]
    fn ignores_entries_outside_desktop_entry_group() {
        let path = Path::new("/tmp/other-group.desktop");
        let input = "[Other]\nName=Wrong Group\nExec=wrong\n";
        assert!(parse_desktop_contents(input, path, SessionKind::X11).is_err());
    }

    #[test]
    fn rejects_missing_exec() {
        let path = Path::new("/tmp/broken.desktop");
        let input = "[Desktop Entry]\nName=Broken\n";
        assert!(parse_desktop_contents(input, path, SessionKind::X11).is_err());
    }
}
