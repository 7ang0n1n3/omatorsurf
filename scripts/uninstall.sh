#!/usr/bin/env bash
# Explicit uninstall; never removes unrelated services, firewall tables or packages.
set -euo pipefail
umask 077

script_path="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/uninstall.sh"
project_dir="$(cd -- "$(dirname -- "$script_path")/.." && pwd)"
purge=false
backend_only=false
usage() {
    printf '%s\n' "Usage: $script_path [--backend-only] [--purge]" \
        'Run as your desktop user to remove the Quickshell plugin and backend.' \
        'Uses pkexec for system cleanup and restores direct networking.' \
        '--backend-only removes just the CLI backend and may be run through pkexec.' \
        'Configuration and Tor data are preserved by default.' \
        '--purge also deletes /etc/omatorsurf and /var/lib/omatorsurf.'
}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --purge) purge=true; shift ;;
        --backend-only) backend_only=true; shift ;;
        *) usage >&2; exit 1 ;;
    esac
done
backend_args=(--backend-only)
[[ "$purge" == false ]] || backend_args+=(--purge)
if [[ "$backend_only" == false ]]; then
    [[ $EUID != 0 ]] || {
        printf '%s\n' "Run $script_path as your regular desktop user, without pkexec, so the plugin is removed from your account. Use --backend-only for root-only cleanup." >&2
        exit 1
    }
    command -v pkexec >/dev/null || { printf '%s\n' 'Missing dependency: pkexec' >&2; exit 1; }
    plugin_destination="$HOME/.config/omarchy/plugins/io.github.7ang0n1n3.omatorsurf"
    if [[ -e "$plugin_destination" || -L "$plugin_destination" ]]; then
        "$project_dir/quickshell-plugin/uninstall.sh"
    else
        # A backend-only or partial installation may have no user widget.
        # Still remove any status publisher before deleting its backend.
        pkexec "$project_dir/quickshell-plugin/scripts/install-system.sh" --uninstall
    fi
    pkexec "$script_path" "${backend_args[@]}" || {
        printf '%s\n' 'Plugin/status publisher removed, but backend cleanup failed. Resolve the error and rerun this uninstaller.' >&2
        exit 1
    }
    printf '%s\n' 'Backend and Quickshell plugin uninstalled for this user. Plugin backups were preserved.'
    exit 0
fi
if [[ $EUID != 0 ]]; then
    command -v pkexec >/dev/null || { printf '%s\n' 'Missing dependency: pkexec' >&2; exit 1; }
    exec pkexec "$script_path" "${backend_args[@]}"
fi

for dependency in systemctl nft python3 flock stat rm rmdir; do
    command -v "$dependency" >/dev/null || {
        printf '%s\n' "Missing dependency: $dependency. Installed files were not removed." >&2
        exit 1
    }
done

# Do not depend on a working binary/config: uninstall must work with damaged config.
# Failed observations are errors, never evidence that protection is absent.
observe_table() {
    local presence
    presence="$(nft --json list tables | python3 -c '
import json, sys
items = json.load(sys.stdin)["nftables"]
present = any(x.get("table", {}).get("family") == "inet" and x.get("table", {}).get("name") == "omatorsurf" for x in items)
print("present" if present else "absent")
')" || return $?
    if [[ "$presence" == absent ]]; then
        printf '%s\n' absent
        return
    fi
    nft --json list table inet omatorsurf | python3 -c '
import json, sys
items = json.load(sys.stdin)["nftables"]
owned = any(x.get("table", {}).get("family") == "inet" and x.get("table", {}).get("name") == "omatorsurf" and x.get("table", {}).get("comment") == "omatorsurf managed table v1" for x in items)
if not owned:
    sys.exit("Refusing uninstall: inet omatorsurf has an unknown ownership marker. No firewall rules were removed.")
print("managed")
' || return $?
}

# Refuse an unrecognized table before stopping anything.
observe_table >/dev/null

units=(omatorsurf.service omatorsurf-guard.service omatorsurf-tor.service)
for unit in "${units[@]}"; do
    load_state="$(systemctl --no-ask-password show "$unit" --property=LoadState --value)"
    case "$load_state" in
        not-found)
            active_state="$(systemctl --no-ask-password show "$unit" --property=ActiveState --value)"
            case "$active_state" in
                inactive|failed) continue ;;
                *) printf '%s\n' "Uninstall stopped: $unit has no loaded unit file but remains $active_state. Stop it before retrying." >&2; exit 1 ;;
            esac
            ;;
        loaded|masked)
            # Stop routing before taking the CLI lock: its ExecStop uses that lock.
            systemctl --no-ask-password disable --now "$unit"
            active_state="$(systemctl --no-ask-password show "$unit" --property=ActiveState --value)"
            case "$active_state" in
                inactive|failed) ;;
                *) printf '%s\n' "Uninstall stopped: $unit remains $active_state. Installed files and protection were retained." >&2; exit 1 ;;
            esac
            ;;
        *) printf '%s\n' "Cannot safely stop $unit (LoadState=$load_state). Installed files were retained." >&2; exit 1 ;;
    esac
done

# Serialize cleanup with CLI mutations. Keep the lock inode until reboot so
# another process cannot acquire a new lock while this operation is finishing.
if [[ -L /run/omatorsurf ]]; then
    printf '%s\n' 'Refusing a symlink at /run/omatorsurf.' >&2; exit 1
fi
mkdir -p /run/omatorsurf
[[ "$(stat -c %u /run/omatorsurf)" == 0 ]] || {
    printf '%s\n' 'Runtime directory must be owned by root.' >&2; exit 1;
}
chmod 0755 /run/omatorsurf
[[ ! -L /run/omatorsurf/lock ]] || { printf '%s\n' 'Refusing a symlink at the runtime lock.' >&2; exit 1; }
exec 9>>/run/omatorsurf/lock
flock --nonblock 9 || {
    printf '%s\n' 'Another omatorsurf operation is running. Retry uninstall after it finishes; installed files and protection were retained.' >&2
    exit 1
}

if [[ "$(observe_table)" == managed ]]; then
    nft delete table inet omatorsurf
fi
[[ "$(observe_table)" == absent ]] || {
    printf '%s\n' 'Firewall removal could not be confirmed; installed files were retained.' >&2; exit 1;
}
# DNS protection used firewall redirects; resolver files need no restoration.

for unit in "${units[@]}"; do
    rm -f -- "/etc/systemd/system/$unit"
done
systemctl daemon-reload
rm -f -- /usr/local/bin/omatorsurf /usr/share/omatorsurf/omatorsurf.nft /run/omatorsurf/state.json
if [[ -d /usr/share/omatorsurf && ! -L /usr/share/omatorsurf ]]; then
    rmdir --ignore-fail-on-non-empty -- /usr/share/omatorsurf
fi

if [[ "$purge" == true ]]; then
    rm -rf -- /etc/omatorsurf /var/lib/omatorsurf
    printf '%s\n' 'Omatorsurf uninstalled; configuration and Tor data deleted.'
else
    printf '%s\n' 'Omatorsurf uninstalled; configuration and Tor data preserved.'
fi
printf '%s\n' 'Application protection removed; direct networking is permitted again.' \
    'The source checkout, dependency packages, and unrelated services/firewall tables were preserved.'
