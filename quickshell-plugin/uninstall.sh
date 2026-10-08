#!/usr/bin/env bash
set -euo pipefail
plugin_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
plugin_id=io.github.7ang0n1n3.omatorsurf
case "${1:-}" in
    --help|-h)
        printf '%s\n' "Usage: $plugin_dir/uninstall.sh" \
            'Run as your desktop user. Removes the plugin and its status publisher.' \
            'Tor routing and the backend installation are preserved.'
        exit 0 ;;
    '') ;;
    *) printf '%s\n' 'No arguments are supported.' >&2; exit 1 ;;
esac
[[ $# == 0 && $EUID != 0 ]] || {
    printf '%s\n' 'Run this script as your regular desktop user; it invokes pkexec itself.' >&2; exit 1;
}
for dependency in pkexec omarchy omarchy-shell python3; do
    command -v "$dependency" >/dev/null || { printf 'Missing dependency: %s\n' "$dependency" >&2; exit 1; }
done
config_root="$HOME/.config/omarchy"
destination="$config_root/plugins/$plugin_id"
if [[ -e "$destination" || -L "$destination" ]]; then
    [[ -d "$destination" && ! -L "$destination" ]] || { printf '%s\n' 'Unexpected plugin destination.' >&2; exit 1; }
    python3 -c 'import json, sys; assert json.load(open(sys.argv[1]))["id"] == sys.argv[2]' \
        "$destination/manifest.json" "$plugin_id"
fi
backup_root="${XDG_STATE_HOME:-$HOME/.local/state}/omatorsurf/plugin-backups/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p -- "$backup_root"
[[ ! -f "$config_root/shell.json" ]] || cp -p -- "$config_root/shell.json" "$backup_root/shell.json"
omarchy plugin disable "$plugin_id"
pkexec "$plugin_dir/scripts/install-system.sh" --uninstall
if [[ -d "$destination" ]]; then
    mv -- "$destination" "$backup_root/plugin"
fi
omarchy-shell shell rescanPlugins
printf '%s\n' "Plugin removed. Backup: $backup_root" 'Tor routing remains in its current state.'
