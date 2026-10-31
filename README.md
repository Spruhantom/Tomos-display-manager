# TomOS Display Manager (TDM)

A lightweight Linux display-manager project written in Rust for TomOS and other Linux distributions.

> **Development status: experimental preview (0.0.5).** TDM includes a session-discovery CLI and a GTK4 greeter with an initial greetd/PAM authentication flow. Desktop session launching is not implemented. Do not install this as your system login manager yet.

## Components

### `tdm` — session discovery CLI

- Discovers Wayland sessions in `/usr/share/wayland-sessions`.
- Discovers X11 sessions in `/usr/share/xsessions`.
- Reads `Name`, `Exec`, `Hidden`, and `NoDisplay` desktop-entry keys.
- Skips hidden entries and reports malformed session metadata.

### `tdm-greeter` — GTK4 greeter

- TomOS dark-charcoal and neon-cyan interface.
- Sends the entered Linux username to greetd using its documented IPC protocol.
- Uses greetd as the authentication broker; PAM policy is applied by the host's greetd installation.
- Handles secret, visible, informational, and error authentication messages.
- Clears response fields after submission, supports cancellation, and applies socket read/write timeouts.
- Runs the IPC conversation on a worker thread so the GTK interface remains responsive.
- Includes protocol framing and JSON field-name unit tests.

**Important limitations:** TDM does not implement PAM itself and does not verify passwords. Authentication only works when the greeter is launched by a correctly configured greetd instance with `GREETD_SOCK` available and a suitable host PAM configuration. After successful authentication, this preview cancels the pending greetd session because desktop session launching is not implemented yet. It is therefore an authentication integration preview, not a usable daily-login display manager. Do not configure it as your only login path.

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

The greeter must be started by greetd to receive its IPC socket. Running it directly from a normal desktop session will show a connection/configuration error when sign-in is attempted.

## Authentication architecture

TDM deliberately delegates authentication to greetd and the host's PAM stack. The greetd IPC protocol carries authentication prompts and responses; TDM never reads shadow files, calls password-verification APIs, or stores credentials. PAM conversation prompts are not assumed to be password-only: visible and secret prompts are presented to the user, while informational and error messages are displayed and acknowledged without an answer.

The protocol implementation uses Serde for typed request/response data and validates the native-endian 32-bit length-prefixed JSON frames. Responses larger than 1 MiB are rejected. This keeps the project on its existing MIT-compatible dependency path rather than adding a GPL-only IPC library.

## Roadmap

- [x] Rust project and CLI foundation
- [x] Wayland and X11 session discovery
- [x] Desktop-entry validation and unit tests
- [x] GTK4 graphical greeter
- [x] Initial greetd/PAM authentication conversation
- [x] Secret/visible/info/error prompt handling
- [x] Authentication cancellation and IPC timeouts
- [x] Basic greetd IPC framing tests
- [ ] Automated end-to-end tests against greetd and PAM
- [ ] Safe desktop session command selection and launching
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
