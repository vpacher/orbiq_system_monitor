#!/bin/bash
set -e

PROJECT_NAME="orbiq_system_monitor"
VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)

echo "Creating Debian packages for ${PROJECT_NAME} v${VERSION}..."

# Create output directory
mkdir -p packages

# Package for x86_64 (amd64) - using pre-built binary
echo "Packaging for amd64..."
if [ ! -f "target/x86_64-unknown-linux-gnu/release/${PROJECT_NAME}" ]; then
    echo "Error: x86_64 binary not found. Run build-releases.sh first."
    exit 1
fi
cargo deb --target x86_64-unknown-linux-gnu --no-build
mv target/x86_64-unknown-linux-gnu/debian/*.deb packages/${PROJECT_NAME}_${VERSION}_amd64.deb

# Package for aarch64 (arm64) - using pre-built binary
echo "Packaging for arm64..."
if [ ! -f "target/aarch64-unknown-linux-gnu/release/${PROJECT_NAME}" ]; then
    echo "Error: aarch64 binary not found. Run build-releases.sh first."
    exit 1
fi
cargo deb --target aarch64-unknown-linux-gnu --no-build
mv target/aarch64-unknown-linux-gnu/debian/*.deb packages/${PROJECT_NAME}_${VERSION}_arm64.deb

# Cleanup old packages - keep only the last 3 versions
echo "Cleaning up old packages..."
cleanup_old_packages() {
    local arch=$1
    local pattern="${PROJECT_NAME}_*_${arch}.deb"
    
    echo "  Looking for ${arch} packages..."
    
    # Check if any files match the pattern first
    if ! ls packages/${pattern} >/dev/null 2>&1; then
        echo "  No ${arch} packages found, no cleanup needed"
        return
    fi
    
    # Extract versions and sort them
    local versions=()
    for file in packages/${pattern}; do
        if [ -f "$file" ]; then
            # Extract version from filename: orbiq_system_monitor_0.1.37_amd64.deb -> 0.1.37
            local version=$(basename "$file" | sed "s/${PROJECT_NAME}_//" | sed "s/_${arch}.deb//")
            versions+=("$version")
        fi
    done
    
    if [ ${#versions[@]} -eq 0 ]; then
        echo "  No valid ${arch} packages found"
        return
    fi
    
    echo "  Found versions: ${versions[*]}"
    
    # Sort versions in descending order (newest first) using version sort
    local sorted_versions=($(printf '%s\n' "${versions[@]}" | sort -V -r))
    echo "  Sorted versions (newest first): ${sorted_versions[*]}"
    
    # Keep only the first 3 versions, delete the rest
    if [ ${#sorted_versions[@]} -gt 3 ]; then
        echo "  Found ${#sorted_versions[@]} ${arch} packages, keeping 3 newest..."
        for ((i=3; i<${#sorted_versions[@]}; i++)); do
            local old_file="packages/${PROJECT_NAME}_${sorted_versions[i]}_${arch}.deb"
            echo "    Removing old package: $(basename "$old_file")"
            rm -f "$old_file"
        done
    else
        echo "  Found ${#sorted_versions[@]} ${arch} packages, no cleanup needed"
    fi
}

# Cleanup for both architectures
cleanup_old_packages "amd64"
cleanup_old_packages "arm64"

# Verify packages and show info
echo ""
echo "Packaging completed successfully!"
echo "Package artifacts:"
for file in "packages/${PROJECT_NAME}_${VERSION}_"*.deb; do
    if [ -f "$file" ]; then
        echo "  $(basename "$file"): $(du -h "$file" | cut -f1)"
        # Show package info
        dpkg-deb --info "$file" | grep -E "Package|Version|Architecture|Description" | sed 's/^/    /'
    fi
done

echo ""
echo "All packages in packages directory:"
ls -la packages/

echo ""
echo "Package history (newest first by version):"
echo "AMD64 packages:"
if ls packages/${PROJECT_NAME}_*_amd64.deb >/dev/null 2>&1; then
    for file in packages/${PROJECT_NAME}_*_amd64.deb; do
        version=$(basename "$file" | sed "s/${PROJECT_NAME}_//" | sed "s/_amd64.deb//")
        echo "  $version: $(basename "$file")"
    done | sort -V -r -k1,1 | head -3
else
    echo "  No AMD64 packages found"
fi
echo "ARM64 packages:"
if ls packages/${PROJECT_NAME}_*_arm64.deb >/dev/null 2>&1; then
    for file in packages/${PROJECT_NAME}_*_arm64.deb; do
        version=$(basename "$file" | sed "s/${PROJECT_NAME}_//" | sed "s/_arm64.deb//")
        echo "  $version: $(basename "$file")"
    done | sort -V -r -k1,1 | head -3
else
    echo "  No ARM64 packages found"
fi

echo ""
echo "Installation commands:"
echo "  AMD64: sudo dpkg -i packages/${PROJECT_NAME}_${VERSION}_amd64.deb"
echo "  ARM64: sudo dpkg -i packages/${PROJECT_NAME}_${VERSION}_arm64.deb"