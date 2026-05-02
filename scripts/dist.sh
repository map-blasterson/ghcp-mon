#!/usr/bin/env bash
# Build, package and tag a ghcp-mon release.
#
# Run from the repo root: ./scripts/dist.sh
#
# Steps:
#   1. Verify Cargo.toml and web/package.json carry the same version.
#   2. Verify that version is strictly greater than the latest `vX.Y.Z` git tag.
#   3. Build all three OS targets via the Containerfile `dist` stage.
#   4. Package the artifacts (tar.gz for unix, zip for windows).
#   5. Create an annotated `vX.Y.Z` git tag.

set -euo pipefail

# ---- locate repo root ----
if [[ ! -f Cargo.toml || ! -f web/package.json || ! -f Containerfile ]]; then
    echo "error: run this script from the repo root (./scripts/dist.sh)" >&2
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

# ---- extract versions ----
cargo_version=$(awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/          { in_pkg = 0 }
    in_pkg && /^version[[:space:]]*=/ {
        n = split($0, a, "\"")
        if (n >= 3) { print a[2]; exit }
    }
' Cargo.toml)

web_version=$(node -e 'process.stdout.write(require("./web/package.json").version)')

if [[ -z "$cargo_version" ]]; then
    echo "error: could not read [package].version from Cargo.toml" >&2
    exit 1
fi
if [[ -z "$web_version" ]]; then
    echo "error: could not read .version from web/package.json" >&2
    exit 1
fi

if [[ "$cargo_version" != "$web_version" ]]; then
    echo "error: version mismatch — Cargo.toml=$cargo_version, web/package.json=$web_version" >&2
    exit 1
fi

VERSION="$cargo_version"
TAG="v$VERSION"

# ---- semver sanity check ----
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: version '$VERSION' is not X.Y.Z" >&2
    exit 1
fi

# ---- compare against last tag ----
last_tag=$(git tag --list 'v[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname | head -n1 || true)
if [[ -n "$last_tag" ]]; then
    last_ver="${last_tag#v}"
    # `sort -V` highest must be the new version, and it must differ from the old.
    highest=$(printf '%s\n%s\n' "$VERSION" "$last_ver" | sort -V | tail -n1)
    if [[ "$VERSION" == "$last_ver" || "$highest" != "$VERSION" ]]; then
        echo "error: version $VERSION is not greater than last tag $last_tag" >&2
        exit 1
    fi
fi

if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    echo "error: tag $TAG already exists" >&2
    exit 1
fi

echo ">> packaging ghcp-mon $TAG (last tag: ${last_tag:-<none>})"

# ---- build ----
rm -rf dist
mkdir -p dist
echo ">> $ENGINE build --target dist -> ./dist"
"$ENGINE" build --target dist -o type=local,dest=./dist -f Containerfile .

for f in dist/ghcp-mon dist/ghcp-mon.exe dist/ghcp-mon-darwin; do
    if [[ ! -f "$f" ]]; then
        echo "error: expected artifact $f missing after build" >&2
        exit 1
    fi
done

# ---- package ----
PKG_DIR="dist/packages"
mkdir -p "$PKG_DIR"

linux_tar="$PKG_DIR/ghcp-mon-${VERSION}-linux-x86_64.tar.gz"
darwin_tar="$PKG_DIR/ghcp-mon-${VERSION}-darwin-universal2.tar.gz"
windows_zip="$PKG_DIR/ghcp-mon-${VERSION}-windows-x86_64.zip"

echo ">> tar  $linux_tar"
tar -czf "$linux_tar" --transform 's,^,ghcp-mon/,' -C dist ghcp-mon

echo ">> tar  $darwin_tar"
# rename darwin binary to plain `ghcp-mon` inside the archive
tar -czf "$darwin_tar" --transform 's,^ghcp-mon-darwin$,ghcp-mon/ghcp-mon,' -C dist ghcp-mon-darwin

echo ">> zip  $windows_zip"
( cd dist && zip -j -q "../$windows_zip" ghcp-mon.exe )

# ---- checksums ----
checksums="$PKG_DIR/SHA256SUMS"
echo ">> sha256 $checksums"
( cd "$PKG_DIR" && sha256sum -- *.tar.gz *.zip > "$(basename "$checksums")" )
cat "$checksums"

# ---- tag ----
echo ">> git tag -a $TAG"
git tag -a "$TAG" -m "ghcp-mon $TAG"

echo
echo "done. artifacts:"
ls -lh "$PKG_DIR"
echo
echo "next: git push origin $TAG"
