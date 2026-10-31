# Initial security review notes

This is an internal engineering checklist, not an independent security audit.

## Existing safeguards observed

- Session commands are parsed into argv and are not passed to a shell.
- greetd IPC response frames are bounded to 1 MiB.
- Socket reads and writes have timeouts.
- Authentication is delegated to greetd and the host PAM stack.
- Secret prompts use a hidden GTK entry and the field is cleared after use.
- The greeter does not read shadow files or persist credentials.
- Power actions use fixed `loginctl` arguments after explicit confirmation.

## Items requiring further review and testing

- Confirm greetd protocol framing and cancellation behavior against each
  supported greetd release.
- Validate PAM prompt sequences, account/session modules, and failure paths
  on each distribution.
- Review greeter account permissions, socket access, and polkit/logind policy.
- Validate desktop-entry parsing against the full Desktop Entry specification.
- Confirm no credentials appear in logs, crash reports, or process arguments.
- Test power-control authorization from the actual greeter account.
- Run dependency auditing and a third-party code review before stable release.
- Test installer/package upgrades, rollback, and uninstall in clean VMs.

Do not describe TDM as production-ready until the outstanding checks have
been completed and recorded.
