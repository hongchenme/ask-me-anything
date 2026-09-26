---
software_version: 0.1.1
process_version: 1.0.0
stage: S2-specification
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [03-product-requirements.md]
updated: 2026-09-26
---

# Design — 0.1.1

No new module, no new layer, no change to any interface between existing
modules. Three decisions, each landing inside a component that already owns
the concern.

```text
config.rs   AgentConfig gains `tools:`          REQ-32
   |
adapter.rs  normalize(argv, tools) grants search  REQ-31 / ADR-007
   |
prompt.rs   PREAMBLE says "answer it, use tools"  REQ-34 / ADR-008

shell/ama.bash  \C-m and \C-j both chain to the hook  REQ-33 / ADR-009
```

## ADR-007 — the adapter grants search authority, because it already owns "what this agent needs to run one-shot"

**Context.** `claude -p` cannot answer a permission prompt, so tools needing
approval are denied and the agent reports itself incapable (F-07).

**Options considered.**

| Option | Rejected because |
|---|---|
| Document it; users add the flag themselves | Leaves the default install broken. The owner rejected this at Q-01 |
| `--permission-mode auto` | Verified to work, but grants far more than search — it is the whole permission system, not one tool |
| `--dangerously-skip-permissions` | Hands an agent full authority over a prompt containing untrusted pane content (RISK-01). Never |
| **Grant one named tool through the adapter table** | **Chosen** |

**Decision.** `adapter::normalize` gains a `tools` parameter and, for
`tools: research`, inserts the narrowest web-search authorization each known
agent accepts (REQ-31's table).

This is the same operation the module already performs and is bound by the
same rule it already states: *insert a missing token; never remove, reorder
or rewrite the user's arguments*. A user who writes their own
`--allowedTools` is not second-guessed — in either direction, wider or
narrower.

**Consequences.** `normalize`'s signature changes; it is crate-internal, and
`Config::agent_spec` is its only caller. `codex` inserts before `exec`
because `codex exec --search` is rejected by the parser (AS-02); the
insertion order is `--search` then `exec`, giving `codex --search exec`.

**Why WebSearch and not WebFetch** is a risk decision, not a design one:
[02-discovery-and-risk.md §4](02-discovery-and-risk.md#risk-01-re-examined-since-this-cycle-touches-it).

## ADR-008 — the preamble tells the agent it is a question box, not a repo assistant

**Context.** The grant alone does not fix the defect (F-08). Claude Code's
default system prompt frames the agent as a coding assistant for the current
directory — visible verbatim in a probe that closed with *"Was there
something coding or project-related I can help with instead?"* — and a
general question gets brushed off.

**Options considered.**

| Option | Rejected because |
|---|---|
| Inject `--append-system-prompt` per agent | System-prompt flags differ per agent and the README already documents that argument as the *user's*. Writing there would silently fight a user's own value |
| Leave the preamble; rely on the grant | Measured: still fails (F-08) |
| **Extend the preamble `ama` already sends** | **Chosen** |

**Decision.** `prompt::PREAMBLE` gains a second paragraph:

> Answer whatever is asked. This is a general question box, not only a coding
> assistant, and the question is often not about the current directory. You
> can search the web and read this machine, so use those tools before
> answering anything you do not already know, and never say you lack internet
> access or tools without having tried them. If the question is missing one
> thing you need, such as a location, ask for just that.

Four things are load-bearing and were each added in response to an observed
failure, not written speculatively:

1. *"general question box, not only a coding assistant"* — counters the
   observed "this isn't a coding task for this repo" refusal.
2. *"use those tools before answering anything you do not already know"* —
   turns an available tool into a used one (F-08).
3. *"never say you lack internet access or tools without having tried"* —
   names the exact false claim in the defect report.
4. *"ask for just that"* — the reporter's question has no location in it, so
   the honest answer is a one-line question back. Without this clause the
   agent padded the same answer with a false incapacity claim.

**Consequences.** ~60 words on every turn (RISK-12, accepted). The wording is
agent-neutral: it names capabilities, never a tool by vendor name, so it
reads correctly to codex, ollama, or a local model with no web access at all
— such an agent tries, fails, and says so, which is the honest outcome.

## ADR-009 — a private accept-line lets both Return keys chain to the hook

**Context.** `bind '"\C-m": "\C-x\C-aq\C-j"'` uses `\C-j` as the macro's
terminator, so `\C-j` cannot also trigger the hook without recursing (F-09).
Terminals sending LF therefore bypass the integration entirely.

**Options considered.**

| Option | Rejected because |
|---|---|
| Bind `\C-j` to the same macro | Self-recursive: the macro ends in the key that expands it |
| Have the hook detect the raw line and re-run it | The hook cannot accept a line; and bash has already parsed by then |
| Tell users to reconfigure their terminal | The product is a key binding. "Use a different terminal" is not a fix |
| **A private keyseq bound to the `accept-line` function** | **Chosen** |

**Decision.**

```bash
bind '"\C-x\C-am": accept-line'          # private terminator
bind '"\C-m": "\C-x\C-aq\C-x\C-am"'
bind '"\C-j": "\C-x\C-aq\C-x\C-am"'
```

`\C-x\C-am` is bound to the readline *function*, not to a key the user's
terminal can send for Return, so remapping `\C-m` or `\C-j` cannot reach it
and the macro always terminates. It sits under the `\C-x\C-a` prefix the
integration already claims for `\C-x\C-aq`.

Chaining (RISK-03) is applied to each key independently, with the existing
logic unchanged: read the current macro from `bind -s`, prepend `\C-x\C-aq`
if it is not already there, and leave a key alone once it is chained.

**zsh is not changed.** It rebinds the `accept-line` *widget*, which both
`^M` and `^J` already point to (F-10). It gets a regression test, not a
patch — the asymmetry is a real difference between the two line editors and
recording it prevents a future "port the fix to zsh" that would break it.

### The hazard this introduces, and its mitigation

Once `\C-j` expands to a macro, a third-party `\C-m` macro ending in `\C-j`
runs the hook twice on one keypress (F-11 / RISK-09). On a triggered line the
second pass would re-quote an already-quoted buffer:

```text
@@ what's it        -> @@ 'what'\''s it'        (first pass, correct)
                    -> @@ ''\''what'\''\'\'''\''s it'\'''   (second pass, corrupt)
```

**Mitigation.** `__ama_hook` remembers the exact buffer it last produced and
returns without touching a buffer equal to it:

```bash
if [[ ${__ama_last_rewrite+x} && $READLINE_LINE == "$__ama_last_rewrite" ]]; then
    return 0
fi
```

Equality against our own output, not a heuristic for "looks quoted". It
cannot misfire on a line the hook did not write, and it satisfies NFR-09 —
one string comparison, no subprocess, so NFR-08's budget is untouched.

## Interfaces changed

| Symbol | Before | After | Callers |
|---|---|---|---|
| `adapter::normalize` | `(&[String]) -> Vec<String>` | `(&[String], Tools) -> Vec<String>` | `Config::agent_spec` only |
| `adapter::Tools` | — | new enum `Research \| None` | config |
| `config::AgentConfig::Structured` | `{command, adapter}` | `{command, adapter, tools}` | — |
| `prompt::PREAMBLE` | 1 paragraph | 2 paragraphs | — |
| `__ama_hook` | rewrites unconditionally | idempotent | `\C-m`, `\C-j` |

No public binary interface changes. `ama doctor` prints `spec.argv`, so it
shows the inserted flag with no change to `doctor` itself — which is the
intended way for a user to see what `ama` decided.

## SG2 — Design accepted

Accepted 2026-09-26.
