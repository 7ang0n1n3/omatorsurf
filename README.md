# omatorsurf

**Version: 1.0.0** — MIT licensed. The package version is defined in [Cargo.toml](Cargo.toml); check your installed binary with `omatorsurf version` or `omatorsurf --version`.

Rust CLI backend for system-wide Tor routing on Arch Linux/Omarchy. The backend manages Tor, nftables and systemd through a command-line interface. Once enabled, the kernel routing rules cover applications, browsers and background services on the host, including applications launched outside the terminal.

Tor routes supported TCP traffic and DNS requests. This build blocks other outbound IP traffic according to the protection policy below. The [Quickshell plugin](quickshell-plugin/README.md) provides native Omarchy taskbar controls with live theme bindings. The main installer installs both the backend and plugin by default; `--backend-only` installs the CLI independently.

## Requirements

- Arch Linux or Omarchy with systemd.
- Tor, nftables, iproute2 (`ss`), curl, Python and python-stem.
- `pkexec` and working polkit authentication for privileged operations.
- For the default full install: Omarchy with its Quickshell shell running, the `omarchy` and `omarchy-shell` commands, and a graphical polkit agent. Run the scripts from that desktop user's session.
- Standard Bash, coreutils and util-linux tools, including `runuser` for automatic builds and `flock` for uninstall locking.
- Rust 1.89 or newer and Cargo when a release needs building; Git for cloning/updating the repository.

The installer checks runtime dependencies and the Tor service account/group. It reports missing dependencies rather than installing them automatically. To install the main runtime packages explicitly:

```bash
pkexec /usr/bin/pacman -S --needed tor nftables iproute2 curl python-stem
```

## Install

Clone and run the installer as your regular desktop user:

```bash
git clone https://github.com/7ang0n1n3/omatorsurf.git
cd omatorsurf
./scripts/install.sh
```

This installs the backend, the Quickshell bar plugin, and its root status publisher. The script invokes pkexec for privileged stages; do not wrap the full installer in pkexec or sudo. Authentication may be requested for each system stage. Runtime packages must already be installed.

### Installer behavior

- Checks that the Omarchy shell is reachable, installs the backend first, then installs/enables the plugin for the current desktop user.
- The default binary is `target/release/omatorsurf` inside the checkout.
- If that binary is missing, the installer runs `cargo build --locked --release --target-dir <checkout>/target` as the user who invoked pkexec, then installs it with root privileges. Cargo and build scripts are never run as root.
- An existing executable release is reused. Installation does not rebuild it when source files change.
- An existing default release that is not executable is reported as an error. A missing or non-executable custom binary also fails without an automatic build.
- Existing `/etc/omatorsurf/config.toml` is preserved. Its configuration determines the generated Tor configuration. Differing installed Tor configuration or unit files cause installation to stop before deployment files are written; back them up and review the differences before replacement.
- Backs up the existing plugin and `shell.json`, copies the plugin into `~/.config/omarchy/plugins/io.github.7ang0n1n3.omatorsurf/`, waits for discovery, and enables the bar widget. Backups go under `~/.local/state/omatorsurf/plugin-backups/` (`XDG_STATE_HOME` is honored).
- Enables/starts `omatorsurf-bar-status.timer` for read-only status observations. Installation does not start/enable Tor routing or apply firewall rules. Use the widget's Start button or `pkexec /usr/local/bin/omatorsurf start` afterward.
- If plugin installation fails after backend installation, the backend remains installed. Resolve the reported error and rerun the installer.

To install another existing executable:

```bash
./scripts/install.sh --binary /absolute/path/to/omatorsurf
```

For a backend-only installation on Arch Linux without the Omarchy desktop, use:

```bash
./scripts/install.sh --backend-only
# Or explicitly invoke the system-only stage:
pkexec "$(pwd)/scripts/install.sh" --backend-only
```

`--backend-only` can also be combined with `--binary PATH`. It skips all plugin/status-publisher installation and does not start or enable services.

View installer help without root:

```bash
./scripts/install.sh --help
```

### Installed files

