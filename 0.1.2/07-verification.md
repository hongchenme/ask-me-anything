---
software_version: 0.1.2
process_version: 1.0.0
stage: S5-verification
status: in-review
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [06-build-record.md]
updated: 2026-09-26
---

# Verification — 0.1.2

**Environment.** macOS 26 (Darwin 25.6.0, arm64, 16 cores) · rustc 1.98.1 ·
bash 5.3.15 · zsh 5.9 · tmux 3.7c · shellcheck 0.11.0 · Claude Code
2.1.283 · codex-cli 0.155.0. The live runs used the owner's own
`~/.ama/config.yml` (`command: [claude]`) and a release build of 0.1.2.

## 1. The requested behaviour, end to end, against real agents

The owner's words were: *"when a user enters a prompt `@@ whats weather?`,
ama will immediately print 🤖 followed by a animation that cycles through 8
unicode moon phases before the answer is returned by an underlying agent."*

### claude, with the verbatim prompt

The prompt was typed into a real bash in a real tmux pane, and the pane was
sampled every 40 ms from Enter until the prompt came back:

```text
ama doctor   ->  agent  claude -p --allowedTools WebSearch   status ok
first moon at 0.290s; answer began at 14.984s
phase changes seen: 138; distinct phases: 8
sequence: 🌑 🌒 🌓 🌔 🌕 🌖 🌗 🌘 🌑 🌒 🌓 🌔 🌕 🌖 🌗 🌘 🌑 🌒 … (17 full cycles, in order)
moon glyphs on final pane: 0
--- final pane ---
V$ @@ 'whats weather?'
🤖: Right now it's sunny and 74°F (feels like 65°F), with 19 mph wind and 59% humidity. That's
probably not where you are, though. Your IP only resolved to the default middle-of-the-US point
in Kansas, which usually means you're on a VPN. What city are you in? I'll pull the forecast for there.
V$
```

For 15 seconds of web searching the terminal showed `🤖` and a moon moving
through all eight phases, in order. The answer took over the moon's line,
and nothing of the moon was left. The transcript this turn wrote had **0**
moon glyphs and **0** escape sequences. The verification transcript was then
removed with `ama session reset`; the owner's other session files are
untouched.

The 0.290 s runs from `tmux send-keys` to a frame showing up in
`capture-pane`. It covers typing, the hook, 40 ms sampling, and the first
execution of a freshly copied binary. The precise latency figure is
NFR-10, §2.

### codex: stderr keeps its lines (REQ-38, RISK-13)

A temporary `AMA_CONFIG` with `command: [codex]` was used, with the pane in
this repository (codex requires a trusted directory):

```text
answer began at 6.7s
samples with the moon on its own line below agent stderr: 65
lines where a moon shared a line with other text: 0
moon glyphs on final pane: 0
```

