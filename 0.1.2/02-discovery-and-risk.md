---
software_version: 0.1.2
process_version: 1.0.0
stage: S1-discovery
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [01-intent.md]
updated: 2026-09-26
---

# Discovery and risk — 0.1.2

Every finding below comes from running the real tools on the owner's
machine. The commands are recorded so they can be re-run.

**Environment.** macOS 26 (Darwin 25.6.0, arm64) · bash 5.3.15 · zsh 5.9 ·
tmux 3.7c · rustc 1.98.1 · shellcheck 0.11.0 · Claude Code 2.1.283 ·
codex-cli 0.155.0 · terminal: iTerm2 (`TERM_PROGRAM=iTerm.app`).

This is the first cycle built on macOS; 0.1.0 and 0.1.1 were built on
Linux. `rust`, `tmux` and `shellcheck` were installed with Homebrew. On a
clean `main`, before any change, `./check.sh` was green on this machine,
every gate included.

## 1. How long the silence is

**F-13. `claude -p` is silent for the whole think, then answers at once.**

```console
$ printf 'Reply with exactly the word: ok\n' | claude -p --allowedTools WebSearch
ok                                    # elapsed 6.1s, stdout 3 bytes, stderr 0 bytes
```

That is 6.1 s of nothing for a three-byte answer. In text mode `claude -p`
prints its result when it is done, not as it goes. So for the default
agent, the indicator covers the entire wait. It also writes nothing to
stderr.

## 2. Agents that talk on stderr

**F-14. `codex exec` writes progress to stderr immediately, and a lot of
it.**

```console
$ printf 'Reply with exactly the word: ok\n' | codex --search exec >out 2>err
$ wc -c out; wc -l err
3 out                       # "ok"
30 err
$ head -3 err
Reading prompt from stdin...
2026-09-26T17:47:42Z ERROR rmcp::transport::worker: worker quit with fatal: ...
OpenAI Codex v0.155.0
```

The first stderr line appears before codex has even read its prompt. Then
comes a header, hook traces, an echo of the answer, and a token count. Only
the final answer goes to stdout.

`ama` spawns the agent with its stderr inherited. So all of that goes to the
same terminal line an animation would be drawing on. If the animation
redraws on that line and codex writes to it unannounced, every stderr line
gets a moon glued onto it:

```text
🤖 🌑Reading prompt from stdin...
🤖 🌒OpenAI Codex v0.155.0
```

That is the interleaving you get when both write to one line. It is shown
here worked out from how the two write, not captured from a build. It is
why stderr needs a design decision (ADR-012) rather than a default.

## 3. The screen is the next prompt

**F-15. Drawing the indicator before the pane is captured puts the
indicator in the capture.**

Inside tmux, `ama` runs `tmux capture-pane -p` to build context
(`context::gather`). If the first frame were drawn first, this turn's
context would end with a `🤖 🌑` line. `context::is_answer_line` would not
recognise it, because it has no colon. So the line would go to the agent as
if the user had typed it. So the capture has to come first.

**F-16. CR + EL leaves nothing behind for a later capture.** Checked in a
real tmux pane, with the exact byte sequences the design uses:

```console
$ printf '\r\033[K🤖 🌑'; sleep 1; printf '\r\033[K🤖 🌔'; sleep 1; printf '\r\033[K🤖: the answer\n'
# capture-pane at t≈0.5s  ->  "🤖 🌑"
# capture-pane at t≈1.5s  ->  "🤖 🌔"
# capture-pane after      ->  "🤖: the answer"      (no glyph, no stray cell)
```

So `\r\033[K` ("return, erase to end of line") fully removes a frame, wide
emoji included. The finished screen is exactly the 0.1.1 screen.

## 4. Where it can live

| Location | Verdict |
|---|---|
| The shell hook (`ama.bash`/`ama.zsh`) | **Rejected.** The hook runs inside the line editor *before* the command. Once the command runs, the shell is busy and cannot animate. A background job would need job control and would race the command's output. The hook also carries NFR-01's 50 ms budget |
| The `ama` binary | **Chosen.** It owns the agent's lifetime and sees the exact moment the first byte arrives. It also covers `ama ask` at a prompt with the same code |

## 5. Assumptions

| ID | Assumption | Basis | If wrong |
|---|---|---|---|
| AS-05 | Terminals honour CR and EL (`\033[K`) | EL is VT100 and universally implemented; checked in tmux (F-16) | Frames would pile up on one line. `TERM=dumb` and `spinner: false` both switch it off |
| AS-06 | A terminal that `TERM=dumb` describes cannot erase a line | `dumb` is the terminfo entry for "no cursor control". Emacs `M-x shell` sets it and shows `\033[K` literally | The spinner would be suppressed where it might have worked. Harmless |
| AS-07 | `claude -p` in text mode prints nothing until it is done | F-13 | Harmless. The indicator stops at the first byte whenever that comes |

## 6. Risk register

New entries only. RISK-01…RISK-12 carry over unchanged.

