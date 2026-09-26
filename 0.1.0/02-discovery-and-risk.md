---
software_version: 0.1.0
process_version: 1.0.0
stage: S1-discovery
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [01-intent.md, README.md]
updated: 2026-09-26
---

# Discovery and risk

All findings below were produced by executing the probe, not by reasoning about it.
Every probe ran against real `bash -i` inside real `tmux` on the target machine.

## 1. Environment baseline

| Fact | Value |
|---|---|
| Shell | `/bin/bash` (bash 5.x, readline available) |
| `TERM` | `xterm-256color`, not currently inside tmux |
| Multiplexers present | `tmux`, `screen`, `script` |
| Toolchain | `cargo 1.98.1`, `rustc 1.98.1` |
| Agent present | `claude` 2.1.274 at `~/.local/bin/claude` |
| Registry access | Available; `serde_yaml_ng`, `clap`, `anyhow`, `serde` resolve |

Existing Rust conventions in this repo come from the gitignored `learn/` curriculum
workspace: edition 2024, workspace-level `[workspace.dependencies]`, `anyhow` +
`thiserror` for errors, `clap` derive for CLI, `assert_cmd` + `predicates` for CLI
tests. The new crate follows these so the owner reads one idiom, not two.

## 2. Findings

### F-01 — `@@` is a legal bash command word, but a bare binary cannot serve the README

A PATH executable named `@@` runs correctly: `@@ hello world` yields `argc=2`.

It fails the README's own first example. `@@ what's this project all about?` aborts in
the parser before `@@` is ever looked up:

```
bash: -c: line 1: unexpected EOF while looking for matching `''
```

The apostrophe opens a quote. The same class of failure applies to `*`, `$`, `!`, `|`,
`>`, `(`, `&`. Natural-language questions contain these constantly. **A PATH binary
alone cannot implement the documented behaviour.**

### F-02 — A readline hook on Enter captures the raw line intact

Binding a `bind -x` helper to a private key sequence and remapping Enter to a macro
that fires it before `accept-line`:

```bash
bind -x '"\C-x\C-aq": __ama_hook'
bind     '"\C-m": "\C-x\C-aq\C-j"'
```

`\C-m` and `\C-j` both default to `accept-line`, and rebinding `\C-m` leaves `\C-j`
intact, so the macro terminates instead of recursing. Observed captures:

| Typed | `$READLINE_LINE` seen by the hook |
|---|---|
| `@@ what's this project all about?` | `what's this project all about?` |
| `@@ list *.rs files \| grep foo && echo $HOME` | `list *.rs files \| grep foo && echo $HOME` |

No quote handling, no glob expansion, no `$HOME` substitution, no pipeline splitting.
`echo NORMAL-COMMAND-STILL-WORKS` executed normally. This retires the central
feasibility risk.

### F-03 — Executing the agent inside the hook is the wrong half of the fork

Two ways exist to act on a captured line. Both were built and run.

**(A) Run the agent inside the `bind -x` handler.** Readline's redisplay erases the
typed line the moment the handler writes output — confirmed both mid-flight and after
completion. Captured pane after a run:

```
🤖: thinking about [what's up?]...
🤖: done
A$
```

The user's own question is gone from the screen, and the line never enters shell
history. Rejected.

**(B) Rewrite `READLINE_LINE` into a normal command and let `accept-line` run it.**
The hook replaces the buffer with `@@ '<single-quote-escaped prompt>'`. Captured pane:

```
C$ @@ 'what'\''s this project all about?'
🤖: thinking about [what's this project all about?]...
🤖: done
C$ @@ 'glob *.rs and $HOME | stay literal'
🤖: thinking about [glob *.rs and $HOME | stay literal]...
^C
(interrupted)
C$ echo SHELL-STILL-FINE
SHELL-STILL-FINE
```

Question stays on screen, metacharacters stay literal, Ctrl-C interrupts the agent
cleanly, the shell survives, and the line enters history. Selected. This is also the
convention used by `fzf` and `atuin`, so it composes with tools the owner may already
run.

Cost: the executed line renders with quoting. The README examples show an unquoted
line and will be corrected rather than left inaccurate.

Note: bash cannot define a *function* named `@@` (`not a valid identifier`), so the
rewrite target must be a real PATH executable.

### F-04 — `tmux capture-pane` is a literal implementation of "current terminal view"

`tmux capture-pane -p` returns the visible screen including other commands' output:

```
hong@bunny:/tmp$ echo HELLO-FROM-SCROLLBACK; ls /nonexistent-xyz
HELLO-FROM-SCROLLBACK
ls: cannot access '/nonexistent-xyz': No such file or directory
```

