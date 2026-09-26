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
    # NFR-09 / RISK-09. One keypress can reach this twice: now that `\C-j`
    # expands to a macro of ours, a third-party `\C-m` macro ending in
    # `\C-j` -- the ordinary way to write one -- routes through the hook and
    # then through it again. A second pass over an already-rewritten line
    # quotes the quoting, and `@@ what's it` reaches the agent as
    # `@@ ''\''what'\''\'\'''\''s it'\'''` (observed, not predicted).
    #
    # So a buffer identical to the one this function last produced is left
    # alone. Equality against our own output, never a guess at what
    # "already quoted" looks like, so it cannot misfire on a line the hook
    # did not write. The `+x` test matters: it keeps an unset variable from
    # matching an empty buffer and skipping the `clear` handling below.
    if [[ -n ${__ama_rewrote+x} && $READLINE_LINE == "$__ama_rewrote" ]]; then
        return 0
    fi
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
        __ama_rewrote=$READLINE_LINE
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

# Print the macro currently bound to key sequence $1 (written the way
# `bind -s` writes it, e.g. `\C-m`), or nothing when that key is unbound or
# bound to a readline *function* -- `bind -s` lists only macros. Done with
# bash pattern matching rather than the `sed` this used to use: the key name
# is now a parameter, and passing `\C-m` through a sed script needs it
# double-escaped to survive BRE, which is one silent-mismatch trap too many
# for a function whose failure mode is "quietly stop chaining".
__ama_bound_macro() {
    local line head="\"$1\": \""
    while IFS= read -r line; do
        if [[ $line == "$head"*'"' ]]; then
            line=${line#"$head"}
            printf '%s' "${line%\"}"
            return 0
        fi
    done < <(bind -s 2>/dev/null)
    return 1
}

# Chain to whatever already owns Enter (RISK-03) instead of clobbering it.
#
# REQ-33: both keys that mean "accept this line" are hooked, not just
# `\C-m`. Return is CR on most terminals but LF on some -- iTerm2 can be
# configured either way, macOS Terminal sends CR -- and 0.1.0 bound only
# `\C-m`, so on an LF terminal the raw line went straight to bash and an
# apostrophe left the shell stuck at PS2 with no output at all.
#
# `\C-j` could not simply be added in 0.1.0 because it *was* the Enter
# macro's terminator: a macro ending in the key that expands it recurses.
# `\C-x\C-am` replaces it -- a private sequence bound to the accept-line
# function, which no terminal sends for Return, so remapping `\C-m` or
# `\C-j` cannot reach it and the macro always terminates.
__ama_install() {
    bind '"\C-x\C-am": accept-line' 2>/dev/null
    bind -x '"\C-x\C-aq": __ama_hook' 2>/dev/null || return 0

    local seq existing
    for seq in '\C-m' '\C-j'; do
        existing=$(__ama_bound_macro "$seq")
        if [[ -z $existing ]]; then
            bind "\"$seq\": \"\\C-x\\C-aq\\C-x\\C-am\"" 2>/dev/null
        elif [[ $existing != *'\C-x\C-aq'* ]]; then
            bind "\"$seq\": \"\\C-x\\C-aq${existing}\"" 2>/dev/null
        fi
        # else (R18, finding 3): already chained -- e.g. a second
        # `eval "$(ama init bash)"` in the same shell -- so leave it alone.
        # Falling into the fresh-install branch here overwrote an
        # already-correct chain, dropping whatever third-party payload the
        # key had been chained to.
    done
    bind -x '"\C-l": __ama_clear_screen_widget' 2>/dev/null
}

__ama_install

fi
