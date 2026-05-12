#!/usr/bin/env bash
# Build a Debian package for ghcp-mon (amd64).
#
# Run from the repo root: ./scripts/deb.sh
#
# Output: ./dist/ghcp-mon_<version>_amd64.deb
#
# The package installs a user systemd unit at
# /usr/lib/systemd/user/ghcp-mon.service and a conffile at
# /etc/default/ghcp-mon. The service is NOT enabled on install — see postinst.

set -euo pipefail

# ---- locate repo root ----
if [[ ! -f Cargo.toml || ! -f Containerfile || ! -d debian ]]; then
    echo "error: run this script from the repo root (./scripts/deb.sh)" >&2
    exit 1
fi

# ---- pick container engine ----
if command -v podman >/dev/null 2>&1; then
    ENGINE=podman
elif command -v docker >/dev/null 2>&1; then
    ENGINE=docker
else
    echo "error: neither podman nor docker found in PATH" >&2
    exit 1
fi

# ---- extract version from Cargo.toml ----
VERSION=$(awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/          { in_pkg = 0 }
    in_pkg && /^version[[:space:]]*=/ {
        n = split($0, a, "\"")
        if (n >= 3) { print a[2]; exit }
    }
' Cargo.toml)

if [[ -z "$VERSION" ]]; then
    echo "error: could not read [package].version from Cargo.toml" >&2
    exit 1
fi

echo ">> packaging ghcp-mon ${VERSION} (.deb / amd64)"

# ---- build ----
mkdir -p dist
echo ">> $ENGINE build --target deb -> ./dist"
"$ENGINE" build --target deb -o type=local,dest=./dist -f Containerfile .

# cargo-deb writes target/debian/<name>_<version>_<arch>.deb. The `deb` stage
# copies that whole directory to /, so it lands at ./dist/<name>_<version>_<arch>.deb.
deb_path="dist/ghcp-mon_${VERSION}-1_amd64.deb"
if [[ ! -f "$deb_path" ]]; then
    # cargo-deb may omit the `-1` revision; accept either.
    alt="dist/ghcp-mon_${VERSION}_amd64.deb"
    if [[ -f "$alt" ]]; then
        deb_path="$alt"
    else
        echo "error: expected .deb missing — looked for:" >&2
        echo "  $deb_path" >&2
        echo "  $alt" >&2
        ls -la dist/ >&2
        exit 1
    fi
fi

echo
echo "done: $deb_path"
ls -lh "$deb_path"

if command -v dpkg-deb >/dev/null 2>&1; then
    echo
    echo "---- dpkg-deb -I ----"
    dpkg-deb -I "$deb_path"
    echo
    echo "---- dpkg-deb -c ----"
    dpkg-deb -c "$deb_path"
fi

cat <<EOF

Install (as root):
    sudo apt install ./$deb_path

Then, as the user that should run the service:
    systemctl --user daemon-reload
    systemctl --user enable --now ghcp-mon.service
EOF