After `clear`, the same call returns only the prompt. **The README's reset semantics
therefore need no reset code at all on this path** — once the screen is clear, no `@@`
line is visible, so no prior conversation is in scope. Outside a multiplexer there is
no portable way for a child process to read the screen, so a fallback is required.

### F-05 — The README's configuration line is not a one-shot invocation

`agent: claude --model opus --effort high` launches interactive Claude Code and never
returns. `claude --help` confirms `-p, --print` is the one-shot flag, alongside
`--model`, `--effort`, and `--output-format`. Either the documented config is wrong or
the tool supplies the missing flag. Resolved by ADR-003.

## 3. Alternatives considered

| Area | Rejected alternative | Reason |
|---|---|---|
| Capture | PATH binary only | F-01: fails the headline example |
| Capture | `command_not_found_handle` | Bash parses before lookup, so F-01 still applies; also collides with the distro `command-not-found` handler |
| Capture | Agent runs inside the readline hook | F-03(A): erases the question, no history, no job control |
| Context | tmux required, no fallback | Tool would refuse to run in the owner's current non-tmux shell |
| Context | Own transcript only | Cannot answer "why did that build fail?", the strongest use case |
| Context | Full PTY logging via `script` | Invasive; restructures how the shell is started |
| Agent | Pure generic pipe, no adapters | Forces the README config to be rewritten and every user to know their agent's one-shot flag |
| Agent | Structured template config only | Verbose for the common case; discards the README's one-line ergonomics |

## 4. Risk register

| ID | Risk | Tier | Disposition |
|---|---|---|---|
| RISK-01 | Terminal view reaches an external agent unfiltered. A printed `.env`, an `export AWS_SECRET_ACCESS_KEY=…`, or a pasted token on screen is sent verbatim. | R1 | **Accepted by owner, 2026-09-26.** Redaction was explicitly cut from 0.1.0. Mitigated only by `NFR-02` (line cap) and `--no-context`. Documented in README. Revisit if the tool leaves single-user use. |
| RISK-02 | The escape function is the security boundary: it re-injects user text into the shell for execution. A flaw turns a typed question into executed code. | R1 | **Blocking control.** Single-quote escaping is total — inside `'…'` bash treats every byte literally, and `'` itself is the only character needing handling (`'` → `'\''`). Verified by property test TEST-P01 over arbitrary byte strings. |
| RISK-03 | Rebinding Enter collides with `fzf`, `atuin`, `ble.sh`, or `bash-preexec`. | R1 | `ama init` captures any existing `\C-m` binding and chains to it instead of clobbering; `ama doctor` reports the detected chain. |
| RISK-04 | `bind -x` unavailable or readline absent (non-interactive, restricted, or minimal shell). | R1 | `ama init` emits nothing and exits 0 when non-interactive; `@@` remains usable directly with manual quoting. |
| RISK-05 | Degraded context outside tmux surprises the user, who cannot tell which mode is active. | R1 | `ama doctor` states the active context source; first run outside a multiplexer prints a one-time hint. |
| RISK-06 | Agent CLI flags drift, silently breaking an adapter. | R1 | Adapters only ever *add* a missing one-shot flag and are overridable by explicit `command:`. Adapter behaviour is unit-tested against a fixture table. |
| RISK-07 | A large pane produces a large prompt: slow and costly, against INT-07. | R1 | NFR-02 caps context lines; the cap is configurable. |
| RISK-08 | Agent writes to stdout slowly or never; user cannot tell whether it hung. | R1 | Output is streamed as it arrives rather than buffered, so first token is visible immediately; Ctrl-C works (F-03). |

## 5. Data classification

| Data | Class | Handling |
|---|---|---|
| Terminal pane content | Potentially secret-bearing (RISK-01) | Held in memory, sent to the user's configured agent, never written to disk by `ama` |
| Conversation transcript (non-tmux fallback) | Same | `~/.ama/sessions/<key>.jsonl`, mode `0600`, deleted on reset |
| Agent credentials | Not handled | `ama` never reads, stores, or forwards API keys; the agent CLI owns its own auth |
| `~/.ama/config.yml` | User configuration | Read only |

## 6. Risk tier

**R1 — reversible internal tooling.** A local developer tool with no production data,
no network service, no multi-user surface, and no persistent state beyond a deletable
session file. It does not reach R2 because there is no user-facing production
behaviour; RISK-01 is a single-user privacy trade the owner has accepted knowingly.

RISK-02 receives R3-grade treatment (property testing, adversarial review) despite the
R1 tier, because the failure mode is arbitrary code execution in the owner's shell.

## SG1 — Discovery accepted

Accepted 2026-09-26 by solo-founder. Problem framing, approach direction, R1 tier, and
the RISK-01 acceptance are approved. No unresolved blocking issue.
