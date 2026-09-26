---
software_version: 0.1.2
process_version: 1.0.0
stage: S4-build
status: complete
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [05-implementation-plan.md]
updated: 2026-09-26
---

# Build record — 0.1.2

Branch `cycle/0.1.2-moon-spinner`, off `main` at `9f81536`. **Nothing is
committed.** The owner has not asked for a commit, and the repository rule
is to commit only on request. The whole change sits in the working tree.
The tracked files change by +952/−57, plus five new files of 526 lines and
this `0.1.2/` record.

| File | Change |
|---|---|
| `crates/ama/src/spinner.rs` | **new**: `Spinner`, `StderrSink`, `strip_frame` (ADR-010…012, 014) |
| `crates/ama/src/agent.rs` | `run` takes the spinner. It stops it on the first byte, after streaming and on every return, routes stderr through it, and drains stderr with a progress-aware wait |
| `crates/ama/src/cli.rs` | starts the moon after the pane capture and drops it after `run` |
| `crates/ama/src/config.rs` | `spinner: bool`, default `true` (ADR-013) |
| `crates/ama/src/context.rs` | `without_moon_frames` on every capture (ADR-014) |
| `crates/ama/src/lib.rs` | `pub mod spinner` |
| `crates/ama/Cargo.toml`, `Cargo.lock` | 0.1.1 → 0.1.2 |
| `README.md` | the moon in *Use*, `spinner:` in *Configure*, the design-record pointer |
| `tests/unit_spinner.rs` | **new**: 10 tests |
| `tests/integration_agent.rs` | +7 tests; the 9 existing calls pass `None` |
| `tests/e2e_shell.rs` | +9 tests, polling helpers, and 5 pre-existing tests moved from fixed sleeps to condition waits (D-04, D-06, D-07, D-09) |
| `tests/integration_cli.rs`, `unit_config.rs`, `unit_context.rs` | +1, +2, +1 tests |
| `tests/fixtures/fake-agent-{thinking,chatty,stderr-kind}` | **new** |

The shell integration (`ama.bash`, `ama.zsh`) is untouched. `git diff` over
`crates/ama/src/shell/` is empty.

## Environment

This is the first cycle built on macOS (26, arm64); 0.1.0 and 0.1.1 were
built on Linux. The machine had no Rust toolchain, tmux or shellcheck, and
all three were installed with Homebrew: rustc 1.98.1, tmux 3.7c, shellcheck
0.11.0. **Before any change, `./check.sh` was green on a clean `main` here.**
That baseline is what makes every later failure attributable.

## Slices

Every slice started with its failing test, observed to fail for the right
reason before any implementation existed.

### TASK-1 — the spinner

RED: against a do-nothing stub, 8 of 10 unit tests failed on their
assertions. The other two could not fail against a stub, so they were
proven afterwards by mutation (below). The `TERM=dumb` gate failed first
too.

**The mutation check found a defect in a test.**
`stop_does_not_wait_out_the_frame_interval` still passed with the
condition-variable wake-up deleted. It called `stop()` before the drawing
thread had begun waiting, so the thread saw the flag without needing to
be woken. The test now lets the thread settle into its 10 s wait first,
and it fails with `stop took 9.946842417s` when the wake-up is removed.

### TASK-2 — `agent::run`

RED: with the parameter accepted but unused, three tests failed, each
showing the exact defect it exists for:

```text
left:  "\r\x1b[K🤖 🌑🤖: hello prompt\n"          <- the moon glued to the answer
right: "\r\x1b[K🤖 🌑\r\x1b[K🤖: hello prompt\n"
```

RISK-14 was demonstrated before it was mitigated. Stderr routing was built
first with an ordinary unbounded join, and the grandchild test failed with
`the turn waited 5.052049042s on a grandchild's pipe`. The bounded wait
followed.

### TASK-3 and TASK-4 — the wiring, the off switch, the real terminal

**Deviation D-03.** TASK-4's e2e tests were written before TASK-3's wiring,
not after. The moon only draws on a terminal, so a tmux pane is the only
place where the wiring's failing test can live. The order changed; no
requirement did.

RED against the unwired binary: 4 of 6 e2e tests failed with
`timed out waiting for the moon`. The two that passed (the off switch and
the clean diagnostic) cannot fail before a moon exists; they were proven by
mutation.

The silent-agent test reads the pane with `capture-pane -J`, because its
diagnostic names the fixture by absolute path and wraps at 100 columns.
That was diagnosed from the real pane, not guessed.

**Deviation D-05.** The plan listed a "separate buffer" test showing that
the answer stream never carries the moon. It was replaced by a stronger
pair of checks. One compares exact bytes on a single shared screen (frame,
erase, answer). The other is an e2e check that the transcript file holds no
moon glyph and no escape sequence.

### TASK-5 — version, README

0.1.1 → 0.1.2 in `crates/ama/Cargo.toml` and `Cargo.lock`. `ama --version`
prints `ama 0.1.2`.

## Pre-existing flaky tests, de-flaked (D-04, D-06, D-07)

