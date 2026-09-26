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
__ama_split() {
    local line=$1 lead trimmed head tail_re
    lead=${line%%[![:space:]]*}
    trimmed=${line#"$lead"}

    if [[ $trimmed == '@@ '* ]]; then
        printf '%s\x1f%s' "$lead" "${trimmed#'@@ '}"
        return 0
    fi
    if [[ $line == *"@@ "* ]]; then
        head=${line%%"@@ "*}
        tail_re='[;&|][[:space:]]*$'
        if [[ $head =~ $tail_re ]]; then
            printf '%s\x1f%s' "$head" "${line#*"@@ "}"
            return 0
        fi
    fi
    return 1
}

# ---- interactive integration ------------------------------------------------
# Nothing to do in a non-interactive shell (REQ-24).
case $- in *i*) ;; *) return 0 2>/dev/null || exit 0 ;; esac

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

__ama_clear_screen_widget() {
    ama session reset >/dev/null 2>&1
    READLINE_LINE=""
    READLINE_POINT=0
    printf '\033[H\033[2J'
}

# Chain to whatever already owns Enter (RISK-03) instead of clobbering it.
__ama_install() {
    local existing
    existing=$(bind -s 2>/dev/null | sed -n 's/^"\\C-m": "\(.*\)"$/\1/p' | head -n1)
    bind -x '"\C-x\C-aq": __ama_hook' 2>/dev/null || return 0
    if [[ -n $existing && $existing != *'\C-x\C-aq'* ]]; then
        bind "\"\\C-m\": \"\\C-x\\C-aq${existing}\"" 2>/dev/null
    else
        # \C-j is also accept-line and is left unbound, so the macro terminates.
        bind '"\C-m": "\C-x\C-aq\C-j"' 2>/dev/null
    fi
    bind -x '"\C-l": __ama_clear_screen_widget' 2>/dev/null
}

__ama_install
