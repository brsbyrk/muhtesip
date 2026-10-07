#!/usr/bin/env bash
#
# Package the built `muhtesip` binary for one release target: an archive plus a checksum.
#
# Called by .github/workflows/release.yml, and runnable by hand — a release pipeline that only ever runs
# on a tag is a pipeline nobody can test, and the naming here is the part most likely to be subtly wrong.
# That is why this is a script and not a shell block inside the workflow: one derivation, runnable in
# both places.
#
# One format everywhere, on purpose. `tar.gz` is not the friendliest shape on Windows, but `tar` is
# present on every runner and on Windows itself, while `zip` is present on some and assumed on others —
# and an archive step that fails on one platform fails the release for it. `muhtesip.exe` comes out of the
# Windows archive with its extension intact.
#
# Usage: scripts/package-release.sh <target-triple> <tag> [<binary-path>]
#   The binary path defaults to target/<target>/release/muhtesip(.exe) — a cross build — and falls back to
#   target/release/muhtesip(.exe), which is where a native `cargo build --release` puts it.

set -euo pipefail

target="${1:?usage: package-release.sh <target-triple> <tag> [<binary-path>]}"
tag="${2:?usage: package-release.sh <target-triple> <tag> [<binary-path>]}"

# The binary carries the platform's extension.
case "$target" in
    *windows*) executable="muhtesip.exe" ;;
    *) executable="muhtesip" ;;
esac

binary="${3:-target/$target/release/$executable}"
if [ ! -f "$binary" ]; then
    binary="target/release/$executable"
fi
[ -f "$binary" ] || {
    echo "no binary for $target: looked in target/$target/release and target/release" >&2
    exit 2
}

name="muhtesip-$tag-$target"
archive="dist/$name.tar.gz"
mkdir -p dist
cp "$binary" "dist/$executable"
tar -czf "$archive" -C dist "$executable"

# `sha256sum` is GNU; macOS ships `shasum`. Whichever exists, the file has the same shape.
if command -v sha256sum >/dev/null 2>&1; then
    (cd dist && sha256sum "$name.tar.gz" > "$name.sha256")
else
    (cd dist && shasum -a 256 "$name.tar.gz" > "$name.sha256")
fi

rm -f "dist/$executable"
echo "$archive"
