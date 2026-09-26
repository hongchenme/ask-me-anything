#!/usr/bin/env bash
# Build and install ama, plus the `@@` trigger, into ~/.local/bin.
set -euo pipefail

cd "$(dirname "$0")"
dest="${AMA_PREFIX:-$HOME/.local/bin}"

cargo build --release
mkdir -p "$dest"
install -m 0755 target/release/ama "$dest/ama"
install -m 0755 target/release/ama "$dest/@@"

# R32: `ama init` only knows bash and zsh, so anything else gets the bash
# integration written to ~/.bashrc and a cheerful "Installed" -- a fish user
# used to end up with nothing that works and nothing that said so. Still
# install (the `ama` and `@@` binaries are useful on their own, REQ-27), but
# say plainly that the Enter trigger will not be live.
shell_name=$(basename "${SHELL:-bash}")
case "$shell_name" in
    zsh) rc="$HOME/.zshrc" ;;
    bash) rc="$HOME/.bashrc" ;;
    *)
        echo "Warning: \$SHELL is '${SHELL:-unset}'. ama's Enter trigger supports bash and zsh only," >&2
        echo "         so '@@ <question>' will not fire in $shell_name. Installing the bash" >&2
        echo "         integration into ~/.bashrc anyway; 'ama ask -- <question>' works in any shell." >&2
        rc="$HOME/.bashrc"
        shell_name=bash
        ;;
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

# R33: `$dest` is quoted inside the generated line too. Unquoted, a prefix
# containing a space (`AMA_PREFIX="$HOME/my tools/bin"`) writes an rc line
# that word-splits on every later shell start.
line="eval \"\$(\"$dest/ama\" init $shell_name)\""
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
# R34: the hook is not live in *this* process, so the report below always
# says `integration  not loaded`. Correct, but straight after "Added
# integration to ~/.bashrc" it reads as a failed install.
echo "(\`ama doctor\` reports \`integration  not loaded\` here -- expected until you open a"
echo " new shell. Run it again there to confirm the trigger is live.)"
"$dest/ama" doctor || true
