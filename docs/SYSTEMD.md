# greetd and systemd setup

TDM is a greetd greeter, not a standalone systemd service. The greetd package
provides and owns `greetd.service`; creating a second TDM service would cause
two login managers to compete for the same seat.

1. Install greetd using your distribution's package manager.
2. Install TDM using a package or build it from source.
3. Back up the existing `/etc/greetd/config.toml`, if present.
4. Review `/usr/share/doc/tomos-display-manager/greetd-config.toml.example`.
5. Confirm the greeter account exists on the distribution. The example uses
   `_greetd`, but some distributions use another account. The example command
   points to `/usr/bin/tdm-greeter` (the package path); if you used the curl
   installer, change it to `/usr/local/bin/tdm-greeter`.
6. Configure the greetd default session to launch `/usr/bin/tdm-greeter`.
7. Keep a recovery TTY and the previous display manager available.
8. Only then, from a local terminal, enable/start the distribution's
   `greetd.service` according to its documentation.

Do not enable greetd remotely unless you have tested recovery access. Do not
disable or uninstall the existing display manager as part of TDM installation.
If the greeter fails, switch to a TTY, restore the backed-up greetd config,
and re-enable the previous display manager.

The exact unit name, greeter account, PAM service file, and polkit policy can
vary by distribution. Verify them on each supported image before declaring
systemd integration complete.
