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

# R20: `ama init`'s own output never needs $dest on PATH -- it's eval'd
# below by absolute path -- but the hook it installs does: ama.bash rewrites
# READLINE_LINE (and ama.zsh, BUFFER) to a bare "${prefix}@@ ...", and both
# call bare `ama session reset`. Without $dest on PATH, every `@@` after a
# successful-looking install fails with "command not found". Fix it, don't
# just warn; prepend it so it lands before the eval line below. Guarded the
# same way as that eval line, so a second run does not duplicate it.
case ":$PATH:" in
    *":$dest:"*)
        echo "$dest is already on PATH"
        ;;
    *)
        if ! grep -qF "PATH=\"$dest:" "$rc" 2>/dev/null; then
            printf '\n# ama -- put %s on PATH\nexport PATH="%s:$PATH"\n' "$dest" "$dest" >> "$rc"
            echo "Added $dest to PATH in $rc"
        else
            echo "PATH entry for $dest already present in $rc"
        fi
        ;;
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
echo "Installed. Open a new shell, then try:  @@ what is this project about"
echo "(\`ama doctor\` will confirm the integration is live.)"
"$dest/ama" doctor || true
