---
software_version: 0.1.2
process_version: 1.0.0
stage: S0-intent
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [../AGENT_NATIVE_SDLC.md, ../0.1.1/README.md]
updated: 2026-09-26
---

# INT-0.1.2 — show that the agent is thinking

## Problem

The owner asked for this feature, verbatim:

> In a patch version 0.1.2, add this feature: when a user enters a prompt
> `@@ whats weather?`, ama will immediately print 🤖 followed by a animation
> that cycles through 8 unicode moon phases before the answer is returned by
> an underlying agent.

Right now, pressing Return on `@@ <question>` produces **nothing at all**
until the agent's first byte arrives. That gap is long. Measured on the
owner's machine (02 §1), `claude -p` is silent for 6.1 s on a question
whose whole answer is the word `ok`. A weather question that needs a web
search takes longer. For all of that time the terminal looks exactly like a
hung command.

## Why it matters

0.1.1 made `@@ whats weather?` actually answer. It still *feels* broken for
the first several seconds. The user cannot tell "thinking" from "stuck", so
they press Ctrl-C on a question that was about to be answered, or ask again.
Every agent `ama` supports is slow at this. The silence belongs to the
product, not to any one agent.

## Target users

Everyone who types `@@` at an interactive prompt. Scripts, pipes and
`$(…)` substitutions have no one watching, so they are explicitly *not*
targets (see constraints).

## Proposed outcome

From Return until the answer starts, the line where the answer will appear
shows `🤖` followed by an animation through the eight moon phases
🌑🌒🌓🌔🌕🌖🌗🌘. When the answer arrives, the answer takes over that line,
and the finished screen looks exactly as it does in 0.1.1.

## Constraints

- **Patch version, by owner direction.** Strictly, a backwards-compatible
  feature is a SemVer *minor* change. Before 1.0 the owner may number as
  they choose, and they chose 0.1.2. The cycle follows SDLC §14 like any
  patch cycle.
- **Nothing changes for a non-terminal.** Anything that reads `ama`'s
  output as data must get byte-for-byte what 0.1.1 produced.
- **The screen is the conversation.** `ama` sends the visible pane to the
  agent as context (ADR-002). An animation that leaves residue, or lands in
  its own capture, corrupts the next turn's prompt. So this is a
  correctness constraint, not a matter of looks.
- **BYOA.** The feature cannot assume Claude. Agents that print progress
  on stderr must not end up with a garbled display.
- **The Enter hook is not touched.** NFR-01 (50 ms, measured at 7.0 ms)
  stays as it is.

## Non-goals

- Progress reporting (percentages, what the agent is doing, elapsed time).
- Any change to how the answer itself is rendered.
- A spinner for `ama doctor`, `ama setup` or any other subcommand.
- Handling Ctrl-C with a signal handler (see RISK-15).

## Primary success measure

In a real terminal, the owner's verbatim `@@ whats weather?` shows `🤖` and
a moon that visibly changes phase until the answer replaces it. Afterwards,
not a single moon glyph is left on screen or in the next turn's context.

## Known risks at intake

- The animation writes to the same screen that the next turn scrapes for
  context. Residue there becomes part of a prompt.
- Agents that write to stderr while thinking (codex does, heavily) share
  that screen with the animation.

## Open questions at intake

| ID | Question | Resolution |
|---|---|---|
| Q-03 | Should the moon be user-disableable? | Resolved in S2 by ADR-013: **yes**, `spinner: false`. Recorded as an implementer decision the owner must confirm at SG5 (README, owner-review list) |
| Q-04 | May `ama` take over the agent's stderr to keep it off the moon's line? | Resolved in S2 by ADR-012: **yes**, and only while the moon is showing on a terminal. Same confirmation needed |

## SG0 — Intent accepted

Accepted 2026-09-26 by the owner, who stated the feature and directed that
it ship as 0.1.2 and that the cycle run without pausing for approval.
