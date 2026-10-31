# TomOS Display Manager (TDM)

A lightweight Linux display-manager project written in Rust for TomOS and other Linux distributions.

> **Development status: experimental preview (0.2.0).** TDM currently includes a session-discovery CLI and a graphical GTK greeter preview. The graphical screen is not connected to authentication or session launching and must not be installed as the system login manager.

## Components

### `tdm` — session discovery CLI

- Discovers Wayland sessions in `/usr/share/wayland-sessions`.
- Discovers X11 sessions in `/usr/share/xsessions`.
- Reads `Name`, `Exec`, `Hidden`, and `NoDisplay` desktop-entry keys.
- Skips hidden entries and reports malformed session metadata.

### `tdm-greeter` — graphical preview

- GTK4 interface with a dark charcoal and neon-cyan TomOS theme.
- Username and password entry controls.
- Wayland/X11/automatic session selector mock-up.
- Login status feedback.
- Shutdown and restart controls shown as disabled until safely integrated.

**Important:** The greeter is a visual prototype only. It does not send, store, or authenticate credentials. The Continue button clears the password field and displays a preview-only message. Power controls are disabled. Do not configure it as a greeter for greetd, systemd, or another display manager.

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

Do not run the preview as a system login manager.

## Security architecture

The intended production design is to delegate authentication and privileged session management to a maintained backend such as greetd with PAM. TDM should act as an unprivileged greeter client and must not implement password verification, retain credentials, or run arbitrary commands as root.

Before production use, the project needs a reviewed greetd IPC state machine that supports all PAM conversation message types, safe session command parsing, cancellation and timeout handling, plus integration tests on supported distributions. No production authentication path is included in this release.

## Roadmap

- [x] Rust project and CLI foundation
- [x] Wayland and X11 session discovery
- [x] Desktop-entry validation and unit tests
- [x] GTK4 graphical greeter preview
- [ ] Live session discovery in the greeter
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
