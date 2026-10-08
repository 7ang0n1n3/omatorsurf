# Omatorsurf for the Omarchy bar

**Plugin version: 1.0.2** · MIT · Omatorsurf backend 1.0.0 or newer.

Native Quickshell plugin for the current Omarchy shell plugin framework. The
manifest is at this directory's root. Install locally using the script below;
`omarchy plugin add` expects a standalone repository with a root manifest and
cannot install this subdirectory from the main Omatorsurf repository.

## Install

First install the backend from the repository root:

```bash
pkexec "$(pwd)/scripts/install.sh"
./quickshell-plugin/install.sh
```

Run the plugin installer as your regular desktop user. Its system stage uses
`pkexec`, installs a root-owned helper and status timer, then the user stage copies
the QML/manifest into `~/.config/omarchy/plugins/io.github.7ang0n1n3.omatorsurf/`,
backs up `shell.json`, waits for asynchronous discovery to complete, and enables
the widget in the right section. Existing layout
placement is preserved by Omarchy when already enabled. Omarchy hot-reloads the
plugin; no shell restart is needed. Neither installer enables Tor routing.

Requires the installed Omarchy shell (`qs.Commons`, `qs.Ui`, plugin manifest
schema 1), a working graphical polkit agent, Python 3, curl, systemd and coreutils.
It is not a standalone Quickshell configuration or a Waybar module. The main
backend installer remains independent: rerun this plugin installer to update the
plugin/helper. User backups are stored under
`~/.local/state/omatorsurf/plugin-backups/` (`XDG_STATE_HOME` is honored). The
installed Omarchy registry uses `~/.config/omarchy` for plugins and layout.

## Controls

The bar contains a single theme-colored Nerd Font icon: **U+F199A** when off
and **U+F0CCC** when the route is verified protected. Unknown, degraded or busy
states use the off icon; the tooltip describes the actual state. Click the icon
to open the popup. All text, IP information and action buttons live in the popup.

| Control | Action |
| --- | --- |
| Start / Stop | One toggle; authenticate with pkexec, then enable routing or restore direct networking |
| Public IP | Tor exit IPv4 while protected; ordinary public IPv4 while off |
| Status | Tor, firewall, kill switch, DNS, IPv6, observation age and error details |
| New circuit | Authenticate and request NEWNYM for future connections; enabled only when verified protected |

Stop also works in the degraded state. If status is unavailable, the popup offers
an explicit **Stop / restore network** recovery action. Authentication cancellation
and command failures appear in the popup. Controls are disabled during an action;
the public IP and protection flags are hidden until the resulting observation.
The popup supports Tab, Enter/Space, Escape, scrolling and outside-click dismissal.
The same single icon is used on horizontal and vertical bars.

Tor may keep the same exit IP after NEWNYM; existing streams remain open. No direct
IP lookup runs while degraded. The ordinary IPv4 lookup while off deliberately
uses the unprotected route to Tor Project's HTTPS IP API. Offline lookup failures
display Unknown without treating the IP lookup failure as a routing state change.

### Layout and settings

```bash
omarchy bar move io.github.7ang0n1n3.omatorsurf --section left
```

The bar always shows just the icon. The previous `showIp` setting is no longer
used; the popup always includes the public IP.

## Theme adherence

All colors are live bindings to Omarchy's bar facade or `Color.popups`.
Typography, geometry and spacing use `Style` tokens. Controls use the host's
`WidgetButton`, `BarIconButton`, `Button`, `KeyboardPanel`, section header and
separator, including shared hover/focus/border styles and theme rounding.
No bundled palette, fixed font, accent override, or theme file modification is
used. Theme/font changes propagate through the existing shell singletons.

## Status and privilege design

`/usr/local/libexec/omatorsurf-bar` is an installed root-owned copy of the Python
bridge. Its command vocabulary is fixed: `read`, `observe`, `start`, `stop`,
`new-circuit`, `activate`, `cleanup`. The last two are installer lifecycle commands.
It accepts no executable, configuration path or command
string. Only `read` runs without root. Each user-requested change goes through
pkexec; no passwordless polkit policy is installed.

`omatorsurf-bar-status.timer` runs a read-only root observation at boot and 15
seconds after each observation completes. The bridge calls the backend's live
`status --json`, validates its types, then atomically publishes
`/run/omatorsurf-bar/status.json`. The directory and file must be root-owned,
not writable by group/others, and not symlinks. The reader checks those properties
using directory/file descriptors. A helper lock serializes publication and plugin
actions; actions may wait for an in-progress route check before starting.

The widget reads locally every two seconds. Observations over 90 seconds old,
failed reads, failed backend checks or inconsistent data are shown as Unknown.
**This is a timestamped observation, not continuous proof of protection.** The
kernel rules remain responsible for the kill switch. An external CLI operation
can change state between observations; always inspect the age. During a plugin
action the bridge publishes a busy state with no IP/protection assertion, then
observes again, including after failures. If an action is interrupted, later
timer observations reconcile state. Root publication errors appear in journald.

The helper retains its runtime lock inode until reboot. Status is shared across
monitors; there is only one expensive status publisher. The observer service
uses a read-only system/home sandbox, a private temporary directory and restricted
address families; its only writable persistent runtime path is its own directory.

## Remove

From the repository root, as your regular desktop user:

```bash
./quickshell-plugin/uninstall.sh
```

This backs up the layout, disables the widget, authenticates to stop/remove the
status timer/service/helper, and moves the installed QML into the user backup.
It preserves the backend, configuration, Tor data, and current network routing.
Wait for active actions and close pending authentication prompts before removal.

For a complete application uninstall, remove the plugin first, then run:

```bash
pkexec "$(pwd)/scripts/uninstall.sh"
```

## Troubleshooting

```bash
systemctl status omatorsurf-bar-status.timer omatorsurf-bar-status.service
pkexec /usr/bin/journalctl -u omatorsurf-bar-status.service --no-pager -n 50
/usr/local/libexec/omatorsurf-bar read
omarchy-shell shell rescanPlugins
```

If an earlier installation reported "plugin is not known", rerun the corrected
installer; it now waits for the live shell registry before enabling the widget.
The entry point is `BarWidget.qml`. If the shell retains an earlier failed QML
component after an update, run `omarchy restart shell` once to clear its cache.

The read command requires no authentication. If the timer/helper is absent, rerun
the plugin installer. The bar icon opens the most recent verified observation;
right-clicking it rereads the local snapshot and does not force a network probe.
The timer continues when the widget is disabled; use the uninstaller to remove it.

The widget and read-only status publisher have been loaded in the live Omarchy
shell. Privileged Start/Stop/NEWNYM interactions have not been exercised as part
of the bar appearance changes.
