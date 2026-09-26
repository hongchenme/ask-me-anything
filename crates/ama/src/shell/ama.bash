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