| Path | Purpose |
| --- | --- |
| `/usr/local/bin/omatorsurf` | Application binary |
| `/etc/omatorsurf/config.toml` | System configuration; existing file is preserved |
| `/etc/omatorsurf/torrc` | Generated Tor configuration |
| `/usr/share/omatorsurf/omatorsurf.nft` | Firewall template; installing it does not apply rules |
| `/etc/systemd/system/omatorsurf.service` | Optional routing startup unit |
| `/etc/systemd/system/omatorsurf-guard.service` | Early network guard unit |
| `/etc/systemd/system/omatorsurf-tor.service` | Dedicated application Tor unit |
| `/run/omatorsurf/` | Runtime state and transaction lock |
| `~/.config/omarchy/plugins/io.github.7ang0n1n3.omatorsurf/` | Quickshell widget and manifest for the invoking desktop user |
| `/usr/local/libexec/omatorsurf-bar` | Root-owned status publisher and authenticated plugin action helper |
| `/etc/systemd/system/omatorsurf-bar-status.service` | Read-only status observation service |
| `/etc/systemd/system/omatorsurf-bar-status.timer` | Enabled periodic status publisher |
| `/run/omatorsurf-bar/` | Published status and helper lock |

The dedicated Tor service creates its private state under `/var/lib/omatorsurf/tor`. Keep the checkout if you want to use its install/uninstall scripts later; these scripts are not copied into `/usr/local/bin`.

### Rebuild and update

From the checkout, update the source and explicitly rebuild an existing release as your regular user:

```bash
git pull --ff-only
cargo build --locked --release --target-dir "$(pwd)/target"
./scripts/install.sh
omatorsurf version
```

The installer updates both backend and plugin. Use `--backend-only` when updating a standalone backend. To build manually before a first installation, use the same Cargo command. Rust dependencies may need network access unless they are already cached. The installer also refuses automatic compilation when it cannot identify a non-root build account.

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

Full status/check require root because nftables inspection needs privileges. The CLI never launches an internal privilege prompt. The optional bar plugin reads timestamped observations from a root-owned status publisher and uses pkexec for explicit changes.

| Command | Behavior |
| --- | --- |
| `start` | Install protection, start Tor and verify the protected route |
| `stop` | Stop application Tor and remove application networking restrictions |
| `status` / `status --json` | Observe actual Tor, firewall and routing state |
| `check` | Run live health checks; return nonzero on required failures |
| `new-circuit` | Request new circuits for future Tor connections |
| `version` / `--version` | Print the installed application version without root |

Use the top-level `start` command to enable system-wide protection. The diagnostic `tor start` command starts only application Tor and does not install networking protection. Closing the terminal does not remove installed kernel rules; explicit `stop` or uninstall removes them.

Status reports `protected` only when the complete rules, Tor readiness and a fresh route check succeed. `degraded` means the application table exists but protection or connectivity is incomplete; a retained guard can block traffic while Tor is unavailable. `disabled` means the application table is absent. A successful status command means an observation was obtained, so inspect its fields to determine protection.

`start` installs the kill switch before starting Tor, checks bootstrap/listeners, DNS rules, and a fresh HTTPS response from Tor Project's IP API. Repeated start replaces the owned table atomically. Startup failure leaves any installed guard in place; retry `start` to recover or explicitly `stop` to restore direct networking. Failed Tor shutdown retains the guard. `stop` affects only application Tor and `inet omatorsurf`; repeated stop is safe.

`new-circuit` uses installed Python Stem with cookie authentication and Tor's NEWNYM signal. Future connections can use new circuits; existing streams remain open and a different exit IP is not guaranteed. [Stem documentation](https://stem.torproject.org/api/control.html) describes these semantics.

## Omarchy taskbar plugin

**Plugin version: 1.0.2.** The main installer includes the plugin. To install or update only the plugin against an already installed backend, run as your regular desktop user:

```bash
./quickshell-plugin/install.sh
```

The installer invokes pkexec for the root status publisher, backs up your shell layout, and enables the native Omarchy bar widget. It does not start Tor routing.

- The bar shows one Nerd Font icon: U+F199A when off, U+F0CCC when verified protected.
- Clicking it opens a popup with Start/Stop, current public IPv4, status details and New Circuit.
- Start/Stop enables routing or restores direct networking through pkexec.
- New Circuit requests future Tor circuits through pkexec; the exit IP may stay the same.

