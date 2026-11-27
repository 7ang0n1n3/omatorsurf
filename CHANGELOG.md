# Release notes

## Plugin 1.1.0 — automatic circuit timer (2026-11-28)

- Add Off, 5, 10, 15, and 30 minute controls plus custom whole-minute intervals.
- Enforce a five-minute minimum in both the widget and privileged helper.
- Authenticate once when setting the timer; share scheduling across monitors and
  keep it running when the popup closes.
- Pause while protection is unverified, reset the countdown after manual
  Start/Stop/New Circuit actions, and wait a full interval after failed renewals.
- Clear timer state on reboot or plugin removal; show the interval and countdown
  in the popup.
- Allow the system service to acquire the backend operation lock and extend its
  timeout for scheduled renewal and subsequent status checks.
- Add five scheduling/validation tests. All 54 backend tests and plugin validation
  pass. The installed plugin/helper/unit were updated and the shell restarted;
  live timer activation and scheduled NEWNYM remain unverified.

The backend remains version 1.0.0. Existing Tor streams remain open, and a new
circuit does not guarantee a different exit IP.

## Plugin 1.0.2 — submission preparation (unreleased)

- Expose the existing Omatorsurf widget through a repository-root manifest.
- Retain the flat local-install manifest and existing backend installation flow.
- Add marketplace metadata, capability disclosures and portable validation.

The widget provides Start/Stop, public IPv4, observed protection details and
NEWNYM controls. The backend remains version 1.0.0. These packaging changes do
not claim new live-shell or privileged interaction testing. No tag or release
has been published by this preparation; see [validation and limitations](README.md#validation-and-limitations).
