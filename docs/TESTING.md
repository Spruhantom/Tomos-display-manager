# TDM integration and release testing

TDM is experimental. Unit tests and CI do not prove that a real login works.
Run system-level tests only in disposable virtual machines with a snapshot,
a recovery TTY, and a separate administrator account.

## Automated checks

The regular CI workflow runs formatting, unit tests, Clippy, and installer
syntax checks. Protocol unit tests use in-memory streams; they do not start
greetd or authenticate through PAM.

## greetd/PAM integration test matrix

For each supported distribution image (Arch Linux, Ubuntu LTS, Fedora):

1. Install the distribution's greetd package and TDM package/build.
2. Confirm `greetd` is installed and its PAM service file exists.
3. Configure a disposable test greeter user and a temporary greetd config.
4. Use the distribution's normal PAM stack. Do not replace production PAM
   files; keep test-only changes inside the disposable VM.
5. Start greetd on a spare virtual terminal and verify the greeter receives
   `GREETD_SOCK`.
6. Authenticate with a disposable test account and confirm greetd accepts
   the session request.
7. Restore the VM snapshot after each test.

A test that substitutes a mock socket or `pam_permit` is a protocol smoke test,
not proof that the distribution's real PAM policy works. Never run this test
against a daily-use workstation or overwrite its PAM configuration.

## Wayland and X11 startup matrix

Test each session independently where available:

- Wayland: GNOME Wayland, KDE Plasma Wayland, and Sway.
- X11: GNOME on Xorg, KDE Plasma X11 (where packaged), and Xfce.
- Confirm the session appears once in `tdm list-sessions`.
- Sign in through greetd, verify the desktop reaches its normal session,
  and verify logout returns to greetd.
- Record distribution, package versions, session desktop file, and logs.

Missing desktop environments are marked not applicable, not passed.

## Installer and package validation

- Install and uninstall the Arch package in a clean Arch VM.
- Build/install the Debian package in a clean Ubuntu/Debian VM.
- Build/install the RPM in a clean Fedora VM.
- Verify both binaries, file ownership, dependencies, and uninstall cleanup.
- Test the curl installer in a disposable VM and confirm it does not enable
  greetd or replace the current display manager.

## Power controls

Power actions must be sent through systemd-logind (for example via
`loginctl`) and rely on the host's authorization policy. Do not run
`shutdown`, `reboot`, or `systemctl poweroff` through a shell, and do not
add a privileged setuid helper. UI actions must require explicit confirmation
and report authorization failures.

## Accessibility and themes

Review keyboard-only operation, focus order/visibility, screen-reader labels,
text scaling, contrast, and high-contrast mode at the actual greeter resolution.
Test themes with both light and dark system preferences. Do not rely on color
alone to communicate authentication errors or status.

## Security review checklist

- No shell interpolation for session launch or power actions.
- No password logging, persistence, or debug output.
- Clear secret entry fields after every response and on cancellation.
- Bound IPC frame sizes and socket read/write timeouts.
- Validate session desktop entries and executable paths.
- Review greetd user permissions and PAM policy per distribution.
- Preserve a recovery TTY and existing display manager until end-to-end tests pass.
