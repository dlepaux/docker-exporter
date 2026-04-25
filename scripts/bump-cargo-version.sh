#!/bin/bash
# Bump version in Cargo.toml and Cargo.lock (called by semantic-release)
set -euo pipefail

VERSION="$1"

# Cargo.toml — replace the package version (first occurrence)
sed -i.bak "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml && rm -f Cargo.toml.bak

# Cargo.lock — replace version only for our package (line after name)
awk -v ver="$VERSION" '
  /^name = "docker-exporter"/ { print; getline; print "version = \"" ver "\""; next }
  { print }
' Cargo.lock > Cargo.lock.tmp && mv Cargo.lock.tmp Cargo.lock
