#!/usr/bin/env bash
# Build ama from this checkout and install it.
#
# Most people do not need this file. The quickest install needs no clone:
#
#   curl --proto '=https' --tlsv1.2 -LsSf \
#     https://github.com/hongchenme/ask-me-anything/releases/latest/download/ama-installer.sh | sh
#   ama setup
#
# This script is the from-source equivalent: it builds, copies the binary into
# place, and then hands off to `ama setup` for everything else — the `@@`
# alias, a starter config, and your shell rc. Keeping that logic in the binary
# means the curl path and the clone path cannot drift apart.
set -euo pipefail

cd "$(dirname "$0")"
dest="${AMA_PREFIX:-$HOME/.local/bin}"

cargo build --release
mkdir -p "$dest"
install -m 0755 target/release/ama "$dest/ama"
echo "Installed $dest/ama"

exec "$dest/ama" setup "$@"