| ID | Risk | Tier | Owner | Disposition |
|---|---|---|---|---|
| RISK-13 | An agent behaves differently when its stderr is a pipe instead of the terminal: drops colour, hides its own progress, or in the worst case refuses to run | R2 | solo-founder | **Mitigated.** Routing happens only while the moon is showing *and* `ama`'s own stderr is a terminal. `spinner: false` restores 0.1.1 inheritance exactly. Checked live against claude and codex in S5 |
| RISK-14 | A grandchild that inherited the agent's stderr pipe outlives the agent. `ama` would wait forever for end-of-file. Once `ama` exits, the grandchild's next stderr write raises SIGPIPE, whose default action terminates the process | R2 | solo-founder | **Mitigated, residual accepted.** After the agent exits, `ama` gives up on a pump that has sat *waiting* on the pipe for 250 ms. It keeps waiting while bytes are being delivered, so a slow terminal still gets the tail unless delivering it takes more than 3 s (a terminal slower than about 24 KB/s with a full 64 KiB Linux pipe outstanding). It never waits more than 3 s in all (ADR-012, amended). Pinned by tests that plant such a grandchild and that deliver stderr at 600 ms per write. stdout has had the same exposure since 0.1.0 (it is read to end-of-file), with no report |
| RISK-15 | Ctrl-C while the moon is up leaves the last frame on screen (`🤖 🌓^C`): `ama` dies by SIGINT with no cleanup | R1 | solo-founder | **Screen residue accepted; context mitigated.** The frame stays on screen. The next turn's capture strips it and keeps only `^C` (ADR-014). The cursor is never hidden, so the terminal stays fully usable. A SIGINT handler would need a new dependency or `unsafe` (the crate forbids it) and would put at risk the exit-130 contract pinned by `an_interrupt_returns_a_usable_shell` |
| RISK-16 | A backgrounded `ama` with stdout still on the terminal (`(@@ q) &`, `ama ask -- q &`) animates over the user's prompt line | R1 | solo-founder | **Accepted.** The `@@` trigger itself cannot background: the hook quotes a trailing `&` into the question. The remaining forms are deliberate and rare, and `spinner: false` or a pipe avoids it. Detecting a background process group needs `tcgetpgrp`, which means `unsafe` or a new dependency |
| RISK-17 | The animation lands in the prompt context of the turn that draws it (F-15), or survives into a later one | R2 | solo-founder | **Mitigated, two layers.** ADR-011: capture first, then draw, and erase with CR+EL. That keeps an ordinary turn clean, pinned by an e2e test that fails on a single leftover glyph. ADR-014: every capture strips frames stranded by the user's keys (RISK-18) |
| RISK-18 | *Found by the SG4 review.* Enter pressed while the moon is up is echoed by the terminal, which moves the cursor down and strands the last frame on the line above. Before ADR-014 the next turn sent those frames to the agent as if typed (reproduced: turn 2's prompt held `🤖 🌑` and `🤖 🌓`) | R2 | solo-founder | **Context mitigated, screen residue accepted.** ADR-014 strips stranded frames from every capture, pinned by an e2e test that presses Enter twice mid-think. The frames stay on screen: preventing that means turning off terminal echo, which needs termios (`unsafe` or a new dependency), and a Ctrl-C would then leave the terminal without echo |
| RISK-19 | *Found by the SG4 review.* The first frame begins with `\r\033[K`, so it erases anything already on the cursor's line: `printf 'Weather: '; ama ask -- q` loses `Weather: ` | R1 | solo-founder | **Accepted.** A trigger typed on its own starts at column 0, right after Enter. What is affected is anything that writes a partial line first: `ama ask` after `printf 'Weather: '` in a script, or `printf 'Weather: '; @@ q` typed as one line. `spinner: false` restores 0.1.1 output. Keeping the prefix needs absolute cursor positioning, which breaks as soon as the screen scrolls |
| RISK-20 | On Ctrl-C, `ama`, the reader of the agent's stderr pipe, dies along with the agent. Any shutdown message the agent writes to stderr afterwards is lost, and the write raises SIGPIPE | R1 | solo-founder | **Accepted.** Only while the moon is showing. `spinner: false` gives the agent the terminal itself again |

## 7. Risk tier

**R2 — user-facing.** It changes what every interactive turn prints. It
also changes how an agent's stderr reaches the user while the moon is
showing. R3 was considered and rejected: no identity, payment, private
document, migration or production access is involved. Rollback is
`spinner: false` or reinstalling 0.1.1. To reinstall 0.1.1, first remove
any `spinner:` line: 0.1.1 rejects keys it does not know (`deny_unknown_
fields`) and would refuse every turn with exit 2.

## SG1 — Discovery accepted

Recorded 2026-09-26 under the exception in [README.md](README.md#process-exception):
the owner directed an uninterrupted cycle, so SG1 was not separately
reviewed. The open questions Q-03 and Q-04 were resolved by the implementer
in S2 and are listed for owner confirmation at SG5.

## Amended after the SG4 review

The independent review found RISK-18 and RISK-19 and sharpened RISK-14 and
RISK-20 (its "declined to judge" list). RISK-15 and RISK-17 were updated for
ADR-014, and the rollback note gained the `spinner:` key trap. Each change
comes from a reviewer finding and its disposition in
[07-verification.md](07-verification.md#5-independent-review). None changes
the risk tier.
