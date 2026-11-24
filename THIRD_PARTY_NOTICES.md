# Dependencies and licensing

Omatorsurf source and widget are MIT licensed; see LICENSE. The root plugin
manifest and the local installer manifest describe the same MIT plugin.

The Rust dependency graph is pinned by Cargo.lock. Direct dependencies are clap,
serde, serde_json, toml, thiserror, tracing and tracing-subscriber. Their upstream
license terms also apply to compiled distributions. Cargo obtains the locked
crates during a source build. No third-party binary release is bundled here.

Runtime dependencies are separately installed system packages: Tor, nftables,
iproute2, curl, Python 3, python-stem, systemd, polkit/pkexec, Bash, coreutils and
util-linux. The desktop additionally requires Omarchy Quattro/Quickshell, its Qt
imports and an active graphical polkit agent. Git and Rust/Cargo are build/setup
tools. Each dependency retains its upstream license; installing Omatorsurf does
not relicense those packages. Use the distribution's package metadata and crate
source license files for their exact terms.

The widget uses the host's Nerd Font glyphs and theme/UI components; it does not
bundle fonts or third-party artwork. No preview artwork is included in this draft.
