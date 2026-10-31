# TomOS Display Manager (TDM)

A lightweight Linux display-manager project written in Rust for TomOS and other Linux distributions.

> **Development status: experimental authentication preview (0.0.4).** TDM includes a session-discovery CLI and a GTK4 greeter that can perform PAM authentication through greetd's IPC socket. It does not yet launch a desktop session and must not replace a working system login manager.

## Components

### `tdm` — session discovery CLI

- Discovers Wayland sessions in `/usr/share/wayland-sessions`.
- Discovers X11 sessions in `/usr/share/xsessions`.
- Reads `Name`, `Exec`, `Hidden`, and `NoDisplay` desktop-entry keys.
- Skips hidden entries and reports malformed session metadata.

### `tdm-greeter` — GTK4 greeter and authentication preview

- Dark charcoal and neon-cyan TomOS theme.
- Connects only to the socket path supplied by `GREETD_SOCK`.
- Starts authentication with greetd, which delegates credential verification to the system's PAM configuration.
- Handles PAM's visible, secret, informational, and error prompts, including additional prompts such as one-time codes.
- Clears prompt entry contents after submission.
- Uses bounded IPC message sizes and socket timeouts.
- Cancels the pending greetd session after authentication succeeds because session launching is not implemented yet.

**Important:** This release verifies authentication but does not start Wayland or X11 sessions. The session selector and power controls are not yet connected. Run TDM only as a greetd greeter for testing, and keep your existing display manager available so you can recover if the greeter fails.

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

The greeter expects to be launched by greetd with `GREETD_SOCK` set. Running it directly from a normal desktop session will show a connection error; it is not a standalone password checker.

## Authentication design and limitations

TDM does not verify passwords itself and does not read or modify password databases. It sends the username and authentication responses to greetd over the greetd IPC socket; PAM policy and verification remain the responsibility of the host system.

The greeter treats PAM prompts as dynamic messages rather than assuming every prompt is a password. Authentication requests have a timeout, and cancelling a prompt cancels the pending greetd session. Credentials are not written to files or logs. This preview has not yet had a security audit or testing across distributions.

Before production use, the project still needs:
- Desktop session discovery wired into the UI.
- Safe parsing of desktop `Exec` entries and verified session launch.
- End-to-end testing with greetd/PAM on supported distributions.
- A security review, installation integration, and recovery testing.

## Roadmap

- [x] Rust project and CLI foundation
- [x] Wayland and X11 session discovery
- [x] Desktop-entry validation and unit tests
- [x] GTK4 graphical greeter
- [x] greetd IPC authentication flow
- [x] PAM visible/secret/info/error prompt handling
- [ ] Live session selection in the greeter
- [ ] Safe desktop session launching
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
