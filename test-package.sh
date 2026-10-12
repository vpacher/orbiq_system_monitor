#!/bin/sh
# Builds the arm64 .deb (no recompile; run build-releases.sh first) and runs
# debian/tests/package-tests.sh in a debian:bookworm container: fresh install,
# preseeded install, upgrades over existing/symlinked/placeholder configs,
# purge, and an interactive run with typed answers.
# Needs podman (or CONTAINER_ENGINE=docker) and packages/orbiq_system_monitor_0.3.0_arm64.deb.
set -e

ENGINE="${CONTAINER_ENGINE:-podman}"
OLD_DEB="${OLD_DEB:-packages/orbiq_system_monitor_0.3.0_arm64.deb}"

VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
cargo deb --target aarch64-unknown-linux-gnu --no-build >/dev/null
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp "target/aarch64-unknown-linux-gnu/debian/orbiq-system-monitor_${VERSION}-1_arm64.deb" "$work/new.deb"
cp "$OLD_DEB" "$work/old.deb"
cp debian/tests/package-tests.sh "$work/tests.sh"

"$ENGINE" run --rm --platform linux/arm64 -v "$work:/t:ro" --hostname testbox \
    docker.io/library/debian:bookworm sh /t/tests.sh | tee "$work/result.txt"
grep -q "RESULT: .* 0 failed" "$work/result.txt"
