# TomOS Display Manager (TDM)

A lightweight Linux display-manager project written in Rust for TomOS and other Linux distributions.

> **Development status: experimental preview (0.0.6).** TDM includes Wayland/X11 session discovery, a GTK4 session selector, and an initial greetd/PAM authentication and session-launch flow. It has not yet completed end-to-end testing on a real machine. Keep a recovery login path available and do not replace your existing login manager yet.

## Components

### `tdm` — session discovery CLI

- Discovers Wayland sessions in `/usr/share/wayland-sessions`.
- Discovers X11 sessions in `/usr/share/xsessions`.
- Reads session names, launch commands, `TryExec`, `Hidden`, and `NoDisplay` desktop-entry keys.
- Skips hidden entries and entries whose `TryExec` program is unavailable.
- Tokenizes launch commands into argument arrays; commands are not passed through a shell.
- Rejects malformed quoting and unsupported desktop-entry field codes.

### `tdm-greeter` — GTK4 greeter

- TomOS dark-charcoal and neon-cyan interface.
- Shows detected Wayland and X11 sessions in a desktop-session selector.
- Sends the entered Linux username to greetd using its IPC protocol.
- Uses greetd as the authentication broker; PAM policy is applied by the host's greetd installation.
- Handles secret, visible, informational, and error authentication messages.
- Clears response fields after submission, supports cancellation, and applies socket read/write timeouts.
- After authentication, sends greetd a `start_session` request containing the selected session's argv and session type.
- Exits after greetd accepts the session request, allowing greetd to launch the user session.

**Important limitations:** TDM does not implement PAM itself and does not verify passwords. Authentication and launching require a correctly configured greetd instance with `GREETD_SOCK` available and a suitable host PAM configuration. Session startup depends on the installed desktop session's own environment and startup scripts. This initial implementation has not yet been validated end-to-end on different distributions or desktop environments. Keep a separate TTY or recovery login available until that testing is complete.

## Install (Arch, Debian/Ubuntu, Fedora)

The installer builds TDM from source and installs `tdm` and `tdm-greeter` into `/usr/local/bin`. It installs build dependencies and Rust for the current user where needed. It does **not** configure greetd, enable a service, or replace your current display manager.

```sh
curl -fsSL https://raw.githubusercontent.com/Spruhantom/Tomos-display-manager/main/install.sh | bash
```

Supported distribution families: Arch-based, Debian/Ubuntu-based, and Fedora. Other distributions are rejected rather than guessed. Review the [installer script](install.sh) before running it. TDM is experimental; keep your existing login manager and a recovery TTY available.

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

## Authentication and session-launch architecture

TDM delegates authentication to greetd and the host's PAM stack. The greetd IPC protocol carries authentication prompts and responses; TDM never reads shadow files, calls password-verification APIs, or stores credentials.

After greetd reports authentication success, TDM sends `start_session` with the selected desktop command as a JSON argument array. It does not use `sh -c` or shell interpolation. The request adds `XDG_SESSION_TYPE=wayland` or `XDG_SESSION_TYPE=x11` to the environment greetd creates from PAM. greetd starts the session after the greeter process exits.

## Packaging and greetd setup

Experimental packaging recipes are provided under `packaging/` for Arch,
Debian-family, and RPM-family systems. They are starting points, not signed
or release-validated packages. Build them in clean distribution VMs and
verify dependencies before distributing them.

An example greetd configuration is installed as documentation by the package
recipes. Review it before copying it to `/etc/greetd/config.toml`. The example
uses the `_greetd` account; account names and service setup vary by distro.
TDM uses the distribution-provided `greetd.service`; it does not install a
competing display-manager service or enable greetd automatically.

The greeter has dark, light, and high-contrast themes, plus confirmed power
controls routed through `loginctl`/systemd-logind. See [power and themes](docs/POWER-AND-THEMES.md),
[systemd setup](docs/SYSTEMD.md), and the [initial security review](docs/SECURITY-REVIEW.md).
These features still require testing on real greetd sessions and distro images.

See [system testing](docs/TESTING.md) and the [release checklist](docs/RELEASE-CHECKLIST.md)
before using TDM as a daily login manager.

## Roadmap

- [x] Rust project and CLI foundation
- [x] Wayland and X11 session discovery
- [x] Desktop-entry validation and unit tests
- [x] GTK4 graphical greeter
- [x] Initial greetd/PAM authentication conversation
- [x] Secret/visible/info/error prompt handling
- [x] Authentication cancellation and IPC timeouts
- [x] Basic greetd IPC framing tests
- [x] GTK session selector and greetd start_session request
- [x] Safe argv-based session command parsing (no shell)
- [ ] Automated end-to-end tests against greetd and PAM
- [ ] Test Wayland and X11 startup on supported distributions
- [x] Initial power-off, restart, and suspend controls with confirmation (logind authorization still needs distro testing) with logind authorization and confirmation
- [ ] Configurable themes and completed accessibility review
- [ ] Verified systemd/greetd integration and release-ready distro packages
- [ ] Independent security review and clean-VM installation testing

## Development

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

## License

MIT. See [LICENSE](LICENSE).
