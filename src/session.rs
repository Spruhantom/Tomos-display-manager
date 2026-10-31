use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub name: String,
    pub exec: String,
    pub command: Vec<String>,
    pub kind: SessionKind,
    pub file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Wayland,
    X11,
}

impl SessionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
        }
    }

    // Used by the greeter binary; the CLI binary compiles this module too.
    #[allow(dead_code)]
    pub fn env_value(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
        }
    }
}

pub fn parse_desktop_entry(path: &Path, kind: SessionKind) -> Result<Option<Session>, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    parse_desktop_contents(&contents, path, kind)
}

pub fn parse_desktop_contents(
    contents: &str,
    path: &Path,
    kind: SessionKind,
) -> Result<Option<Session>, String> {
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut try_exec = None;
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
                "TryExec" => try_exec = Some(value.trim().to_owned()),
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

    if let Some(binary) = try_exec {
        if !binary.is_empty() && !executable_exists(&binary) {
            return Ok(None);
        }
    }

    // Desktop session Exec values are tokenized into argv. They are never
    // passed to a shell, so metacharacters cannot become shell commands.
    let mut command = shlex::split(&exec)
        .ok_or_else(|| format!("{}: invalid quoting in Exec", path.display()))?;
    if command.is_empty() || command[0].is_empty() {
        return Err(format!("{}: Exec contains no executable", path.display()));
    }

    for argument in &mut command {
        let mut expanded = String::with_capacity(argument.len());
        let mut characters = argument.chars();
        while let Some(character) = characters.next() {
            if character == '%' {
                match characters.next() {
                    Some('%') => expanded.push('%'),
                    _ => {
                        return Err(format!(
                            "{}: unsupported desktop-entry field code in Exec",
                            path.display()
                        ));
                    }
                }
            } else {
                expanded.push(character);
            }
        }
        *argument = expanded;
    }

    Ok(Some(Session {
        name,
        exec,
        command,
        kind,
        file: path.to_path_buf(),
    }))
}

fn executable_exists(program: &str) -> bool {
    let path = Path::new(program);
    if path.components().count() > 1 {
        return is_executable_file(path);
    }

    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .any(|directory| is_executable_file(&directory.join(program)))
        })
        .unwrap_or(false)
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
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
            Ok(Some(session)) if !executable_exists(&session.command[0]) => {
                warnings.push(format!(
                    "{}: session executable '{}' was not found or is not executable",
                    file.display(),
                    session.command[0]
                ));
            }
            Ok(Some(session)) => sessions.push(session),
            Ok(None) => {}
            Err(warning) => warnings.push(warning),
        }
    }

    sessions.sort_by_key(|session| session.name.to_lowercase());
    (sessions, warnings)
}

pub fn discover_sessions() -> (Vec<Session>, Vec<String>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_wayland_desktop_entry_into_safe_argv() {
        let path = Path::new("/tmp/example.desktop");
        let input = "[Desktop Entry]\nName=Example Desktop\nExec=example-session --wayland\nType=Application\n";
        let parsed = parse_desktop_contents(input, path, SessionKind::Wayland).unwrap();
        let session = parsed.unwrap();
        assert_eq!(session.name, "Example Desktop");
        assert_eq!(session.command, ["example-session", "--wayland"]);
        assert_eq!(session.kind, SessionKind::Wayland);
    }

    #[test]
    fn handles_quoted_arguments_without_shell_expansion() {
        let path = Path::new("/tmp/quoted.desktop");
        let input = "[Desktop Entry]\nName=Quoted\nExec=desktop-session --title 'TomOS Desktop'\n";
        let session = parse_desktop_contents(input, path, SessionKind::X11)
            .unwrap()
            .unwrap();
        assert_eq!(
            session.command,
            ["desktop-session", "--title", "TomOS Desktop"]
        );
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

    #[test]
    fn rejects_unclosed_quotes() {
        let path = Path::new("/tmp/unclosed.desktop");
        let input = "[Desktop Entry]\nName=Broken\nExec=desktop-session 'oops\n";
        assert!(parse_desktop_contents(input, path, SessionKind::X11).is_err());
    }

    #[test]
    fn rejects_unsupported_field_codes() {
        let path = Path::new("/tmp/field-code.desktop");
        let input = "[Desktop Entry]\nName=Broken\nExec=desktop-session %f\n";
        assert!(parse_desktop_contents(input, path, SessionKind::X11).is_err());
    }
}
