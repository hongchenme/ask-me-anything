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
# Recognition order is fixed (ADR-006): line start wins over an operator,
# so "@@ compare a && @@ b" is one prompt, not two.
__ama_split() {
    local line=$1 lead trimmed
    lead=${line%%[![:space:]]*}
    trimmed=${line#"$lead"}

    if [[ $trimmed == '@@ '* ]]; then
        printf '%s\x1f%s' "$lead" "${trimmed#'@@ '}"
        return 0
    fi
    local re='^(.*[;&|][[:space:]]*)@@ (.*)$'
    if [[ $line =~ $re ]]; then
        printf '%s\x1f%s' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
        return 0
    fi
    return 1
}
