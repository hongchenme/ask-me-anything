# ama shell integration (zsh). Sourced via: eval "$(ama init zsh)"

# Single-quote a string so the shell parses it back byte-for-byte. `(qq)` is
# zsh's own quote-for-later-reparsing flag: it always produces a single-
# quoted word (doubling embedded quotes as '\''), so there is no hand-rolled
# escaping to get wrong here -- verified against real zsh by the property
# test (TEST-P01), not just reasoned about. (A direct port of ama.bash's
# `${1//\'/\'\\\'\'}` was tried first and is broken under zsh: zsh's
# `${...//pat/rep}` does not remove backslashes from the replacement text
# the same way bash does, so it leaves stray backslashes in the output --
# caught by that same round trip, not by inspection.)
__ama_sq() {
    printf '%s' "${(qq)1}"
}

# Split a line into prefix and prompt at the trigger.
# Prints "<prefix><US><prompt>" and returns 0 when the line is triggered,
# where <US> is the 0x1f unit separator. Returns 1 otherwise.
# Recognition order is fixed (ADR-006, amended by A-03): line start wins
# over an operator, and among operator-triggered lines the FIRST trigger
# wins -- so both "@@ compare a && @@ b" and "clear && @@ compare a && @@ b"
# keep "compare a && @@ b" whole, as one prompt. This mirrors ama.bash's
# parameter-expansion approach (shortest/longest match, not a greedy
# regex); `extended_glob`, scoped to this function only, is the one
# zsh-specific addition, needed for the `#` repetition operator below.
#
# The quote-balance guard below mirrors ama.bash's (R26), for the same
# reason and with the same parameter expansions: a `;`, `&` or `|` inside a
# quoted word is not a command separator, so `echo 'clear && @@ joke'` and
# `echo "a; @@ not a prompt"` must pass through exactly as an uninstrumented
# shell would run them (REQ-03). An odd `'` or `"` count before the trigger
# fails closed.
__ama_split() {
    setopt local_options extended_glob
    local line=$1 lead trimmed head sq dq nsq ndq
    lead=${line%%[^[:space:]]*}
    trimmed=${line#"$lead"}

    if [[ $trimmed == '@@ '* ]]; then
        printf '%s\x1f%s' "$lead" "${trimmed#'@@ '}"
        return 0
    fi
    if [[ $line == *'@@ '* ]]; then
        head=${line%%'@@ '*}
        # Discard everything that is not a quote, then count what is left.
        sq=${head//[!\']/}
        dq=${head//[!\"]/}
        nsq=${#sq}
        ndq=${#dq}
        if [[ $head == *[\;\&\|][[:space:]]# ]] && (( nsq % 2 == 0 && ndq % 2 == 0 )); then
            printf '%s\x1f%s' "$head" "${line#*'@@ '}"
            return 0
        fi
    fi
    return 1
}

# ---- interactive integration ------------------------------------------------
# Nothing to do in a non-interactive shell (REQ-24). As in ama.bash (R18,
# finding 1), a guard that `return`s here would be wrong under the
# documented `eval "$(ama init zsh)"` install path: outside a function or a
# sourced file, `return` is an error, not a silent no-op -- verified
# directly: `zsh -c 'eval "$(cat ama.zsh)"; echo REACHED'` must still print
# REACHED. An `if` just skips its own body, disturbing nothing before or
# after it in any calling context -- and it is also the only way to reach
# the `zle` calls below at all, since registering or invoking a widget
# needs an active line editor and errors out otherwise.
if [[ -o interactive ]]; then

# A stable per-shell identity for the transcript fallback.
export AMA_SESSION="$$"

__ama_clears_screen() {
    case $1 in
        clear | reset | 'clear '* | 'reset '* | *'tput clear'*) return 0 ;;
        *) return 1 ;;
    esac
}

# Chain to whatever `accept-line` already means (RISK-03) instead of
# clobbering it -- e.g. a framework that already wraps accept-line for its
# own purposes -- via `__ama_orig_accept_line`, installed below. Every exit
# path (blank, rewritten, passthrough) ends by running that chain, not just
# the passthrough one, so a re-`eval` in the same shell keeps working too.
__ama_accept_line() {
    local split prefix prompt
    if split=$(__ama_split "$BUFFER"); then
        prefix=${split%%$'\x1f'*}
        prompt=${split#*$'\x1f'}
        # A blank prompt is a no-op, not an agent call (REQ-06).
        if [[ -z ${prompt//[[:space:]]/} ]]; then
            BUFFER=""
            CURSOR=0
        else
            # `clear && @@ ...`: drop the transcript now, since the
            # pane-scrape path resets itself but the fallback path has no
            # other signal.
            __ama_clears_screen "$prefix" && ama session reset >/dev/null 2>&1
            BUFFER="${prefix}@@ $(__ama_sq "$prompt")"
            CURSOR=${#BUFFER}
        fi
    else
        __ama_clears_screen "$BUFFER" && ama session reset >/dev/null 2>&1
    fi
    zle __ama_orig_accept_line
}

# Ctrl-L: zle's own clear-screen widget already preserves and redraws
# whatever the user was typing (verified directly against real zsh --
# unlike bash's `bind -x` callbacks, ama.bash's R18 finding 2 does not
# apply here: `zle .clear-screen` alone, with no buffer reassignment at
# all, still redraws an in-progress line correctly). The remaining gap is
# scrollback, and closing it needs bash's exact recipe, not just its `3J`
# byte: a lone `printf '\033[3J'` before `zle .clear-screen` measurably
# under-clears in tmux -- verified directly with a pane deliberately
# scrolled well past its height, using `capture-pane -S` (which can see
# scrollback `-p` alone cannot): most of the pane's *currently on-screen*
# rows were still recoverable afterwards, because at the moment that bare
# `3J` ran, tmux had not yet reclassified them from "on-screen" to
# "scrollback" -- only the rows already scrolled off before Ctrl-L was even
# pressed were caught. Sending the same three bytes ama.bash does, in the
# same order and as one write -- home, erase the whole screen, *then* erase
# scrollback -- leaves nothing still classified as on-screen for `3J` to
# miss, and `capture-pane -S` confirms zero survivors. `zle .clear-screen`
# runs after, redundantly re-clearing the (by then already-erased) screen,
# purely to get the buffer redraw.
__ama_clear_screen() {
    ama session reset >/dev/null 2>&1
    printf '\033[H\033[2J\033[3J'
    zle .clear-screen
}

# Install, idempotently. A second `eval "$(ama init zsh)"` in the same
# shell must not re-chain: doing so unconditionally would capture our own
# widget as "the previous one", making `__ama_orig_accept_line`
# self-referential -- verified directly that this does not deadlock or
# recurse forever on the very next Enter (one extra install just adds one
# harmless extra level of indirection), but repeat it a handful of times,
# as re-sourcing an rc file a few times over a session would, and it
# reliably hits zsh's own `FUNCNEST` recursion guard, surfacing as
# `maximum nested function level reached` and silently dropping whatever
# `accept-line` was originally chained to. `$widgets` reports each
# widget's current definition, so "already ours" is a direct
# associative-array lookup, not text-scraping the way ama.bash has to
# (bash has nothing equivalent to `$widgets`).
if [[ ${widgets[accept-line]:-} != user:__ama_accept_line ]]; then
    zle -A accept-line __ama_orig_accept_line
    zle -N accept-line __ama_accept_line
fi
zle -N clear-screen __ama_clear_screen

fi
