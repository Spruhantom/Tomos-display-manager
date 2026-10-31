# TomOS Display Manager (TDM)

A lightweight Linux display-manager project written in Rust for TomOS and other Linux distributions.

> **Development status: experimental preview (0.0.4).** TDM includes a session-discovery CLI and a GTK4 graphical greeter preview. **PAM authentication and desktop session launching are not implemented yet.** Do not install this as your system login manager.

## Components

### `tdm` — session discovery CLI

- Discovers Wayland sessions in `/usr/share/wayland-sessions`.
- Discovers X11 sessions in `/usr/share/xsessions`.
- Reads `Name`, `Exec`, `Hidden`, and `NoDisplay` desktop-entry keys.
- Skips hidden entries and reports malformed session metadata.

### `tdm-greeter` — graphical preview

- GTK4 interface with a dark charcoal and neon-cyan TomOS theme.
- Username and password fields for layout preview only.
- Wayland/X11/automatic session selector mock-up.
- Preview status feedback.
- Shutdown and restart controls are disabled.

**Important:** The Continue button does not send credentials anywhere. It clears the password field and displays a preview-only message. No PAM authentication is performed. The session selector is not connected to a session launcher. Do not configure this binary as the greeter for greetd, systemd, or another display manager.

## Build

Requires Rust 1.83 or newer, GTK4 development files, and `pkg-config`.

On Debian/Ubuntu:

```sh
sudo apt install build-essential pkg-config libgtk-4-dev
```

Build both binaries:

```sh
cargo build --release
```

Binaries:
- `target/release/tdm`
- `target/release/tdm-greeter`

## Run

List detected desktop sessions:

```sh
./target/release/tdm list-sessions
```

Check session metadata:

```sh
./target/release/tdm check
```

Launch the graphical preview from an existing desktop session:

```sh
./target/release/tdm-greeter
```

## Authentication roadmap

The planned design is to use greetd as the session and authentication backend, with PAM performing authentication according to the host distribution's policy. TDM must not implement password verification itself or store credentials.

Before authentication can be described as implemented, the project needs a protocol-correct greetd IPC state machine, handling all PAM conversation message types, cancellation, timeouts, and errors, plus integration tests with greetd/PAM. After authentication, it must safely launch a selected desktop session. Neither capability is included in this version.

## Roadmap

- [x] Rust project and CLI foundation
- [x] Wayland and X11 session discovery
- [x] Desktop-entry validation and unit tests
- [x] GTK4 graphical greeter preview
- [ ] Reviewed greetd IPC authentication flow
- [ ] PAM multi-prompt handling and cancellation
- [ ] Safe session command parsing and launching
- [ ] Functional power controls
- [ ] Theme configuration and accessibility review
- [ ] systemd integration and distro packages
- [ ] Security review and installation testing

## Development

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

## License

MIT. See [LICENSE](LICENSE).
