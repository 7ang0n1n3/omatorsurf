#!/usr/bin/env bash
set -euo pipefail
plugin_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
plugin_id=io.github.7ang0n1n3.omatorsurf
case "${1:-}" in
    --help|-h)
        printf '%s\n' "Usage: $plugin_dir/install.sh" \
            'Run as your desktop user. Uses pkexec to install the root status publisher.' \
            'Copies the plugin, backs up shell.json, and enables it in the right bar section.'
        exit 0 ;;
    '') ;;
    *) printf '%s\n' 'No arguments are supported.' >&2; exit 1 ;;
esac
[[ $# == 0 && $EUID != 0 ]] || {
    printf '%s\n' 'Run this installer as your regular desktop user, without pkexec. It invokes pkexec for its system installation.' >&2
    exit 1
}
for dependency in pkexec omarchy omarchy-shell python3 install; do
    command -v "$dependency" >/dev/null || { printf 'Missing dependency: %s\n' "$dependency" >&2; exit 1; }
done
# The installed Omarchy PluginRegistry uses HOME/.config, not XDG_CONFIG_HOME.
config_root="$HOME/.config/omarchy"
destination="$config_root/plugins/$plugin_id"
backup_root="${XDG_STATE_HOME:-$HOME/.local/state}/omatorsurf/plugin-backups/$(date +%Y%m%d-%H%M%S)-$$"
if [[ -e "$destination" || -L "$destination" ]]; then
    [[ -d "$destination" && ! -L "$destination" ]] || {
        printf '%s\n' 'Refusing to replace a non-directory or symlink plugin destination.' >&2; exit 1;
    }
    python3 -c 'import json, sys; assert json.load(open(sys.argv[1]))["id"] == sys.argv[2], "Unexpected plugin manifest"' \
        "$destination/manifest.json" "$plugin_id"
fi
# This is the only privileged stage; cancellation leaves user config untouched.
pkexec "$plugin_dir/scripts/install-system.sh"
mkdir -p -- "$config_root/plugins" "$backup_root"
if [[ -f "$config_root/shell.json" ]]; then
    cp -p -- "$config_root/shell.json" "$backup_root/shell.json"
fi
if [[ -d "$destination" ]]; then
    cp -a -- "$destination" "$backup_root/plugin"
fi
install -d -m 0755 -- "$destination"
for file in manifest.json BarWidget.qml Backend.qml StatusRow.qml README.md LICENSE; do
    install -m 0644 -- "$plugin_dir/$file" "$destination/$file"
done
# The previous entry point is retained in the backup, not in the live plugin.
rm -f -- "$destination/Widget.qml"
omarchy-shell shell rescanPlugins
# Discovery is asynchronous. Wait for the live registry, rather than relying on
# a fixed delay or enabling an ID that the shell has not discovered yet.
python3 - "$plugin_id" <<'PY'
import json
import subprocess
import sys
import time

deadline = time.monotonic() + 20
while time.monotonic() < deadline:
    result = subprocess.run(
        ["omarchy-shell", "shell", "listPlugins"],
        capture_output=True, text=True, timeout=5,
    )
    if result.returncode == 0:
        try:
            if any(plugin.get("id") == sys.argv[1] for plugin in json.loads(result.stdout)):
                break
        except (ValueError, TypeError):
            pass
    time.sleep(0.25)
else:
    sys.exit("Omarchy did not discover the plugin. Inspect the omarchy-shell journal; no enable request was sent.")
PY
omarchy plugin enable "$plugin_id"
printf '%s\n' "Omatorsurf enabled in the bar. Backup: $backup_root" \
    'Use the Start button to enable Tor; installing the plugin does not enable routing.'
