# Security and capability disclosure

Omatorsurf changes system networking. Review the source and README protection
policy before use. Automated plugin checks are not a security audit.

## Report a problem

For non-sensitive bugs, use https://github.com/7ang0n1n3/omatorsurf/issues.
For a suspected vulnerability, use GitHub's private “Report a vulnerability”
option on this repository's Security page if available. If private reporting is
unavailable, open an issue requesting a private contact without exploit details,
credentials, public IP addresses, or identifying logs. No response SLA is promised.

## Privileges and processes

The widget runs as the desktop user and reads the installed root-owned status
snapshot. Start, stop, and NEWNYM authenticate through pkexec and invoke the
fixed-command root helper. There is no bundled passwordless polkit policy.
The backend controls its own nftables table and systemd units; Tor runs under
its dedicated account. Installation requires root writes through pkexec and may
build the Rust binary as the invoking user. Dependencies are installed separately.

## Network and data

Tor connects to the Tor network. Status checks and the ordinary public IPv4 lookup
contact https://check.torproject.org/api/ip using curl. While routing is disabled,
that lookup uses the direct connection. Degraded state suppresses direct IP lookup.
Runtime files live under /run/omatorsurf and /run/omatorsurf-bar; configuration
under /etc/omatorsurf; Tor data under /var/lib/omatorsurf. The status snapshot
contains network/protection information, timestamps, errors and public IPv4 and
is readable by local users. The plugin does not collect account credentials.

## Installation and removal

Installation writes the application binary, helper and systemd units, and updates
the invoking user's plugin and bar layout after backing them up. Running the
installer explicitly requests these changes; it does not enable Tor routing.
Plugin-only removal leaves the backend and routing state intact. Full removal
restores direct networking and removes application services/files; configuration,
Tor data and user backups remain unless the documented purge option applies.
See README.md for exact commands, paths, retained files and failure behavior.

## Validation limits

A status observation can become stale. The kernel guard, not the widget, enforces
routing policy. Privileged software and the Tor account are trusted. Reboot,
suspend/resume, and privileged widget interactions require further validation as
documented in submission/VALIDATION.md. No universal anonymity claim is made.