The final pane showed codex's own stderr log line by line (`Reading prompt
from stdin...`, the header, hook traces, `tokens used`), each line intact,
then `🤖: ok`. codex ran normally with its stderr piped. This is RISK-13's
live check.

## 2. Non-functional requirements

### NFR-10 — first frame within 100 ms: **met**

A Python pty harness spawned `ama ask` on a real pseudo-terminal from inside
a tmux pane, so the real `tmux capture-pane` runs first. It timed spawn to
the first byte of the first frame. The fixture agent was used, so there was
no cost.

| Build | Where | Runs | Median | p90 | Max |
|---|---|---|---|---|---|
| debug | tmux | 30 | 46.3 ms | 51.8 ms | 54.2 ms |
| release, first run after the build | tmux | 30 | 48.7 ms | 55.1 ms | **423.3 ms** |
| release, warm | tmux | 50 | 47.4 ms | 51.8 ms | 56.3 ms |
| release | no multiplexer | 30 | 23.7 ms | 26.3 ms | 30.9 ms |
| `tmux capture-pane` alone | tmux | 50 | 9.1 ms | — | 14.6 ms |

The single 423 ms outlier happened in the first run after a fresh build and
did not recur in 50 further runs. That fits the one-time cost of a first
execution of a new binary on macOS, though the cause was not isolated. The
first bytes captured were `\r\x1b[K🤖 🌑\r\x1b[K🤖 🌒`: the right frames, in
order, 100 ms apart.

### NFR-11 — the moon never delays the answer: **met**

`stop_does_not_wait_out_the_frame_interval` stops a spinner whose next frame
is 10 s away, and it has been shown to fail (`stop took 9.95s`) when the
wake-up is removed. The integration test `the_moon_is_erased_before_the_
answer_starts` runs a whole turn with a 10 s frame interval and asserts it
finishes in well under that. After the first erase, `stop()` is lock-free,
so the rest of the answer never waits behind the agent's stderr.

### NFR-12 — the moon never fails a turn: **met**

`a_terminal_that_refuses_every_write_does_not_stop_stderr_draining`: every
write fails, nothing panics, and the stderr sink keeps reporting success, so
the agent's pipe is always drained.

Turn time is unaffected. With the echo fixture in a real pane, a turn takes
54–79 ms with the moon and 53–69 ms without (10 runs each).

## 3. Automated suite

`./check.sh` passes every gate, on this machine, after the last change.
**193 tests pass**, `cargo test -- --test-threads=1`, 0 failed, 0 ignored,
against 162 at 0.1.1.

| Requirement | Tests | Level |
|---|---|---|
| REQ-35 | `the_first_frame_is_on_screen_before_start_returns`, `frames_run_through_the_eight_moon_phases_and_wrap` | unit |
| REQ-35 | `the_moon_waxes_while_the_agent_thinks` (verbatim prompt), `the_moon_waxes_in_zsh_too` | e2e |
| REQ-36 | `the_moon_is_erased_before_the_answer_starts`, `a_silent_agent_leaves_no_moon_behind`, `a_spawn_failure_leaves_no_moon_behind` | integration |
| REQ-36 | `stop_erases_the_moon_and_nothing_is_drawn_after_it`, `dropping_the_spinner_erases_the_moon` | unit |
| REQ-36 | `the_moon_waxes_while_the_agent_thinks` (screen and transcript), `a_silent_agent_is_reported_on_a_clean_line` | e2e |
| REQ-36 / RISK-18 | `moon_frames_stranded_on_screen_are_not_context` (unit), `an_enter_pressed_mid_think_never_puts_the_moon_in_the_next_prompt` (e2e) | unit + e2e |
| REQ-37 | `the_moon_needs_a_terminal_that_can_erase_a_line` (unit), `a_piped_answer_carries_no_trace_of_the_moon` (integration) | unit + integration |
| REQ-38 | `a_stderr_line_lands_on_a_cleared_line_and_the_moon_redraws_below_it`, `a_partial_stderr_line_holds_the_moon_until_the_line_completes`, `after_stop_stderr_passes_through_untouched`, `without_a_stderr_writer_there_is_nothing_to_route` | unit |
| REQ-38 | `the_agents_stderr_is_routed_around_the_moon`, `a_slow_terminal_still_gets_the_whole_of_the_agents_stderr` | integration |
| REQ-38 | `agent_stderr_keeps_its_own_lines_while_the_moon_is_up`, `the_agents_stderr_is_routed_only_while_the_moon_shares_its_terminal` | e2e |
| REQ-39 | `the_spinner_is_on_unless_the_config_turns_it_off`, `a_spinner_value_that_is_not_a_boolean_is_rejected` | unit |
| REQ-39 | `the_moon_can_be_turned_off`, `spinner_false_hands_the_agent_its_terminal_back` | e2e |
| NFR-11 / 12 | see §2 | unit + integration |
| RISK-14 | `a_grandchild_holding_the_stderr_pipe_does_not_hold_up_the_turn`, `a_grandchild_that_never_stops_writing_cannot_hold_the_turn_forever` | integration |
| RISK-15 | `an_interrupt_during_the_moon_returns_a_usable_shell` | e2e |

### Every test was seen to fail

The build record has the RED states for each slice. Tests that could not
fail before the feature existed were proven by mutating the finished code,
one mutation at a time, restoring after each:

| Mutation | Fails |
|---|---|
| delete the condition-variable wake-up in `stop` | `stop_does_not_wait_out_the_frame_interval` (after the test was fixed; see 06) |
| `stderr_sink` always `Some` | `without_a_stderr_writer_there_is_nothing_to_route` |
| never hold for a partial stderr line | `a_partial_stderr_line_holds_the_moon_until_the_line_completes` |
| no erase before a stderr chunk | `a_stderr_line_lands_on_a_cleared_line_and_the_moon_redraws_below_it` |
| `StderrSink` propagates the terminal's failure | `a_terminal_that_refuses_every_write_does_not_stop_stderr_draining` |
| no erase in `Drop` | `dropping_the_spinner_erases_the_moon` |
| two phases swapped | `frames_run_through_the_eight_moon_phases_and_wrap` |
| unbounded stderr join | `a_grandchild_holding_the_stderr_pipe…` (waited 5.05 s) |
| give up on a pump that is writing | `a_slow_terminal_still_gets…` ("cut off") |
| treat a reading pump as never idle | `a_grandchild_holding_the_stderr_pipe…` (waited 3.03 s) |
| `STDERR_CAP` 3 s → 3600 s | `a_grandchild_that_never_stops_writing…` (waited 13.8 s) |
| ignore `spinner: false` | `the_moon_can_be_turned_off`, `spinner_false_hands_the_agent_its_terminal_back` |
| no stop before `run`'s own diagnostic | `a_silent_agent_is_reported_on_a_clean_line` |
| draw the moon before the pane capture | `the_moon_waxes_while_the_agent_thinks`, with `🤖 🌑` echoed back inside the prompt |
| never route stderr | `agent_stderr_keeps_its_own_lines_while_the_moon_is_up` |
| always route stderr | `the_agents_stderr_is_routed_only_while_the_moon_shares_its_terminal` |
| draw on a pipe | `a_piped_answer_carries_no_trace_of_the_moon` |
| drop zsh's scrollback erase | `ctrl_l_clears_the_screen_and_scrollback…_in_zsh` (after D-07) |

## 4. Regression checks

| Check | Result |
|---|---|
| Pre-existing tests | All pass. Changes were limited to the `agent::run` signature (`None` in 9 calls) and to e2e tests whose fixed sleeps became condition waits, assertions unchanged: three observed flaky under load with A/B evidence against 0.1.1 (D-04, D-06, D-07), and one preventive (D-09) |
| Output to a pipe | Byte-identical: all 33 pre-existing CLI tests pass unmodified, plus an explicit no-ESC/no-moon test |
| Shell integration | `ama.bash`/`ama.zsh` untouched; NFR-01 unaffected |
| 0.1.1 configs | Load unchanged, and get the moon (`spinner` defaults to `true`) |
| `spinner: false` | No moon, stderr inherited: 0.1.1 behaviour, pinned by e2e |
| Turn time | 54–79 ms with the moon, 53–69 ms without |

## 5. Independent review

The reviewer was a separate agent with a fresh context, given the spec, the
design, the plan and the diff, but not this session's history. It worked
read-only (verified: `git status` identical before and after). It ran
`./check.sh` and built its own harnesses in `/tmp`, then deleted them.
Verdict: **"With fixes"**. It found no Critical issue, and found the
concurrency and process handling correct: "no deadlock, no leaked child, no
unbounded join, and no path by which a frame reaches a pipe, file or
transcript."

| Finding | Severity | Disposition |
|---|---|---|
| I-1 An Enter mid-think strands frames, and the next turn sends them as context | Important | **Fixed**: ADR-014, reproduced first (see 06), unit + e2e tests. Screen residue accepted as RISK-18 |
| M1 The first frame erases a partial line already printed | Minor | **Documented**: RISK-19, ADR-011 amendment |
| M2 README: `@@ … \| less` pipes nothing; "nothing is left" is absolute | Minor | **Fixed** in README |
| M3 No test pins stderr inheritance | Minor | **Fixed**: new fixture, 2 e2e tests, mutation-checked |
| M4 `stop()` locks on every stdout chunk | Minor | **Fixed**: stop once, lock-free after the erase |
| M5 The 250 ms grace can cut off the stderr tail on a slow terminal | Minor | **Fixed**: progress-aware drain (idle 250 ms, cap 3 s), deterministic RED at 600 ms per write |
| M6 `std::thread::spawn` panics if a thread cannot be made | Minor | **Declined**: same pattern as the existing stdin writer. It only fails on resource exhaustion, where the panic at least surfaces the failure. A fallback cannot un-pipe a child that is already running |
| M7 A fixed sleep in the new e2e tests | Minor | **Fixed**: polls the screen and the transcript |
| M8 Rollback trap: 0.1.1 rejects `spinner:` | Minor | **Documented** in 02 §7 and 08 |
| M9 No cycle index or build record | Minor | **Fixed**: [README.md](README.md), [06](06-build-record.md) |
| M10 `run` returns with the moon drawn on spawn failure | Minor | **Fixed**: drop guard, test-first |
| Recommendation: stderr/answer ordering now depends on scheduling | — | **Recorded** in ADR-012's consequences (0 reorders in 1,500+ reviewer runs) |

**Declined-to-judge list, ruled on:**

| Item | Ruling |
|---|---|
| Ctrl-C/SIGTERM/SIGHUP leave a frame | RISK-15; the screen residue is accepted, and the context residue is now stripped (ADR-014) |
| Backgrounded `ama` animates over the prompt | RISK-16, accepted |
| A grandchild's later writes raise SIGPIPE | RISK-14 reworded to say so |
| The agent's Ctrl-C shutdown messages are lost | New RISK-20, accepted |
| RISK-13 needs live agents | Done in §1 (claude, codex) |
| NFR-10 unmeasured | Done in §2 |
| A partial stderr line glued to the answer | Identical to 0.1.1; no action |
| A panic prints beside the moon | No reachable panic path found; no action |
| Patch rather than minor version | Owner's direction |
| Glyph width without emoji fonts | Pre-existing with `🤖:`; no action |

The fix wave was re-reviewed by a second independent agent; see §8.

## 6. Residual risk

| ID | State |
|---|---|
| RISK-13 | Mitigated and live-checked (claude, codex); `spinner: false` restores inheritance, pinned by e2e |
| RISK-14 | Mitigated; bounded (idle 250 ms, cap 3 s), pinned both ways |
| RISK-15, RISK-18 | Frames the user's keys strand stay on screen (cosmetic); never sent as context |
| RISK-16, RISK-19, RISK-20 | Accepted; each has `spinner: false` or a pipe as a workaround |
| F-12 (from 0.1.1) | Still open: the agent sometimes copies `🤖:` into its answer. Not in this cycle's scope; carried forward |
| Remaining fixed sleeps in `e2e_shell.rs` | Reviewed twice. An earlier version of this line said they could only pass wrongly. The re-review showed that was false for one: the zsh accept-line test had D-06's shape (`clear`, a fixed sleep, an "is it clear" check). It held 20/20 under load and now waits for the event anyway (D-09). The rest sit between keystrokes, where tmux keeps key order |
| GNU screen | *From the re-review.* The capture path filters screen hardcopies too, but no test drives screen at all, and that predates this cycle. macOS's bundled screen 4.00.03 writes every non-BMP glyph as byte 0xFD in `hardcopy`. So there the moon filter, like the existing `🤖:` answer detection, never sees the glyphs. Modern screen is untested. Carried forward |

## 7. Release recommendation

**Recommend release.**

- The requested behaviour works as asked against the owner's real agent,
  with the owner's verbatim prompt.
- All 193 tests pass, and every `check.sh` gate is green.
- Every test has been seen to fail.
- The independent review's one Important finding is fixed, and its Minor
  findings are fixed or dispositioned.

The implementer cannot grant SG5 (SDLC §11). On 2026-09-26 the owner, as
release authority, directed the release: *"push to remote version branch,
create a PR to merge to main and approve the PR via code owner overwrite.
Then git push a new tag 0.1.2 to trigger the release."* The decisions below
were taken without the owner. That directive did not address them
individually, so they stay listed for confirmation; each is reversible:

1. **Q-03 / ADR-013**: an off switch (`spinner: false`) that was not asked
   for.
2. **Q-04 / ADR-012**: `ama` takes over the agent's stderr while the moon
   shows.
3. **ADR-014**: the context filter, and accepting frames stranded on screen
   (RISK-18).

## 8. Re-review of the fix wave

A second independent agent, again with a fresh context and read-only,
checked each disposition above against the code. It re-ran `./check.sh`,
ran a mutation for every fix, stress-tested the lock-free `stop()` (1,000
iterations of 4 concurrent callers; moving the flag before the erase gave
357 failures), and drove live bash and zsh panes. Its verdict:
**no blocking issues.** All eight dispositions were **CONFIRMED**,
including agreement with declining M6. It found five Minor issues, all
now addressed:

| Finding | Disposition |
|---|---|
| N1 The 3 s cap was untested (3600 s passed everything) | **Fixed**: `a_grandchild_that_never_stops_writing…`, mutation-checked (13.8 s) |
| N2 REQ-38 and RISK-14 overclaimed: the cap also truncates on a terminal slower than ~24 KB/s | **Fixed**: both now state the limit |
| N3 The queue comment said "per 8 KiB"; it is per `read` | **Fixed** in `agent.rs` and ADR-012, with the growth stated |
| N4 §6 wrongly said the remaining fixed sleeps could not fail wrongly | **Fixed**: claim corrected; the one such test given a condition wait (D-09) |
| N5 Wording: other signals strand frames too; a chained trigger can follow a partial line | **Fixed** in README, ADR-011 and RISK-19 |
| Declined: GNU screen at runtime | Recorded in §6 and carried forward |
| Declined: the slow-terminal test's margin on a 64 KiB Linux pipe (~2.4 s of 3 s) | **Acted on**: now 400 ms × ~11 KB, about 0.8 s of the cap, still deterministic RED (3/3) |

That second fix wave was not itself sent back for a third review. Each item
carries its own RED or mutation evidence (06, D-09).

## SG5 — Verification accepted

**Owner directed release, 2026-09-26** (quoted in §7). The three
implementer decisions in §7 remain open for the owner's confirmation.
