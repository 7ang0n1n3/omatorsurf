#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
binary="$project_dir/target/release/omatorsurf"
custom_binary=false
backend_only=false
usage() {
    printf '%s\n' "Usage: $project_dir/scripts/install.sh [--backend-only] [--binary PATH]" \
        'Run as your desktop user to install the backend and Quickshell plugin.' \
        'Uses pkexec for system installation; enables the bar status publisher.' \
        '--backend-only installs just the CLI backend and may be run through pkexec.' \
        'Builds a missing default release as the invoking user.' \
        'An existing release is reused; --binary requires an existing executable.' \
        'Tor routing is not started or enabled; no firewall rules are applied.'
}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --backend-only) backend_only=true; shift ;;
        --binary)
            [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || { usage >&2; exit 1; }
            binary="$2"
            [[ "$binary" == /* ]] || binary="$PWD/$binary"
            custom_binary=true
            shift 2 ;;
        *) usage >&2; exit 1 ;;
    esac
done
if [[ "$backend_only" == false ]]; then
    [[ $EUID != 0 ]] || {
        printf '%s\n' "Run $project_dir/scripts/install.sh as your regular desktop user, without pkexec, so the plugin uses your desktop session. Use --backend-only for a root-only install." >&2
        exit 1
    }
    for dependency in pkexec omarchy omarchy-shell python3 install; do
        command -v "$dependency" >/dev/null || { printf 'Missing dependency: %s\n' "$dependency" >&2; exit 1; }
    done
    # Confirm the desktop shell is reachable before installing system files.
    omarchy-shell shell listPlugins >/dev/null
    backend_args=(--backend-only)
    [[ "$custom_binary" == false ]] || backend_args+=(--binary "$binary")
    pkexec "$project_dir/scripts/install.sh" "${backend_args[@]}"
    "$project_dir/quickshell-plugin/install.sh" || {
        printf '%s\n' 'Backend installed, but plugin installation failed. Resolve the error and rerun this installer.' >&2
        exit 1
    }
    printf '%s\n' 'Backend and Quickshell plugin installed. Use the widget Start button or pkexec /usr/local/bin/omatorsurf start to enable routing.'
    exit 0
fi
if [[ $EUID != 0 ]]; then
    command -v pkexec >/dev/null || { printf '%s\n' 'Missing dependency: pkexec' >&2; exit 1; }
    backend_args=(--backend-only)
    [[ "$custom_binary" == false ]] || backend_args+=(--binary "$binary")
    exec pkexec "$project_dir/scripts/install.sh" "${backend_args[@]}"
fi
for dependency in tor nft systemctl journalctl ss install cmp mktemp getent curl timeout python3; do
    if ! command -v "$dependency" >/dev/null; then
        printf '%s\n' "Missing dependency: $dependency. On Arch, install tor, nftables, systemd, and iproute2." >&2
        exit 1
    fi
done
python3 -I -c 'import stem' || { printf '%s\n' 'Missing Stem: pkexec /usr/bin/pacman -S python-stem' >&2; exit 1; }
getent passwd tor >/dev/null || { printf '%s\n' 'The tor service account is missing; install the Arch tor package.' >&2; exit 1; }
getent group tor >/dev/null || { printf '%s\n' 'The tor group is missing; install the Arch tor package.' >&2; exit 1; }
[[ -f /etc/arch-release ]] || printf '%s\n' 'Warning: this service is designed for Arch Linux and Omarchy.' >&2

if [[ ! -x "$binary" ]]; then
    if [[ "$custom_binary" == true ]]; then
        printf '%s\n' "Selected binary is missing or not executable: $binary" >&2
        exit 1
    fi
    if [[ -e "$binary" || -L "$binary" ]]; then
        printf '%s\n' "Release binary exists but is not executable: $binary. Review it before rebuilding." >&2
        exit 1
    fi
    for dependency in runuser stat env bash; do
        command -v "$dependency" >/dev/null || {
            printf '%s\n' "Missing build helper: $dependency" >&2; exit 1;
        }
    done
    # pkexec identifies its caller. Direct root invocation falls back to the
    # checkout owner; never execute Cargo/build scripts with root privileges.
    build_uid="${PKEXEC_UID:-$(stat -c %u "$project_dir")}"
    if [[ ! "$build_uid" =~ ^[0-9]+$ || "$build_uid" == 0 ]]; then
        printf '%s\n' 'Cannot identify a non-root build user. Build the release as your regular user, then run this installer through pkexec.' >&2
        exit 1
    fi
    account="$(getent passwd "$build_uid")" || {
        printf '%s\n' "Cannot find the non-root build account for UID $build_uid." >&2; exit 1;
    }
    IFS=: read -r build_user account_password account_uid account_gid account_description build_home account_shell <<< "$account"
    if [[ -z "$build_user" || "$account_uid" != "$build_uid" || "$build_home" != /* ]]; then
        printf '%s\n' 'Cannot resolve the build user and home directory.' >&2
        exit 1
    fi
    build_path="$build_home/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
    printf '%s\n' "Release binary is missing; building as $build_user..."
    # Argument arrays preserve paths with spaces; the shell command is fixed.
    # A clean user environment restores rustup/Cargo lookup after pkexec.
    runuser -u "$build_user" -- env -i \
        HOME="$build_home" USER="$build_user" LOGNAME="$build_user" PATH="$build_path" \
        bash -c 'cd -- "$1" || exit 1; command -v cargo >/dev/null || { printf "%s\n" "Cargo is missing. Install Rust/Cargo for your regular user first." >&2; exit 1; }; exec cargo build --locked --release --target-dir "$1/target"' \
        omatorsurf-build "$project_dir"
    [[ -x "$binary" ]] || {
        printf '%s\n' "Build did not produce the expected release binary: $binary" >&2
        exit 1
    }
fi

config_source="$project_dir/config/omatorsurf.toml"
if [[ -e /etc/omatorsurf/config.toml || -L /etc/omatorsurf/config.toml ]]; then
    config_source=/etc/omatorsurf/config.toml
fi
generated_torrc="$(mktemp)"
trap 'rm -f -- "$generated_torrc"' EXIT
"$binary" --config "$config_source" tor config > "$generated_torrc"

# Refuse configuration drift before installing any files.
if [[ -e /etc/omatorsurf/torrc || -L /etc/omatorsurf/torrc ]]; then
    if ! cmp -s -- "$generated_torrc" /etc/omatorsurf/torrc; then
        printf '%s\n' 'Existing /etc/omatorsurf/torrc differs from the selected configuration. Back it up and review it before replacing it. No files were installed.' >&2
        exit 1
    fi
fi
service_target=/etc/systemd/system/omatorsurf-tor.service
if [[ -e "$service_target" || -L "$service_target" ]]; then
    if ! cmp -s -- "$project_dir/systemd/omatorsurf-tor.service" "$service_target"; then
        printf '%s\n' "Existing $service_target differs. Back it up and review it before replacing it. No files were installed." >&2
        exit 1
    fi
fi

for unit in omatorsurf.service omatorsurf-guard.service; do
    if [[ -e /etc/systemd/system/$unit || -L /etc/systemd/system/$unit ]]; then
        cmp -s "$project_dir/systemd/$unit" "/etc/systemd/system/$unit" || {
            printf '%s\n' "Existing $unit differs; back it up and review before replacing it. No files were installed." >&2; exit 1;
        }
    fi
done
install -d -o root -g root -m 0755 /etc/omatorsurf /usr/local/bin /usr/share/omatorsurf
if [[ "$config_source" != /etc/omatorsurf/config.toml ]]; then
    install -o root -g root -m 0644 -- "$config_source" /etc/omatorsurf/config.toml
fi
if [[ ! -e /etc/omatorsurf/torrc && ! -L /etc/omatorsurf/torrc ]]; then
    install -o root -g root -m 0644 -- "$generated_torrc" /etc/omatorsurf/torrc
fi
if [[ ! -e "$service_target" && ! -L "$service_target" ]]; then
    install -o root -g root -m 0644 -- "$project_dir/systemd/omatorsurf-tor.service" "$service_target"
fi
install -o root -g root -m 0755 -- "$binary" /usr/local/bin/omatorsurf
install -o root -g root -m 0644 -- "$project_dir/nftables/omatorsurf.nft" /usr/share/omatorsurf/omatorsurf.nft
for unit in omatorsurf.service omatorsurf-guard.service; do
    install -o root -g root -m 0644 -- "$project_dir/systemd/$unit" "/etc/systemd/system/$unit"
done
install -d -o root -g root -m 0755 /run/omatorsurf
systemctl daemon-reload
printf '%s\n' 'Files installed. No services were started or enabled; no firewall rules were applied.' 'Next: pkexec /usr/local/bin/omatorsurf start'
