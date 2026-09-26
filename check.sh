#!/usr/bin/env bash
# Repository feedback loop. Run from the repository root:  ./check.sh
set -uo pipefail
cd "$(dirname "$0")"

BOLD=$'\033[1m'; GREEN=$'\033[32m'; RED=$'\033[31m'; DIM=$'\033[2m'; RESET=$'\033[0m'
fail=0

step() {
    local label=$1; shift
    printf '  %-34s' "$label"
    if output=$("$@" 2>&1); then
        printf '%sok%s\n' "$GREEN" "$RESET"
    else
        printf '%sFAILED%s\n' "$RED" "$RESET"
        printf '%s\n' "$output" | sed 's/^/      /'
        fail=1
    fi
}

printf '\n%s\n' "${BOLD}ama -- checks${RESET}"
printf '%s\n\n' "${DIM}$(rustc --version)${RESET}"

step "rustfmt"            cargo fmt --all -- --check
step "clippy"             cargo clippy --lib --bins -- -D warnings \
                              -D clippy::unwrap_used -D clippy::expect_used
step "clippy (tests)"     cargo clippy --tests -- -D warnings
step "bash syntax"        bash -n crates/ama/src/shell/ama.bash
step "install.sh syntax"  bash -n install.sh
if command -v zsh >/dev/null; then
    step "zsh syntax"     zsh -n crates/ama/src/shell/ama.zsh
fi
if command -v shellcheck >/dev/null; then
    step "shellcheck"     shellcheck crates/ama/src/shell/ama.bash install.sh
else
    printf '  %-34s%sskipped (shellcheck not installed)%s\n' "shellcheck" "$DIM" "$RESET"
fi
step "build"              cargo build --all-targets
step "tests"              cargo test -- --test-threads=1

printf '\n'
if [[ $fail -eq 0 ]]; then
    printf '  %sAll checks passed.%s\n\n' "$GREEN" "$RESET"
else
    printf '  %sSomething failed. See above.%s\n\n' "$RED" "$RESET"
fi
exit "$fail"
