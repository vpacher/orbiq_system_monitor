#!/bin/bash
set -e

echo "Building and packaging temp_daemon for multiple architectures..."

# Build the binaries
echo "Step 1: Building binaries..."
./build-releases.sh

# Package into .deb files
echo ""
echo "Step 2: Creating .deb packages..."
./package-deb.sh

echo ""
echo "Build and packaging completed successfully!"
echo "Binary releases are in: releases/"
echo "Debian packages are in: releases/deb/"
