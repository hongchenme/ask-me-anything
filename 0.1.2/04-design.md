---
software_version: 0.1.2
process_version: 1.0.0
stage: S2-specification
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [03-product-requirements.md]
updated: 2026-09-26
---

# Design — 0.1.2

There is one new module, `spinner.rs`. Three existing modules learn about it:
two drive it, and one (after the SG4 review) cleans up after it. The shell
integration is not touched.

```text
cli.rs::ask   load config -> capture context -> start Spinner -> agent::run -> drop Spinner
                                                    |                 |
spinner.rs    draws "🤖 <moon>" on stdout  <---------+                 |
              erases on stop; clears/redraws around stderr            |
                                                                      |
agent.rs      first stdout byte  -> spinner.stop()   (REQ-36)  <------+
              stdout closed      -> spinner.stop()   (before any diagnostic)
              every return       -> spinner.stop()   (EraseOnReturn guard)
              stderr, while spinning on a tty -> piped through the spinner (REQ-38)

context.rs    every pane capture -> without_moon_frames   (ADR-014, RISK-18)
```

## ADR-010 — the moon lives in the binary, not the shell hook

**Context.** The indicator has to run for as long as the agent does, and
stop at the agent's first byte.

**Decision.** The `ama` binary draws it. The shell hook runs *before* the
command, inside the line editor, and cannot run anything while the command
runs (02 §4). The binary is the only component that can see the first byte
arrive. Because `ama ask` shares the code path, it gets the moon too.

**Consequences.** `ama.bash` and `ama.zsh` are unchanged, so NFR-01 is not
affected. The moon works in any shell, because it is not a hook feature.

## ADR-011 — draw on stdout, after the capture, and erase with CR + EL

**Decision.**

| Question | Choice | Why |
|---|---|---|
| Which stream? | **stdout** | The moon holds the line the answer will take over. If stdout is redirected there is no such line, and a moon on stderr would show while the answer went somewhere else |
| When is it drawn? | Only when stdout is a terminal, `TERM` is not `dumb`, and `spinner` is not `false` | REQ-37. `std::io::IsTerminal`, no new dependency |
| When does it start? | After the pane capture and before the agent is spawned. The first frame is written before `Spinner::start` returns | F-15: drawn earlier, it lands in its own context. Starting before spawn keeps it immediate (NFR-10) |
| How is a frame drawn? | `\r\033[K` then `🤖 <moon>`, then flush | CR + EL erases a frame completely, wide glyphs included (F-16) |
| How is it erased? | `\r\033[K`, then flush, before the answer's first byte and before any diagnostic | REQ-36 |
| Frame rate | 100 ms: a full cycle of 8 every 0.8 s | Standard moon-spinner cadence. Ten small writes a second costs nothing |
| Hide the cursor? | **No** | Hiding it needs `\033[?25l`. A Ctrl-C (RISK-15) would then leave the user's terminal with no cursor, and nothing would restore it |

**Threading.** One thread per spinner, sleeping on a condition variable
between frames. `stop()` sets a flag under the lock, erases, and notifies.
The drawing thread therefore wakes at once instead of finishing its
100 ms (NFR-11). `stop()` then joins it, so no frame can follow the erase.
`stop()` is idempotent, and `Drop` calls it. Every write result is
ignored: the moon is decoration (NFR-12). If the drawing thread cannot be
spawned, the first frame stays up without moving until it is erased.

**Amended after the SG4 review.**

- After the first call, `stop()` returns on an atomic flag without taking
  the screen lock. The flag is set only once the erase is complete, so a
  caller that sees it knows no frame can follow. This keeps the rest of the
  answer, and the calls after the answer, from waiting behind a stderr
  write to a slow terminal. That waiting once hid a real bug from a test
  (06-build-record, TASK-2 review fixes).
- `agent::run` erases the moon on every return path through a drop guard,
  not only on the paths where it prints something itself.
- The first frame's `\r\033[K` erases whatever was already on the cursor's
  line (RISK-19). A trigger typed on its own starts at column 0. What is
  affected is anything that writes a partial line first: `ama ask` in a
  script, or a chained `printf 'Weather: '; @@ q`.

**Rendering is unchanged.** The answer still flows
agent → `Robot` → `Tee` → stdout + transcript. The spinner writes to its own
stdout handle and never through `Tee`, so the transcript cannot contain it.

## ADR-012 — while the moon shows on a terminal, the agent's stderr comes through `ama`

**Context.** An agent's inherited stderr and the moon share one screen
line. codex writes 30 lines of it, starting at once (F-14).

**Options considered.**

| Option | Rejected because |
|---|---|
| Leave stderr inherited | Every codex progress line gets a moon glued on (02 §2) |
| Stop the moon for good at the first stderr byte | codex writes stderr immediately, so codex users would never see the moon. It would also be missing during the long reasoning gaps between codex's lines |
| Only show the moon for agents known to be quiet | A per-vendor list. It breaks BYOA for every agent not on it |
| **Pipe stderr through the spinner: erase, write the chunk, redraw below** | **Chosen** |

**Decision.** When the spinner was given a stderr writer, `agent::run`
spawns the agent with `stderr(Stdio::piped())` and a pump thread copies the
pipe through `spinner::StderrSink`. The spinner gets that writer only when
`ama`'s own stderr is a terminal. For each chunk, under the spinner's lock:

1. erase the moon if it is drawn;
2. write the chunk to stderr and flush;
3. if the moon is still running: when the chunk ends in `\n`, redraw the
   current frame; otherwise hold (no frames drawn) until a chunk does.

After `stop()`, chunks are written through untouched.

