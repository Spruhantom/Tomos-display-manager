# TomOS Display Manager (TDM)

A lightweight Linux display-manager project written in Rust, intended for TomOS and other Linux distributions.

> **Development status: early foundation (0.1.0).** This first version discovers and validates installed desktop-session metadata. It is not yet a login screen and must not be configured as the system's login manager.

## Current functionality

- Native Rust command-line utility.
- Discovers Wayland sessions in `/usr/share/wayland-sessions`.
- Discovers X11 sessions in `/usr/share/xsessions`.
- Reads each desktop entry's `Name`, `Exec`, `Hidden`, and `NoDisplay` keys.
- Skips hidden sessions and reports malformed session entries.
- Includes unit tests and a CI workflow.

## Build

Requires Rust 1.75 or newer.

```sh
cargo build --release
```

The executable is created at `target/release/tdm`.

## Usage

```sh
tdm help
tdm list-sessions
tdm check
tdm version
```

## Security and architecture

TDM deliberately does **not** authenticate users, read passwords, run commands as root, or start desktop sessions in this initial release. A production display manager needs a carefully audited privilege boundary and a supported authentication/session backend.

The planned architecture is to use an established Linux login backend (such as `greetd` with PAM) rather than implementing password handling or PAM itself. The future greeter will communicate with that backend, while TDM handles session discovery, configuration, and the user interface. Until that integration and its security review are complete, keep your existing display manager enabled.

## Roadmap

- [x] Rust project and CLI foundation
- [x] Wayland and X11 session discovery
- [x] Desktop-entry validation and tests
- [ ] TOML configuration and theme support
- [ ] Graphical greeter
- [ ] Authentication backend integration
- [ ] Session launch and logout lifecycle
- [ ] systemd integration and distro packages
- [ ] Security review and installation testing

## Development

```sh
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```

## License

MIT. See [LICENSE](LICENSE).
