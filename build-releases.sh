#!/bin/bash
set -e

PROJECT_NAME="orbiq_system_monitor"
VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)

echo "Building ${PROJECT_NAME} v${VERSION} for multiple targets..."

# Create output directory
mkdir -p releases

# Build for x86_64 Linux (amd64)
echo "Building for x86_64-unknown-linux-gnu (amd64)..."
cross build --release --target x86_64-unknown-linux-gnu
cp "target/x86_64-unknown-linux-gnu/release/${PROJECT_NAME}" "releases/${PROJECT_NAME}-${VERSION}-amd64"

# Build for aarch64 Linux (arm64/Raspberry Pi)
echo "Building for aarch64-unknown-linux-gnu (aarch64)..."
cross build --release --target aarch64-unknown-linux-gnu
cp "target/aarch64-unknown-linux-gnu/release/${PROJECT_NAME}" "releases/${PROJECT_NAME}-${VERSION}-aarch64"

# Cleanup old releases - keep only the last 3 versions
echo "Cleaning up old releases..."
cleanup_old_releases() {
    local arch=$1
    local pattern="${PROJECT_NAME}-*-${arch}"

    echo "  Debug: Looking for pattern '${pattern}' in releases directory..."

    # Check if any files match the pattern first
    if ! ls releases/${pattern} >/dev/null 2>&1; then
        echo "  Debug: No files found matching pattern"
        echo "Found 0 ${arch} releases, no cleanup needed"
        return
    fi

    # Use array to collect files, sorted by time
    local files=()
    while IFS= read -r -d '' file; do
        files+=("$file")
    done < <(find releases -name "${pattern}" -print0 2>/dev/null | xargs -0 ls -t 2>/dev/null || true)

    echo "  Debug: Found ${#files[@]} files: ${files[*]}"

    if [ ${#files[@]} -gt 3 ]; then
        echo "Found ${#files[@]} ${arch} releases, keeping 3 newest..."
        for ((i=3; i<${#files[@]}; i++)); do
            echo "  Removing old release: ${files[i]}"
            rm -f "${files[i]}"
        done
    else
        echo "Found ${#files[@]} ${arch} releases, no cleanup needed"
    fi
}

# Cleanup for both architectures
cleanup_old_releases "amd64"
cleanup_old_releases "aarch64"

# Verify builds and show file info
echo "Builds completed successfully!"
echo "Build artifacts:"
for file in "releases/${PROJECT_NAME}-${VERSION}-"*; do
    if [ -f "$file" ]; then
        echo "  $(basename "$file"): $(file "$file" | cut -d: -f2-)"
        echo "    Size: $(du -h "$file" | cut -f1)"
    fi
done

echo ""
echo "All files in releases directory:"
ls -la releases/

echo ""
echo "Release history (newest first):"
echo "AMD64 releases:"
if ls releases/${PROJECT_NAME}-*-amd64 >/dev/null 2>&1; then
    ls -t releases/${PROJECT_NAME}-*-amd64 | head -3
else
    echo "  No AMD64 releases found"
fi
echo "ARM64 releases:"
if ls releases/${PROJECT_NAME}-*-aarch64 >/dev/null 2>&1; then
    ls -t releases/${PROJECT_NAME}-*-aarch64 | head -3
else
    echo "  No ARM64 releases found"
fi