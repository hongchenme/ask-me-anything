# ama shell integration (bash). Sourced via: eval "$(ama init bash)"

# Single-quote a string so the shell parses it back byte-for-byte.
# Inside '...' every byte is literal; ' is the only character that can end
# the quote, so it is the only one needing treatment.
__ama_sq() {
    printf "'%s'" "${1//\'/\'\\\'\'}"
}

# Split a line into prefix and prompt at the trigger.
# Prints "<prefix><US><prompt>" and returns 0 when the line is triggered,
# where <US> is the 0x1f unit separator. Returns 1 otherwise.
# Recognition order is fixed (ADR-006, amended by A-03): line start wins
# over an operator, and among operator-triggered lines the FIRST trigger
# wins. So both "@@ compare a && @@ b" and "clear && @@ compare a && @@ b"
# keep "compare a && @@ b" whole, as one prompt — POSIX ERE has no lazy
# quantifier, so this is done with parameter expansion (shortest/longest
# match), not a greedy regex.
#
# Rule 2 only inspects the *text* before the trigger, and a `;`, `&` or `|`
# inside a quoted word is not a command separator at all (R26): `echo
# 'clear && @@ joke'` and `echo "a; @@ not a prompt"` are ordinary commands
# that an uninstrumented shell simply prints, and REQ-03 makes behaving
# identically non-negotiable for anything bound to Enter. Left unguarded,
# the first was silently rewritten into a corrupted string and the second
# left the shell sitting at PS2. Counting quotes is enough to fail closed:
# an odd `'` or `"` count before the trigger means the prefix cannot be a
# complete command, so the line is passed through untouched. A real trigger
# hidden behind an unbalanced quote degrades to the plain `@@` binary --
# the safe direction.
__ama_split() {
    local line=$1 lead trimmed head tail_re sq dq nsq ndq
    lead=${line%%[![:space:]]*}
    trimmed=${line#"$lead"}

    if [[ $trimmed == '@@ '* ]]; then
        printf '%s\x1f%s' "$lead" "${trimmed#'@@ '}"
        return 0
    fi
    if [[ $line == *"@@ "* ]]; then
        head=${line%%"@@ "*}
        tail_re='[;&|][[:space:]]*$'
        # Discard everything that is not a quote, then count what is left.
        sq=${head//[!\']/}
        dq=${head//[!\"]/}
        nsq=${#sq}
        ndq=${#dq}
        if [[ $head =~ $tail_re ]] && (( nsq % 2 == 0 && ndq % 2 == 0 )); then
            printf '%s\x1f%s' "$head" "${line#*"@@ "}"
            return 0
        fi
    fi
    return 1
}

# ---- interactive integration ------------------------------------------------
# Nothing to do in a non-interactive shell (REQ-24). A guard that returns or
# exits here is wrong (R18, finding 1): line 1 of this file documents
# `eval "$(ama init bash)"` as the install path, and under `eval` a bare
# `return` is not inside a function or a sourced script, so it fails and
# `|| exit` fires instead -- killing the calling shell outright. Verified:
# `bash -c 'eval "$(cat ama.bash)"; echo REACHED'` prints nothing. Sourced
# from a non-interactive `.bashrc` (an ssh remote command, `$BASH_ENV`) it
# is worse: `return` succeeds, but returns from the *rc file* itself,
# silently skipping every later line for the rest of that file. An `if`
# just skips its own body in every calling context, disturbing nothing
# before or after it.
if [[ $- == *i* ]]; then

# A stable per-shell identity for the transcript fallback.
export AMA_SESSION="$$"

__ama_clears_screen() {
    case $1 in
        clear | reset | 'clear '* | 'reset '* | *'tput clear'*) return 0 ;;
        *) return 1 ;;
    esac
}

__ama_hook() {
    local split prefix prompt
    if split=$(__ama_split "$READLINE_LINE"); then
        prefix=${split%%$'\x1f'*}
        prompt=${split#*$'\x1f'}
        # A blank prompt is a no-op, not an agent call (REQ-06).
        if [[ -z ${prompt//[[:space:]]/} ]]; then
            READLINE_LINE=""
            READLINE_POINT=0
            return 0
        fi
        # `clear && @@ ...`: drop the transcript now, since the pane-scrape
        # path resets itself but the fallback path has no other signal.
        __ama_clears_screen "$prefix" && ama session reset >/dev/null 2>&1
        READLINE_LINE="${prefix}@@ $(__ama_sq "$prompt")"
        READLINE_POINT=${#READLINE_LINE}
        return 0
    fi
    __ama_clears_screen "$READLINE_LINE" && ama session reset >/dev/null 2>&1
    return 0
}

# Ctrl-L: readline's own clear-screen preserves and redraws whatever the
# user was typing, so this must too -- clearing READLINE_LINE here would
# throw the in-progress line away. `3J` also clears scrollback, matching
# what a user means by Ctrl-L.
#
# This deliberately does NOT touch READLINE_LINE/READLINE_POINT. It once
# self-assigned them, hedging an unreproduced report (R18 finding 2) that
# \C-l gets no post-callback redraw otherwise. Two reviewers failed to
# reproduce that on bash 5.3.9, and shellcheck independently calls the
# self-assignment a no-op (SC2269). The hedge is gone because the
# behaviour it guarded is now pinned by a test --
# `ctrl_l_clears_the_screen_and_scrollback_without_losing_the_typed_line`
# in e2e_shell.rs -- which did not exist when the decision to keep it was
# made. A test beats a suppressed lint on code nobody can show is needed.
__ama_clear_screen_widget() {
    ama session reset >/dev/null 2>&1
    printf '\033[H\033[2J\033[3J'
}

# Chain to whatever already owns Enter (RISK-03) instead of clobbering it.
__ama_install() {
    local existing
    existing=$(bind -s 2>/dev/null | sed -n 's/^"\\C-m": "\(.*\)"$/\1/p' | head -n1)
    bind -x '"\C-x\C-aq": __ama_hook' 2>/dev/null || return 0
    if [[ -z $existing ]]; then
        # \C-j is also accept-line and is left unbound, so the macro terminates.
        bind '"\C-m": "\C-x\C-aq\C-j"' 2>/dev/null
    elif [[ $existing != *'\C-x\C-aq'* ]]; then
        bind "\"\\C-m\": \"\\C-x\\C-aq${existing}\"" 2>/dev/null
    fi
    # else (R18, finding 3): \C-m is already chained -- e.g. a second
    # `eval "$(ama init bash)"` in the same shell -- so leave it alone.
    # Falling into the old unconditional `else` here overwrote an
    # already-correct chain with a bare `\C-x\C-aq\C-j`, dropping whatever
    # third-party payload `\C-m` had been chained to.
    bind -x '"\C-l": __ama_clear_screen_widget' 2>/dev/null
}

__ama_install

fi
