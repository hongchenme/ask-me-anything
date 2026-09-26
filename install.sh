#!/usr/bin/env bash
# Build and install ama, plus the `@@` trigger, into ~/.local/bin.
set -euo pipefail

cd "$(dirname "$0")"
dest="${AMA_PREFIX:-$HOME/.local/bin}"

cargo build --release
mkdir -p "$dest"
install -m 0755 target/release/ama "$dest/ama"
install -m 0755 target/release/ama "$dest/@@"

shell_name=$(basename "${SHELL:-bash}")
case "$shell_name" in
    zsh) rc="$HOME/.zshrc" ;;
    *) rc="$HOME/.bashrc"; shell_name=bash ;;
esac

line="eval \"\$($dest/ama init $shell_name)\""
if ! grep -qF "ama init $shell_name" "$rc" 2>/dev/null; then
    printf '\n# ama -- ask me anything\n%s\n' "$line" >> "$rc"
    echo "Added integration to $rc"
else
    echo "Integration already present in $rc"
fi

mkdir -p "$HOME/.qmx2"
if [[ ! -f "$HOME/.qmx2/config.yml" ]]; then
    printf 'agent: claude --model opus --effort high\n' > "$HOME/.qmx2/config.yml"
    echo "Wrote a starter config to ~/.qmx2/config.yml"
fi

echo
echo "Installed. Start a new shell, then try:  @@ what is this project about"
"$dest/ama" doctor || true
