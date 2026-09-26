---
software_version: 0.1.0
process_version: 1.0.0
stage: S2-specification
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [01-intent.md, 02-discovery-and-risk.md]
updated: 2026-09-26
---

# Product requirements

Each requirement has a stable identifier, a statement, and an acceptance criterion
that can be executed. Criteria are written as the check itself, so S3 can turn each
one into a named test without reinterpreting it. The traceability table at the end
maps requirement groups to the test layers defined in
[04-design.md §8](04-design.md#8-verification-strategy).

## Domain terms

| Term | Meaning |
|---|---|
| **Trigger** | The literal `@@` followed by one space at the start of a shell line |
| **Prompt** | Everything after the trigger, byte-for-byte as typed |
| **View** | The visible terminal screen, from the first trigger line on it to the bottom |
| **Turn** | One prompt and its answer |
| **Session** | The sequence of turns sharing a view; ends when the screen is cleared |
| **Agent** | The user's own CLI, named in `~/.qmx2/config.yml` |
| **Adapter** | Built-in knowledge of how to make a known agent one-shot |

## Functional requirements

### Capture

| ID | Requirement | Acceptance criterion |
|---|---|---|
| REQ-01 | Typing `@@ ` then a prompt and pressing Enter invokes the agent with that prompt. | tmux e2e: send `@@ hello`, pane shows a `🤖: ` answer. |
| REQ-02 | The prompt reaches the agent byte-for-byte. No glob expansion, quote interpretation, variable substitution, history expansion, or pipeline splitting. | tmux e2e with an echoing fake agent: `@@ what's *.rs $HOME \| & ! done?` round-trips exactly. |
| REQ-03 | Lines not beginning with the trigger behave exactly as in an uninstrumented shell. | tmux e2e: `echo hi`, a pipeline, a quoted string, a failing command, and a multi-line `for` loop all behave identically with and without the integration loaded. |
| REQ-04 | The typed question remains visible on screen and in shell history after the turn. | tmux e2e: pane contains the question above the answer; `history` shows the entry. |
| REQ-05 | Ctrl-C during a turn interrupts the agent and returns a usable prompt. | tmux e2e: interrupt a slow fake agent; shell survives and the next command runs. |
| REQ-06 | An empty prompt (`@@ ` alone) is a no-op, not an agent call. | tmux e2e: no agent process spawned, no error. |
| REQ-28 | The trigger is recognised at the start of the line (ignoring leading whitespace) **and** at a command position after `;`, `&&`, `\|\|`, or `\|`. The preceding commands run first, unmodified. The **earliest** trigger on the line always wins (ADR-006, as amended by A-03). | tmux e2e: `clear && @@ joke` clears the screen, then answers with no prior context. Unit: split fixtures for line-start, leading whitespace, `&&`, `;`, a prompt that merely *contains* `\|` or `&&`, and `clear && @@ compare a && @@ b` splitting at the first trigger. |

### Answering

| ID | Requirement | Acceptance criterion |
|---|---|---|
| REQ-07 | The answer is prefixed `🤖: ` and written to stdout. | Integration: `ama ask -- hi` stdout starts with `🤖: `. |
| REQ-08 | Agent output is streamed as it arrives, not buffered until exit. | Integration: fake agent emits a line, sleeps 2s, emits another; first line is readable before the agent exits. |
| REQ-09 | The agent runs with the user's current working directory. | Integration: fake agent prints `$PWD`; equals the invoking directory. |
| REQ-10 | Agent stderr is surfaced on failure, not swallowed. | Integration: failing fake agent; its stderr appears and exit code is non-zero. |

### Context

| ID | Requirement | Acceptance criterion |
|---|---|---|
| REQ-11 | Inside tmux or screen, context is the visible pane from the first trigger line to the bottom, including other commands' output. | tmux e2e: run `ls /nonexistent`, then `@@ …`; the composed prompt contains the `ls` error. |
| REQ-12 | Outside a multiplexer, context is the per-session transcript of prior turns. | Integration with `TMUX` unset: second turn's composed prompt contains the first turn's question and answer. |
| REQ-13 | Clearing the screen starts a fresh conversation. | tmux e2e: two turns, `clear`, third turn; composed prompt for the third contains neither earlier turn. Non-tmux: the hook fires `ama session reset` on `clear`, `reset`, and Ctrl-L. |
| REQ-14 | Sessions are isolated per pane or per TTY. | Integration: two distinct session keys produce independent transcripts. |
| REQ-15 | `--no-context` sends the prompt alone. | Integration: composed prompt equals the prompt. |

### Bring your own agent

| ID | Requirement | Acceptance criterion |
|---|---|---|
| REQ-16 | The agent is read from `~/.qmx2/config.yml`, honouring `$XDG_CONFIG_HOME`-style override via `AMA_CONFIG` for testability. | Unit: config load from a temp path. |
| REQ-17 | `agent:` accepts a bare command string. The prompt is written to the agent's stdin. | Unit: `agent: mycli --flag` → argv `[mycli, --flag]`, prompt on stdin. |
| REQ-18 | An adapter table supplies a missing one-shot flag for known agents, so the README's `agent: claude --model opus --effort high` works as written. | Unit fixture table: `claude …` → `claude -p …`; `-p` already present → unchanged; unknown agent → unchanged. |
| REQ-19 | An explicit structured form overrides all adapter behaviour, with `{prompt}` substituted as an argument when present. | Unit: `command: [my-bot, --ask, "{prompt}"]` → argv with the prompt substituted and nothing on stdin. |
| REQ-20 | With no config file, `ama` reports what to write rather than guessing. | Integration: missing config → exit 2 with a message naming the path and a valid example. |
| REQ-21 | `ama` never reads, stores, or forwards agent credentials. | Review: no code path touches an API-key-shaped environment variable or a credentials file. |

### Commands

| ID | Requirement | Acceptance criterion |
|---|---|---|
| REQ-22 | `ama init bash` and `ama init zsh` print shell integration to stdout for `eval`. | Integration: output is non-empty and `bash -n` / `zsh -n` parse it cleanly. |
| REQ-23 | `ama init` chains to any pre-existing Enter binding instead of clobbering it. | tmux e2e: load a competing `\C-m` binding first; both it and the trigger work. |
| REQ-24 | `ama init` emits nothing and exits 0 in a non-interactive shell. | Integration: `bash -c 'eval "$(ama init bash)"'` succeeds silently. |
| REQ-25 | `ama doctor` reports shell, integration state, context source, session key, resolved agent argv, and whether the agent is executable. | Integration: output contains each field; exit 0 when healthy, non-zero when the agent is missing. |
| REQ-26 | `ama session reset` discards the current session's transcript. | Integration: transcript file removed; next turn has no prior context. |
| REQ-27 | `@@` is usable directly, including in scripts, as an alias for `ama ask --`. | Integration: `@@ 'hello'` from a non-interactive script produces an answer. |

## Non-functional requirements

| ID | Requirement | Acceptance criterion |
|---|---|---|
| NFR-01 | `ama`'s own overhead before the agent process is spawned stays under 50 ms. | Benchmark against a `true` agent; measured wall time minus spawn, 20-run median. |
| NFR-02 | Context is capped at `max_context_lines` (default 200), keeping the most recent lines. | Unit: a 1000-line pane yields 200 lines, the last 200. |
| NFR-03 | Absence of tmux degrades to the transcript path without error. | Integration with `TMUX` unset: turn succeeds. |
| NFR-04 | Malformed config produces a diagnostic naming the field, never a panic. | Unit: malformed YAML, wrong type, and empty `agent:` each yield a typed error. |
| NFR-05 | Exit codes: 0 success, 1 agent failure, 2 configuration error, 130 interrupted. | Integration: one case per code. |
| NFR-06 | The transcript file is created mode `0600`. | Integration: permission bits asserted. |
| NFR-07 | No `unsafe`, no `.unwrap()` or `.expect()` on fallible runtime paths outside tests. | `#![forbid(unsafe_code)]` plus a clippy gate. |

## Traceability summary

| Requirement group | Design section | Covered by |
|---|---|---|
| REQ-01 … REQ-06, REQ-28 | ADR-001, ADR-006, `shellinit`, `escape` | TEST-E2E-*, TEST-P01 |
| REQ-07 … REQ-10 | `agent`, `render` | TEST-INT-* |
| REQ-11 … REQ-15 | ADR-002, `context`, `session` | TEST-E2E-*, TEST-INT-* |
| REQ-16 … REQ-21 | ADR-003, `config` | TEST-UNIT-* |
| REQ-22 … REQ-27 | ADR-004, `cli`, `shellinit` | TEST-INT-* |
| NFR-01 … NFR-07 | cross-cutting | benchmark, clippy gate, unit |

## SG2 — Specification accepted

Accepted 2026-09-26 by solo-founder.

**Amendment A-01, 2026-09-26 (during S3).** REQ-28 added. Planning against README
example 3 (`clear && @@ show me the joke of the day`) showed the trigger is not always
at the start of the line, which every capture requirement had assumed. Recorded under
SDLC §14 as a plan-time return to S2. Scope impact: one extra branch in the shell hook
and a unit-tested split function (ADR-006 in [04-design.md](04-design.md)); no change
to architecture, risk tier, or any other requirement.
