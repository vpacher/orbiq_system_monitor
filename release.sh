#!/bin/bash
# Builds, tests and publishes a GitHub release for the version in Cargo.toml.
#
#   ./release.sh            full release: gate, tag v<version>, push, gh release
#   ./release.sh --dry-run  everything except tag/push/release; shows the notes
#
# Gate (any failure stops the release): build-and-package.sh (clippy -D
# warnings, unit tests, amd64 + arm64 cross builds, .debs), then
# test-package.sh (install/upgrade/purge tests in a Debian container).
# Assets: both .debs, both raw binaries, SHA256SUMS (sha256sum -c format).
set -euo pipefail

DRY_RUN=false
[ "${1:-}" = "--dry-run" ] && DRY_RUN=true

PROJECT=orbiq_system_monitor
VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
TAG="v$VERSION"
DIST="dist/$VERSION"

die() { echo "release: $*" >&2; exit 1; }

# --- preconditions -----------------------------------------------------------
[ "$(git symbolic-ref --short HEAD)" = main ] || die "not on main"
[ -z "$(git status --porcelain)" ] || die "working tree not clean"
git fetch -q origin main --tags
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] \
    || die "main is not in sync with origin/main (push or pull first)"
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    die "tag $TAG already exists - bump the version in Cargo.toml"
fi
gh auth status >/dev/null 2>&1 || die "gh is not logged in (gh auth login)"

# Previous release tag (v-prefixed or the old bare 0.1.37 style), for the notes.
PREV_TAG=$(git tag -l --sort=-v:refname | grep -E '^v?[0-9]+\.[0-9]+\.[0-9]+$' | head -1 || true)

# --- gate --------------------------------------------------------------------
CROSS_CONTAINER_ENGINE="${CROSS_CONTAINER_ENGINE:-podman}" ./build-and-package.sh
./test-package.sh

# --- assets ------------------------------------------------------------------
rm -rf "$DIST"
mkdir -p "$DIST"
cp "packages/${PROJECT}_${VERSION}_amd64.deb" "packages/${PROJECT}_${VERSION}_arm64.deb" "$DIST/"
cp "releases/${PROJECT}-${VERSION}-amd64" "$DIST/${PROJECT}-${VERSION}-linux-amd64"
cp "releases/${PROJECT}-${VERSION}-aarch64" "$DIST/${PROJECT}-${VERSION}-linux-arm64"
(cd "$DIST" && shasum -a 256 -- * > SHA256SUMS)

# --- notes -------------------------------------------------------------------
NOTES="$DIST/RELEASE_NOTES.md"
{
    echo "## Changes"
    echo
    if [ -n "$PREV_TAG" ]; then
        git log --no-merges --format='- %s' "$PREV_TAG..HEAD"
    else
        git log --no-merges --format='- %s'
    fi
    echo
    echo "## Install (Debian / Ubuntu / Raspberry Pi OS)"
    echo
    echo '```bash'
    echo "sudo apt install ./${PROJECT}_${VERSION}_amd64.deb   # or _arm64.deb"
    echo '```'
    echo
    echo "The package asks for the MQTT broker, credentials and device name on first install."
    echo "Upgrades keep the existing configuration."
    echo
    echo "## Verify"
    echo
    echo '```bash'
    echo "sha256sum -c SHA256SUMS --ignore-missing"
    echo '```'
} > "$NOTES"

echo
echo "=== $TAG (previous: ${PREV_TAG:-none})"
cat "$NOTES"
echo
echo "=== assets"
cat "$DIST/SHA256SUMS"

if $DRY_RUN; then
    echo
    echo "Dry run: no tag, no push, no release. Assets are in $DIST/"
    exit 0
fi

# --- publish -----------------------------------------------------------------
git tag -a "$TAG" -m "OrbIQ System Monitor $VERSION"
git push -q origin "$TAG"
(cd "$DIST" && gh release create "$TAG" \
    --title "OrbIQ System Monitor $VERSION" \
    --notes-file RELEASE_NOTES.md \
    --latest \
    ./*.deb ./"${PROJECT}-${VERSION}"-linux-* SHA256SUMS)
echo "Released: $(gh release view "$TAG" --json url --jq .url)"
