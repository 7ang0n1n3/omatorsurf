#!/usr/bin/env bash
# Omatorsurf bar integration: install/remove only the root status bridge.
set -euo pipefail
plugin_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
helper=/usr/local/libexec/omatorsurf-bar
units=(omatorsurf-bar-status.timer omatorsurf-bar-status.service)
case "${1:-}" in
    --help|-h)
        printf '%s\n' "Usage: pkexec $plugin_dir/scripts/install-system.sh [--uninstall]" \
            'Installs a status publisher; never starts/stops Tor or modifies firewall rules.'
        exit 0 ;;
    --uninstall|'') ;;
    *) printf '%s\n' 'Unknown argument' >&2; exit 1 ;;
esac
[[ $# -le 1 && $EUID == 0 ]] || {
    printf '%s\n' "Use pkexec $plugin_dir/scripts/install-system.sh [--uninstall]" >&2; exit 1;
}
for dependency in install systemctl python3 stat; do
    command -v "$dependency" >/dev/null || { printf 'Missing dependency: %s\n' "$dependency" >&2; exit 1; }
done

if [[ ${1:-} == --uninstall ]]; then
    # The helper lock prevents removing an authenticated operation in progress.
    # Stop the timer first so it cannot race with cleanup.
    for unit in "${units[@]}"; do
        if [[ -e /etc/systemd/system/$unit ]]; then
            systemctl disable --now "$unit"
        fi
    done
    if [[ -e "$helper" ]]; then
        [[ ! -L "$helper" && $(stat -c %u "$helper") == 0 ]] || {
            printf '%s\n' 'Refusing an untrusted installed status helper.' >&2; exit 1;
        }
        "$helper" cleanup || {
            printf '%s\n' 'Close pending authentication prompts and wait for actions to finish, then retry. The status timer was stopped.' >&2
            exit 1
        }
    fi
    rm -f -- "$helper" /etc/systemd/system/omatorsurf-bar-status.timer /etc/systemd/system/omatorsurf-bar-status.service
    systemctl daemon-reload
    printf '%s\n' 'Bar status publisher removed. Tor and the firewall were left in their current state.'
    exit 0
fi

for dependency in curl timeout; do
    command -v "$dependency" >/dev/null || { printf 'Missing dependency: %s\n' "$dependency" >&2; exit 1; }
done
[[ -f /usr/local/bin/omatorsurf && ! -L /usr/local/bin/omatorsurf && -x /usr/local/bin/omatorsurf ]] || {
    printf '%s\n' 'Install the Omatorsurf backend with scripts/install.sh first.' >&2; exit 1;
}
[[ $(stat -c %u /usr/local/bin/omatorsurf) == 0 ]] || {
    printf '%s\n' 'The installed backend must be owned by root.' >&2; exit 1;
}
# Reject writable installed executables before authorizing periodic root execution.
python3 -I -c 'import os, stat; p="/usr/local/bin/omatorsurf"; s=os.stat(p); assert not s.st_mode & 0o022, "Backend is writable by other users"'
[[ ! -L /usr/local/libexec ]] || { printf '%s\n' 'Refusing a symlink at /usr/local/libexec.' >&2; exit 1; }
install -d -o root -g root -m 0755 /usr/local/libexec
install -o root -g root -m 0755 -- "$plugin_dir/scripts/omatorsurf-bar.py" "$helper"
"$helper" activate || {
    printf '%s\n' 'Wait for active status checks/actions to finish, then rerun the plugin installer.' >&2; exit 1;
}
for unit in "${units[@]}"; do
    install -o root -g root -m 0644 -- "$plugin_dir/systemd/$unit" "/etc/systemd/system/$unit"
done
systemctl daemon-reload
systemctl enable --now omatorsurf-bar-status.timer
printf '%s\n' 'Status publisher installed and enabled. Tor routing was not changed.'
