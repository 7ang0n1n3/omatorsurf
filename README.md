# omatorsurf

Rust CLI backend for system-wide Tor routing on Arch Linux/Omarchy. The backend manages Tor, nftables and systemd through a command-line interface.

## Build and install

Dependencies: Rust 1.89 or newer/Cargo, tor, nftables, systemd, iproute2, curl, Python and python-stem. Install missing packages explicitly through `pkexec /usr/bin/pacman -S tor nftables iproute2 curl python-stem`. Run the installer from the cloned repository:

```bash
pkexec "$(pwd)/scripts/install.sh"
```

If the default release is missing, the installer runs `cargo build --locked --release` as the user who invoked pkexec, then installs it with root privileges. An existing release is reused. To rebuild after source changes, run `cargo build --locked --release` as your regular user before installation. Cargo and build scripts are never run as root.

The installer preserves configuration and refuses differing Tor/service files before installing. It installs the release binary and three dedicated systemd units and never enables or starts them automatically. Use `--binary /absolute/path/to/omatorsurf` to select an existing executable; a missing custom binary is reported as an error.

## Use

```bash
omatorsurf version
pkexec /usr/local/bin/omatorsurf status
pkexec /usr/local/bin/omatorsurf start
pkexec /usr/local/bin/omatorsurf status --json
pkexec /usr/local/bin/omatorsurf check
pkexec /usr/local/bin/omatorsurf new-circuit
pkexec /usr/local/bin/omatorsurf stop
```

Full status/check require root because nftables inspection needs privileges. The application never launches an internal privilege prompt. Unattended GUI access would require a future privileged service with authenticated IPC.

`start` installs the kill switch before starting Tor, checks bootstrap/listeners, DNS rules, and a fresh HTTPS response from Tor Project's IP API. Repeated start replaces the owned table atomically. Startup failure leaves any installed guard in place; retry `start` to recover or explicitly `stop` to restore direct networking. Failed Tor shutdown retains the guard. `stop` affects only application Tor and `inet omatorsurf`; repeated stop is safe.

`new-circuit` uses installed Python Stem with cookie authentication and Tor's NEWNYM signal. Future connections can use new circuits; existing streams remain open and a different exit IP is not guaranteed. [Stem documentation](https://stem.torproject.org/api/control.html) describes these semantics.

## Protection policy

- IPv4 TCP is transparently redirected to Tor; Tor's dedicated non-root UID alone bypasses redirection for its own IPv4 TCP.
- UDP DNS on port 53 redirects before loopback/private exclusions. Resolver files are never rewritten. Successful protected HTTPS requests exercise system name resolution.
- Non-loopback IPv6, general UDP, forwarding and private/LAN TCP are blocked. Local IPv4 DHCP ports 68→67 are explicitly allowed. Loopback remains available. ARP is outside this IP firewall.
- Existing direct connections receive no broad established/related exception. They are interrupted when protection starts.
- Tor stops/fails, the CLI crashes, or an interface changes: kernel rules remain installed. Recovery depends on network/Tor availability and may require `start` again.
- Privileged software and software running as the Tor account remain trusted. This backend does not conceal browser fingerprinting, application identifiers, or traffic before protection is enabled.

Only the dedicated nftables table is changed. Unrelated tables can independently block connections. Status certifies the complete ordered application rules against native numeric nft output; unexpected formatting or changed rules conservatively remain unverified. No cached state can certify protection.

## Configuration

Default: `/etc/omatorsurf/config.toml`; sample: [config/omatorsurf.toml](config/omatorsurf.toml). If the default file is absent, safe defaults apply. Explicit `--config PATH` must exist. Unknown fields, invalid/duplicate ports, disabling the kill switch and IPv6 policies other than `block` are rejected. Tor readiness timeout defaults to 120 seconds, configurable from 1–600.

After port changes, render `omatorsurf --config PATH tor config`, review it, and replace `/etc/omatorsurf/torrc` through pkexec. Startup requires that file to match the generated configuration exactly. All four listeners use IPv4 loopback, control uses cookie authentication, and the dedicated service runs as `tor`. Its current invocation must have logged completed bootstrap. Missing/inaccessible bootstrap logs conservatively prevent readiness.

`general.verify_connection=false` skips the extra initial route probe. Final health and status verification still require a fresh confirmed Tor route before reporting protection.

The extra startup probe retries transient curl DNS/connect/timeout errors at most three times while retaining the guard. A response that does not confirm Tor fails immediately. Persistent external failure leaves protection installed and reports an error; bootstrap alone cannot certify Internet availability.

`/run/omatorsurf/state.json` records transaction progress/errors atomically and is informational only. A kernel-backed lock serializes mutations and releases automatically on process death. Status always observes live state, even with a missing, malformed or stale state file.

## Optional systemd startup

Manual CLI startup is the default. After reviewing and testing the units in a VM, opt in:

```bash
pkexec /usr/bin/systemctl enable omatorsurf.service
pkexec /usr/bin/systemctl start omatorsurf.service
```

The required guard unit is ordered before `network-pre.target`; routing starts after `network-online.target`. Service stop/shutdown stops application Tor and retains the kernel guard. To disable boot startup and restore direct networking:

```bash
pkexec /usr/bin/systemctl disable --now omatorsurf.service
pkexec /usr/local/bin/omatorsurf stop
```

Manual CLI start does **not** persist protection through reboot. Optional service startup has been tested on the running host; actual reboot, suspend/resume, initramfs networking and network-manager-specific boot ordering still need VM/hardware validation. Do not assume protection before the early guard has successfully installed. The installer leaves boot startup disabled.

## Uninstall

The uninstaller stops/disables the three application units, removes only the owned firewall table, restores direct networking and removes installed application files. Configuration and Tor data are preserved by default. It leaves dependency packages and the source checkout in place. It also works when the installed binary or configuration is damaged. If service/firewall cleanup fails, it aborts before removing installed files.

```bash
pkexec "$(pwd)/scripts/uninstall.sh"
```

To also delete application configuration and Tor data:

```bash
pkexec "$(pwd)/scripts/uninstall.sh" --purge
```

## API and project layout

[API.md](API.md) documents JSON fields, degraded status, exit codes and frontend integration.

```text
omatorsurf/
├── Cargo.toml / Cargo.lock    Rust package and pinned dependencies
├── README.md / API.md        Deployment and API documentation
├── LICENSE                   MIT license
├── src/                      Application source
├── config/                   Configuration, Tor template and JSON schema
├── nftables/                 Routing template and runtime verification rules
├── systemd/                  Application service units
└── scripts/                  Install, uninstall and Tor circuit helper
```

Licensed under the MIT license; see [LICENSE](LICENSE).