Colors, typography, spacing, borders, rounding and popup behavior use the active Omarchy shell theme and shared UI components. Stale or failed observations display Unknown. See the [plugin documentation](quickshell-plugin/README.md) for installation, privilege design, settings and removal. The widget's bar rendering and read-only status publisher have been confirmed in the live desktop; privileged button interactions remain untested.

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

Manual CLI start does **not** persist protection through reboot. Optional service startup has been tested on the running host; actual reboot, suspend/resume, initramfs networking and network-manager-specific boot ordering still need VM/hardware validation. Do not assume protection before the early guard has successfully installed. Routing at boot is opt-in; installation does not change existing routing service enablement. The full installer enables the plugin's read-only status timer.

## Uninstall

Run from the checkout as the desktop user who installed the plugin:

```bash
./scripts/uninstall.sh
```

The uninstaller:

1. Backs up the shell layout, disables the widget, removes its status timer/service/helper through pkexec, and moves the user plugin into its backup directory. If no user plugin exists, it still removes any status publisher from a partial installation.
2. Invokes the privileged backend cleanup, checks ownership of `inet omatorsurf`, and refuses to remove an unrecognized table.
3. Stops/disables the three application units and obtains the transaction lock.
4. Removes only the owned application firewall table and confirms removal.
5. Removes installed unit files, reloads systemd, and removes the installed binary, template and `state.json`.

Do not wrap the full uninstaller in pkexec or sudo. It removes the plugin for the invoking user; plugin copies in other users' accounts are preserved. Wait for active plugin actions and close pending authentication prompts before removal. A plugin cleanup failure stops before backend removal; if backend cleanup fails after the plugin is removed, resolve the error and rerun the script.

Removing the application guard permits direct networking again, subject to any unrelated firewall rules. Resolver files need no restoration because the application never rewrites them. The uninstaller does not require a working application binary or configuration. Service/firewall cleanup failure stops removal of installed files; a busy transaction lock requires retrying after the other operation finishes.

| Item | Default uninstall | With `--purge` |
| --- | --- | --- |
| Application binary, template and units | Removed | Removed |
| Application firewall table and `state.json` | Removed | Removed |
| User plugin, status helper and status units | Removed | Removed |
| Plugin/layout backups | Preserved | Preserved |
| `/etc/omatorsurf` configuration | Preserved | Deleted |
| `/var/lib/omatorsurf` Tor data | Preserved | Deleted |
| Dependency packages, checkout and unrelated services/tables | Preserved | Preserved |

To also delete application configuration and persistent Tor data:

```bash
./scripts/uninstall.sh --purge
```

To remove only a standalone backend, use `./scripts/uninstall.sh --backend-only` (or `pkexec "$(pwd)/scripts/uninstall.sh" --backend-only`). Combine with `--purge` if needed. This option leaves any installed plugin and status publisher in place; use the default full uninstall for a combined installation.

The runtime lock is deliberately retained under `/run/omatorsurf` until reboot to avoid unlinking a lock held during cleanup. Other files in `/usr/share/omatorsurf` are preserved if present; custom systemd drop-in directories are not deleted by the script.

View uninstall help without root:

```bash
./scripts/uninstall.sh --help
```

## Troubleshooting

For startup, bootstrap or route-check failures, inspect live state and the application Tor journal:

```bash
pkexec /usr/local/bin/omatorsurf status --json
pkexec /usr/local/bin/omatorsurf check
pkexec /usr/bin/journalctl -u omatorsurf-tor.service --no-pager -n 100
```

A failed startup can leave networking blocked by the guard. Retry `start` after resolving the cause, or explicitly run `stop` to allow direct networking. Completed Tor bootstrap alone does not guarantee an immediately reachable exit circuit or external endpoint.

Additional inspection commands:

```bash
pkexec /usr/local/bin/omatorsurf tor status --json
pkexec /usr/local/bin/omatorsurf firewall status --json
pkexec /usr/local/bin/omatorsurf firewall check
omatorsurf tor config
```

`firewall check` validates a proposed nftables transaction without applying it. `tor config` prints the configuration expected at `/etc/omatorsurf/torrc`. For debug output from health checks:

```bash
pkexec /usr/bin/env RUST_LOG=debug /usr/local/bin/omatorsurf check
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
├── scripts/                  Install, uninstall and Tor circuit helper
└── quickshell-plugin/        Omarchy bar widget, status bridge and its installers
```

Licensed under the MIT license; see [LICENSE](LICENSE).