Once the agent has exited, the pump is waited for as long as it is
*delivering*. A slow terminal can take seconds to accept the rest of a big
dump, and its tail is usually where the error is. The wait gives up on a
pump that has sat *waiting on the pipe* for **250 ms**, and in any case
after **3 s**. A grandchild that inherited the pipe can hold it open
indefinitely, and that must not hang the shell (RISK-14). The pump reports
every switch between reading and writing on a channel, and the waiter
replays them, so the last one tells it what the pump is doing now. That is
two small messages per `read`, usually per line of stderr, so the queue
grows with the agent's stderr (about 16 bytes a message) until the turn
ends. That is harmless at any realistic volume, and bounding it would mean
polling an atomic state instead.

*Amended after the SG4 review* (finding M5). The first design joined for
at most 250 ms after the agent exited, however busy the pump was. A
terminal slower than about 32 KB/s would have lost the tail of a 64 KiB
Linux pipe.

**Consequences.**

- The agent's stderr is a pipe while the moon is on, so an agent that
  colours its stderr only on a terminal stops colouring it (RISK-13).
- In every other case (a non-terminal stderr, a non-terminal stdout,
  `spinner: false`) stderr is inherited, byte-for-byte as in 0.1.1.
- Forwarded stderr is never written to the transcript. It never was.
- In 0.1.1 the agent's stderr went straight to the terminal, so anything it
  wrote before its answer appeared before the answer. Now that ordering is
  up to thread scheduling. The reviewer ran over 1,500 attempts to provoke
  a reorder (idle, all cores saturated, 2–20 ms simulated terminal latency)
  and got none. Recorded, not fixed.

## ADR-013 — `spinner: false` turns it off

**Context.** Discovery produced three reasons for an off switch, none of
them cosmetic:

1. **Accessibility.** WCAG 2.2.2 (Pause, Stop, Hide) asks for a way to stop
   moving content that starts on its own and runs for more than 5 s. The
   moon runs for the agent's whole think, 6 s or more even for a trivial
   question (F-13). Screen readers can announce each redraw.
2. **Rollback.** It is the only way back to 0.1.1's output short of
   reinstalling.
3. **RISK-13's escape hatch.** It restores stderr inheritance for any agent
   that misbehaves when its stderr is a pipe.

**Options considered.**

| Option | Rejected because |
|---|---|
| No switch | Fails all three reasons above |
| An environment variable | Undiscoverable, and not where `ama` is configured. `ama doctor` points at the config file |
| **A top-level boolean in `~/.ama/config.yml`** | **Chosen**, default `true` |

**Decision.** `Config` gains `spinner: bool` with a serde default of `true`.
It is a real `bool`, so `spinner: off` or `spinner: maybe` is a parse error
(exit 2), never read as either value. This matches REQ-32's rule for
`tools:`.

This is scope the owner did not ask for. It is listed for confirmation at
SG5 (Q-03).

## ADR-014 — a capture strips moon frames the user's keys left behind

*Added after the SG4 review* (finding Important-1, RISK-18).

**Context.** While `ama` runs, the terminal is in cooked mode with echo on.
Enter pressed during the moon is echoed as a newline: the cursor moves
down, the moon carries on below, and the last frame stays on the line
above. Ctrl-C leaves `🤖 🌓^C` the same way (RISK-15). The next turn
captured those lines and sent them to the agent as if typed. That was
reproduced end to end before this ADR: turn 2's prompt held `🤖 🌑` and
`🤖 🌓`.

**Options considered.**

| Option | Rejected because |
|---|---|
| Turn off terminal echo while the moon shows | Needs termios (`unsafe` or a new dependency). A Ctrl-C would leave the terminal without echo, the same hazard that ruled out hiding the cursor (ADR-011) |
| Draw at a saved cursor position (`\0337`/`\0338`) | A saved position is a screen row, not a line of text. At the bottom of a full terminal, where prompts usually are, the echoed Enter scrolls the screen and the saved row then points at the wrong line |
| **Strip frames from every capture** | **Chosen.** It fixes the part that matters, what reaches the agent, and it also cleans RISK-15's residue out of the next prompt |

**Decision.** `context::without_moon_frames` runs on every pane capture,
before slicing. A line that starts with a frame loses the frame. If nothing
is left, the line is dropped; otherwise what follows is kept (`^C`, or keys
echoed onto the frame's line). The frame format is recognised by
`spinner::strip_frame`, next to the code that draws it, so the two cannot
drift apart.

**Consequences.** A stranded frame can stay on screen. That is cosmetic and
recorded as residual risk. A line that some other program printed starting
with `🤖 ` and a moon phase would lose those two glyphs from the context.

## Interfaces changed

| Symbol | Before | After | Callers |
|---|---|---|---|
| `spinner::Spinner` | — | new: `for_terminal() -> Option<Spinner>`, `start(term, stderr, every)`, `stop()`, `stderr_sink()` | `cli::ask`, `agent::run` |
| `spinner::StderrSink` | — | new: `io::Write`, clones the spinner's shared state | `agent::run`'s pump |
| `agent::run` | `(spec, input, out)` | `(spec, input, out, spinner: Option<&Spinner>)` | `cli::ask`; tests pass `None` |
| `config::Config` | `{agent, max_context_lines}` | `+ spinner: bool` (default `true`) | `cli::ask` |
| `spinner::strip_frame` | — | new: `(&str) -> Option<&str>` | `context` |
| `context::without_moon_frames` | — | new: `(Vec<String>) -> Vec<String>` | `context::gather` |

No command-line, shell or file-format interface changes. A 0.1.1 config
loads unchanged and gets the moon.

## SG2 — Design accepted

Recorded 2026-09-26 under the [process exception](README.md#process-exception).
