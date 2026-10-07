# CLI API v1

Use `pkexec /usr/local/bin/omatorsurf status --json` for a single live observation. Successful stdout contains exactly one JSON object followed by a newline. Diagnostics go to stderr. Consumers must handle nonzero exit and empty stdout as an unavailable observation, never as proof of protection. Unattended GUI polling requires a future privileged service with authenticated IPC; repeated interactive pkexec prompts are unsuitable for that use.

| Field | Type | Meaning |
| --- | --- | --- |
| enabled | boolean | Complete rules verified, application Tor ready, and fresh HTTPS Tor route confirmed |
| tor_running | boolean | Dedicated application service observed active |
| firewall_active | boolean | Entire application table matches expected rules, hooks, order and policies |
| killswitch | boolean | Verified blocking policy is installed |
| dns_protected | boolean | Verified DNS redirect and blocking rules are installed |
| ipv6_protected | boolean | Verified non-loopback IPv6 blocking is installed |
| public_ip | string or null | Confirmed exit IPv4 address when enabled; otherwise null |
| protection_state | string | protected, degraded, or disabled |
| errors | array of strings | Observation details and failure reasons; no machine parsing of message wording |

Example while Tor is unavailable but the guard remains:

```json
{"enabled":false,"tor_running":false,"firewall_active":true,"killswitch":true,"dns_protected":true,"ipv6_protected":true,"public_ip":null,"protection_state":"degraded","errors":["Tor is unavailable or not ready; retained rules continue blocking bypass traffic"]}
```

DNS/IPv6/kill-switch flags describe installed protection even when Tor cannot deliver traffic. `enabled` describes a verified usable protected route. `disabled` means the application table was absent; unrelated firewall configuration may still restrict connectivity. An existing but altered/unrecognized application table is degraded with unverified flags. Runtime progress is not substituted for live observations.

Status may make a bounded 40-second HTTPS request only when the table is verified and Tor is ready. It disables curl configuration and proxy settings and uses normal IPv4 system routing. Rules and Tor readiness are checked again after that request. Offline/blocked requests produce degraded status, null IP, and a diagnostic. Status is a point-in-time observation, not a continuous monitor.

## Exit codes

- 0: command succeeded. Status 0 means an observation was available; inspect `enabled`/`protection_state` to know protection.
- 1: operation failed or `check` found any required health failure. Partial startup may have left a fail-closed guard; use status and explicitly stop/retry.
- 2: invalid CLI arguments from clap.

`check` prints a structured human report and fails if dependencies, Tor service/bootstrap/listeners, complete firewall, DNS redirect, kill switch, IPv6 guard, route/DNS connectivity, or the final protection recheck fail. It never probes an unverified route.

All mutation commands require root. Full status/check also require root for nftables observation in v1. The backend does not escalate itself. `version`, help and config rendering need no privilege. Missing root access emits a pkexec hint and no fabricated JSON.

Existing field names/types are stable. Consumers should tolerate extra fields and future protection_state values, treating unknown states as unconfirmed protection. Use the supplied [JSON schema](config/status.schema.json) for documentation/validation.
