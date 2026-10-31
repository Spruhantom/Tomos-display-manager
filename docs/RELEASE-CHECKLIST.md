# TDM release checklist

- [ ] Rust formatting, unit tests, and Clippy pass.
- [ ] greetd/PAM tests pass on disposable VMs for every supported distro.
- [ ] Wayland session startup passes on each distro/session combination.
- [ ] X11 session startup passes where the distro provides X11 sessions.
- [ ] Power actions require confirmation and respect logind authorization.
- [ ] Theme settings persist safely and accessibility review is complete.
- [ ] Arch, Debian, and RPM packages build and install cleanly.
- [ ] systemd/greetd setup and recovery instructions are verified.
- [ ] Installer tested from a clean VM snapshot.
- [ ] Security review completed; no unresolved critical/high findings.
- [ ] Existing display manager remains recoverable until TDM is proven.
