---
software_version: 0.1.2
process_version: 1.0.0
stage: S3-plan
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [04-design.md]
updated: 2026-09-26
---

# Implementation plan — 0.1.2

Five slices, run in order by one implementer. Each depends on the previous
one's interface: TASK-2 needs `Spinner`, TASK-3 needs the new `agent::run`,
and TASK-4 needs the wiring. The whole change is a few hundred lines, so
splitting it across agents would cost more than it saves.

Every slice writes its failing test first (`superpowers:test-driven-
development`) and watches it fail for the right reason. Only then does it
write the smallest change that passes.

## TASK-1 — the spinner (REQ-35, REQ-36, REQ-38, NFR-11, NFR-12, ADR-011/012)

| | |
|---|---|
| **Files** | `crates/ama/src/spinner.rs` (new), `crates/ama/src/lib.rs` |
| **Forbidden** | writing through the answer stream; hiding the cursor; any `unwrap`/`expect` |
| **Interfaces produced** | `Spinner`, `StderrSink` |
| **Risk** | R1: pure, driven through injected writers |
| **Rollback** | nothing calls it until TASK-3 |

**Tests first** (`tests/unit_spinner.rs`, drawing into an in-memory
"screen" that both writers share, so ordering across them is visible):

- the first frame is on the screen before `start` returns;
- frames run 🌑…🌘 and wrap to 🌑, compared against a hand-written list;
- `stop` erases, and nothing is drawn afterwards;
- `stop` returns promptly with a 10 s frame interval (NFR-11);
- a stderr chunk ending in `\n` gives erase, chunk, redraw;
- a partial stderr chunk holds the moon until a chunk completes the line;
- after `stop`, stderr passes through with no escape bytes;
- a writer that always fails panics nowhere (NFR-12).

`TERM=dumb` gating is a private decision function, unit-tested in the module.

## TASK-2 — `agent::run` drives the spinner (REQ-36, REQ-38, RISK-14, ADR-012)

| | |
|---|---|
| **Files** | `crates/ama/src/agent.rs`, `tests/integration_agent.rs` |
| **Depends on** | TASK-1 |
| **Forbidden** | changing the stream, kill or reap logic for `spinner: None`; capturing stderr into the answer |
| **Risk** | R2: this is the process-plumbing path every turn takes |
| **Rollback** | pass `None`; this is exactly 0.1.1's behaviour |

**Tests first:**

- with a spinner drawing into a separate buffer, `out` holds exactly
  `🤖: hello prompt\n` (REQ-36: the transcript source is untouched);
- with one shared screen and a 10 s interval, the bytes are exactly
  `frame`, erase, `🤖: hello prompt\n`, and it completes in well under the
  interval (REQ-36 order, NFR-11);
- an agent writing `warn` to stderr, then answering, produces
  `frame`, erase, `warn\n`, `frame`, erase, `🤖: answer\n` (REQ-38);
- a grandchild holding the stderr pipe for 5 s does not hold up `run`
  (RISK-14);
- a silent agent's diagnostic comes after the erase (REQ-36). Pinned end to
  end in TASK-4, because the diagnostic goes to the real stderr.

Existing callers are updated to pass `None`. That is mechanical, and it is
recorded as the interface change in 04-design.md.

## TASK-3 — wiring and the off switch (REQ-37, REQ-39, ADR-011, ADR-013)

| | |
|---|---|
| **Files** | `crates/ama/src/cli.rs`, `crates/ama/src/config.rs`, `tests/unit_config.rs`, `tests/integration_cli.rs` |
| **Depends on** | TASK-2 |
| **Forbidden** | starting the spinner before `context::gather` (F-15); changing any output when stdout is not a terminal |
| **Risk** | R2 |
| **Rollback** | `spinner: false` |

**Tests first:**

- `spinner` defaults to `true`, and `spinner: false` reads as `false`;
- `spinner: sometimes` is a configuration error;
- `ama ask` with stdout on a pipe emits no ESC byte and no moon glyph
  (REQ-37).

## TASK-4 — the real terminal (all REQs, in tmux)

| | |
|---|---|
| **Files** | `tests/e2e_shell.rs`, `tests/fixtures/fake-agent-thinking` (new), `tests/fixtures/fake-agent-chatty` (new) |
| **Depends on** | TASK-3 |
| **Risk** | R1: tests only |

The fixtures:

- `fake-agent-thinking` reads the prompt, is silent for 2 s, then echoes the
  prompt back. The echo matters: if the moon ever landed in a capture, it
  would come back in the answer.
- `fake-agent-chatty` writes `progress one`, pauses, writes `partial`,
  pauses, finishes the line with ` line`, pauses, and answers.

**Tests first.** Each must fail before TASK-1…3 exist:

1. `the_moon_waxes_while_the_agent_thinks`. The verbatim `@@ whats weather?`:
   the moon within 1 s, at least 3 distinct phases before the answer, then
   `🤖:` and not one glyph left (REQ-35, REQ-36, RISK-17).
2. `agent_stderr_keeps_its_own_lines_while_the_moon_is_up`. The chatty
   fixture: exact lines, the moon below a complete stderr line, nothing
   left (REQ-38).
3. `a_silent_agent_is_reported_on_a_clean_line`. The diagnostic starts at
   column 0, with no glyph before it (REQ-36).
4. `the_moon_can_be_turned_off`. `spinner: false`: no glyph while the agent
   thinks, and the answer arrives (REQ-39).
5. `an_interrupt_during_the_moon_returns_a_usable_shell`. Ctrl-C mid-think:
   `rc=130` and a working shell (RISK-15).
6. `the_moon_waxes_in_zsh_too`. Shell-independence (ADR-010).

## TASK-5 — version, docs, cycle record

| | |
|---|---|
| **Files** | `crates/ama/Cargo.toml`, `Cargo.lock`, `README.md`, `0.1.2/*` |
| **Depends on** | TASK-1…4 |

- `0.1.1` → `0.1.2`.
- README: the moon in *Use*, and `spinner:` in *Configure*.

## Test matrix

| Requirement | Level | File |
|---|---|---|
| REQ-35 | unit + e2e | `unit_spinner.rs`, `e2e_shell.rs` |
| REQ-36 | integration + e2e | `integration_agent.rs`, `e2e_shell.rs` |
| REQ-37 | unit + integration | `spinner.rs`, `integration_cli.rs` |
| REQ-38 | unit + integration + e2e | all three |
| REQ-39 | unit + e2e | `unit_config.rs`, `e2e_shell.rs` |
| NFR-10 | measurement | 07-verification.md |
| NFR-11 | unit + integration | `unit_spinner.rs`, `integration_agent.rs` |
| NFR-12 | unit | `unit_spinner.rs` |
| RISK-14 | integration | `integration_agent.rs` |
| RISK-15 | e2e | `e2e_shell.rs` |
| No regression | full suite | `./check.sh` |

## Feedback loop

`./check.sh` is the SG4 gate, exactly as in 0.1.1. It runs rustfmt, clippy
(`-D warnings`, no unwrap/expect in lib/bins), `bash -n`, `zsh -n`,
shellcheck, the build, and the full suite single-threaded. The live-agent
check with real claude and codex spends money, so it runs by hand and is
recorded in 07.

## SG3 — Plan approved

Recorded 2026-09-26 under the [process exception](README.md#process-exception).