Two full `./check.sh` runs each failed on a *different* pre-existing e2e
test and then passed on re-run. Neither was assumed to be flakiness, and
neither was assumed to be pre-existing. Each was measured with an A/B stress
test: 20 runs with all 16 cores saturated, on this branch and on a
worktree of `main` (0.1.1).

| Deviation | Test | 0.1.1 | 0.1.2 before | 0.1.2 after | Cause |
|---|---|---|---|---|---|
| D-04 | `clear_and_trigger_on_one_line_starts_a_fresh_conversation` | 4/20 fail | 5/20 fail | **20/20 pass** | a fixed 300 ms sleep waiting for `clear` |
| D-06 | `the_question_stays_on_screen_and_in_history` | 3/20 fail | 4/20 fail | **20/20 pass** | the same fixed sleep, same race |
| D-07 | both Ctrl-L tests (bash, zsh) | — | 0/20, 1/20 fail | **20/20, 20/20** | a fixed 400 ms sleep waiting for Ctrl-L |

Each fix replaces the sleep with a wait for the event itself, keeps every
assertion, and was mutation-checked (e.g. removing zsh's scrollback erase
still fails the Ctrl-L test). The moon was also ruled out directly: timed
in a real pane with the echo fixture, a turn takes 54–79 ms with the moon
and 53–69 ms without.

## Review fix wave (D-08)

The independent review (07 §5) found one Important issue and ten Minor
ones. The fixes below reach outside the plan's file list, so they are
recorded under SDLC §14. Each started with a failing test or a mutation.

| Finding | Fix | RED evidence |
|---|---|---|
| I-1 stranded frames reach the next prompt | ADR-014: `context::without_moon_frames` and `spinner::strip_frame` | e2e: turn 2's prompt held `🤖 🌑` and `🤖 🌓`; unit: missing function |
| M10 `run` returns with the moon drawn on spawn failure | `EraseOnReturn` drop guard | `left: "\r\x1b[K🤖 🌑"`, no erase |
| M4 `stop()` locks on every chunk | stop once per turn, and `stop()` is lock-free after the erase | refactor; exact-bytes tests unchanged |
| M5 stderr tail lost on a slow terminal | progress-aware drain: idle 250 ms, cap 3 s | 5/5 runs cut off at 600 ms per write |
| M3 stderr inheritance unpinned | `fake-agent-stderr-kind` and 2 e2e tests | mutations: unconditional routing, ignoring `spinner: false` |
| M7 fixed sleeps in the new e2e tests | poll the screen and the transcript | — |

**M5 hid behind M4.** The first M5 test passed against the bug. Every
`stop()` took the spinner lock, so the calls after the answer queued behind
the pump's slow writes, and that quietly lengthened the 250 ms grace. Only
after `stop()` became lock-free did the RED become deterministic, failing 5
runs in 5 in 0.28 s. The test's answer-first ordering and its comment
record why.

## Re-review fix wave (D-09)

A second independent review of the fix wave confirmed all eight
dispositions and found five Minor issues (07 §8). The fixes:

- **N1.** `a_grandchild_that_never_stops_writing_cannot_hold_the_turn_forever`
  pins the 3 s cap. With `STDERR_CAP` set to 3600 s it fails: the turn waited
  13.8 s.
- **N3.** The pump-queue comment now says what the queue really holds: two
  messages per `read`, growing with stderr.
- **N4.** The zsh accept-line test's `clear` plus fixed 300 ms sleep is now
  a condition wait. It held 20 runs in 20 under load *before* the change,
  so this is preventive and recorded as such.
- **Linux margin.** The slow-terminal test is now 400 ms per write and
  ~11 KB. It uses about 0.8 s of the 3 s cap, down from ~2.4 s on a 64 KiB
  pipe, and its RED is unchanged: treating a writing pump as idle still
  fails it 3 runs in 3.
- **N2, N5.** Wording in 02, 03, 04 and README.

## Evidence

`./check.sh` is fully green: rustfmt, clippy on the library and binary
(`-D warnings`, unwrap and expect denied), clippy on the tests, bash, zsh
and install.sh syntax, shellcheck, the build, and the suite single-threaded.
Two further full e2e runs were 33/33 each.

**193 tests pass**, up from 162 at 0.1.1:

| Suite | 0.1.1 | 0.1.2 |
|---|---|---|
| `e2e_shell` | 24 | 33 |
| `integration_cli` | 33 | 34 |
| `unit_config` | 37 | 39 |
| `unit_context` | 21 | 22 |
| `shell_functions` | 20 | 20 |
| `integration_agent` | 9 | 16 |
| `unit_spinner` | — | 10 |
| lib · `unit_render` · `unit_session` | 8 · 5 · 5 | 9 · 5 · 5 |

## SG4 — Build complete

Passed. Deviations D-03 to D-09 are recorded above, and none removes or
weakens a requirement. The independent review ran before this gate, and all
its findings are dispositioned in [07-verification.md](07-verification.md).
