---
software_version: 0.1.2
process_version: 1.0.0
stage: S2-specification
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [01-intent.md, 02-discovery-and-risk.md]
updated: 2026-09-26
---

# Product requirements — 0.1.2

Numbering continues from 0.1.1, which ended at REQ-34 and NFR-09. Every
earlier requirement stays in force. This document adds requirements and
amends none.

"The moon" below means the indicator: `🤖`, a space, and one moon-phase
glyph, redrawn in place.

## Functional requirements

### REQ-35 — the moon shows from Return until the answer

When a question runs with stdout on a terminal, `ama` prints `🤖` followed by
a space and a moon phase. It advances through exactly these eight glyphs,
in this order, then repeats:

```text
🌑 🌒 🌓 🌔 🌕 🌖 🌗 🌘
```

It advances every 100 ms. The moon is drawn on the line where the answer
will appear. Its first frame is on screen before the agent process is
started.

**Acceptance**

- The owner's verbatim `@@ whats weather?`, against an agent that thinks for
  2 s, shows `🤖 ` + a moon phase within 1 s of Return, while no `🤖:` answer
  is on screen yet.
- At least three different phases are seen during that think: an
  animation, not a static glyph.
- Frames are drawn in the order above and wrap from 🌘 back to 🌑.
- The same happens for `ama ask -- <question>` run at a prompt.

### REQ-36 — the answer replaces the moon without a trace

The moon is erased the moment the agent's first byte of output arrives. The
answer then renders exactly as it does in 0.1.1 (`🤖: ` then the text,
continuation lines indented), starting on the line the moon occupied.

**Acceptance**

- Once a turn finishes, the pane contains **no** moon glyph. This covers the
  pane-scrape context the next turn sends.
- The moon's bytes never reach the answer stream. So the transcript
  (`~/.ama/sessions/*.jsonl`) and the answer's own bytes are identical to
  0.1.1's.
- When the agent fails or exits silently, the moon is erased *before* any
  `ama:` diagnostic is printed. The diagnostic starts on a clean line.
- *Added after the SG4 review.* A frame left on screen by the user's own
  keys (Enter or Ctrl-C while the moon is up, RISK-15 and RISK-18) is never
  part of a later turn's context. The frame itself may stay on screen.

### REQ-37 — only on a terminal

No moon is drawn when stdout is not a terminal (a pipe, a file, `$(…)`),
when `TERM=dumb`, or when `spinner: false` is set (REQ-39).

**Acceptance**

- `ama ask -- q` with stdout on a pipe writes exactly `🤖: <answer>\n`: no
  escape byte, no moon glyph.
- Every 0.1.1 integration test passes unmodified. Those tests run ama with
  stdout on a pipe.

### REQ-38 — agent stderr never collides with the moon

While the moon is showing, every chunk of the agent's stderr is written to
a cleared line, and the moon is redrawn below it. A stderr line that has
not yet ended keeps the moon hidden until it does. Once the moon has
stopped, stderr passes through untouched.

**Acceptance**

- An agent that writes `progress one` to stderr, then the partial line
  `partial`, then ` line`, pausing between each, and only then answers,
  leaves a screen whose lines read exactly `progress one`, `partial line`,
  `🤖: <answer>`. Nothing is glued to either stderr line, and no glyph is
  left.
- While that agent pauses after a complete stderr line, the moon is visible
  on the line below it.
- If `ama`'s stderr is not a terminal, or the moon is off, the agent's
  stderr is inherited exactly as in 0.1.1.
- *Added after the SG4 review, corrected by the re-review.* All of the
  agent's stderr reaches the screen, including what is still on its way
  when the agent exits and a slow terminal takes a while to accept, within
  two limits. Once the agent has exited, `ama` stops waiting after 250 ms
  with nothing to deliver (a pipe held open by a process that outlives the
  agent, RISK-14), and after 3 s in all. The 3 s cap can cut off the tail
  only when the terminal is slower than about 24 KB/s and a full pipe is
  still outstanding.

### REQ-39 — the moon can be turned off

A top-level `spinner:` config key takes `true` (the default) or `false`.
`false` means no moon and no stderr routing: 0.1.1's output, exactly.

**Acceptance**

- A config without the key shows the moon.
- `spinner: false` shows no moon while the agent thinks, and the answer
  renders normally.
- A value that is not a boolean is a configuration error, exit 2. It is not
  silently read as either value.

## Non-functional requirements

### NFR-10 — the first frame is immediate

The first frame is drawn within **100 ms** of `ama` starting on a terminal.
That includes loading the config and capturing the tmux pane. It is
measured, not estimated, and recorded in 07-verification.md.

### NFR-11 — the moon never delays the answer

Stopping the moon wakes its thread at once. It never waits out the rest of
a 100 ms frame. The answer's first byte reaches the screen as soon as the
erase has been flushed. Pinned by a test whose frame interval is 10 s.

### NFR-12 — the moon never fails a turn

The moon draws on a best-effort basis. A failed write, or a drawing thread
that could not be started, never changes the answer, the exit code or the
transcript.

## Traceability

| Requirement | Decision | Test level |
|---|---|---|
| REQ-35 | ADR-010, ADR-011 | unit (frame order), e2e (verbatim prompt, bash + zsh) |
| REQ-36 | ADR-011 | integration (answer stream untouched, erase-then-answer order), e2e (no glyph left, clean diagnostic) |
| REQ-37 | ADR-011 | integration (pipe), unit (`TERM=dumb`) |
| REQ-38 | ADR-012 | unit (clear, write, redraw, hold), integration (routing), e2e (real pane) |
| REQ-39 | ADR-013 | unit (config), e2e (off in a real pane) |
| NFR-10 | ADR-011 | measured in 07 |
| NFR-11 | ADR-011 | unit + integration (10 s tick) |
| NFR-12 | ADR-011 | unit (failing writer) |
| RISK-14 | ADR-012 | integration (grandchild holds the pipe; 600 ms-per-write terminal) |
| RISK-15 | ADR-014 | e2e (Ctrl-C during the moon, exit 130); unit (`^C` residue stripped) |
| RISK-18 | ADR-014 | unit (stranded frames stripped), e2e (Enter mid-think, next prompt clean) |

## Amended after the SG4 review

REQ-36 and REQ-38 each gained one acceptance criterion, both from reviewer
findings (Important-1 and M5; dispositions in
[07-verification.md](07-verification.md#5-independent-review)). No
requirement was removed or weakened.

## SG2 — Specification accepted

Recorded 2026-09-26 under the [process exception](README.md#process-exception).
