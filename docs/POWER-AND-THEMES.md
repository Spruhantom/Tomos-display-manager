# Power controls and greeter themes

## Power controls

The greeter offers Power off, Restart, and Suspend actions. Each action opens
a confirmation dialog and then invokes `loginctl` directly with a fixed
argument. It does not invoke a shell or use a setuid helper. systemd-logind
and the host's authorization policy decide whether the action is permitted.

If an action is denied, check the distribution's polkit/logind policy and
greeter account setup. Do not grant broad administrative privileges to the
greeter account to make the buttons work.

## Theme selection

Copy `/usr/share/doc/tomos-display-manager/greeter.conf.example` to
`/etc/tdm/greeter.conf` (create the directory if needed), then set one of:

- `theme=dark` (default)
- `theme=light`
- `theme=high-contrast`

The `TDM_THEME` environment variable overrides the system config file.
Unknown values fall back to dark. GTK keyboard navigation and focus outlines
are retained across themes. GTK text scaling and assistive-technology
settings should be configured through the host desktop/greeter environment.

These themes are an initial implementation, not a completed accessibility
certification. Please follow the testing checklist before production use.
