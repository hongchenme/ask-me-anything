---
software_version: 0.1.1
process_version: 1.0.0
stage: S0-intent
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [../AGENT_NATIVE_SDLC.md, ../0.1.0/README.md]
updated: 2026-09-26
---

# INT-0.1.1 — two defects make `@@` look broken on first contact

## Problem

The owner reported two failures from ordinary use of a freshly installed 0.1.0:

> Currently when running `@@ what's the weather`, it returns
> `🤖: I don't have live internet access`; or when running
> `@@ what's the files listed under this dir`, it starts a new edit line
> with zero output conversation.

Both are first-contact failures. The weather question is close to the README's
own pitch — "ask your own agent a question without leaving the shell" — and the
second leaves the shell sitting at a continuation prompt with no answer, no
error, and nothing to search for.

## Why it matters

0.1.0's whole product claim is that the terminal becomes a question box. A user
tries it, the first two things they ask both fail, and neither failure explains
itself:

- The weather answer is *confidently wrong about its own capabilities*. The
  agent can search the web; it says it cannot. A user has no way to tell that
  from a real limitation, so they conclude `ama` is a thin and useless wrapper.
- The directory question produces no output at all. There is no error message,
  no exit code the user sees, and no `🤖:` line. The shell just presents a new
  line to type on.

Neither is a corner case. Both were hit within minutes of installing.

## Target users

Everyone using 0.1.0, with one important split discovered during S1: defect 2
depends on the terminal emulator, so it hits some users every single time and
others never. The owner observed it under iTerm2 and not under macOS Terminal.

## Proposed outcome

1. A question that needs the live web gets a real answer, or an honest request
   for the one missing detail. It never claims a capability is absent when it
   is present.
2. Pressing Return on `@@ <question>` runs the question, on every terminal that
   bash itself supports — not only those that send CR.

## Constraints

- **Patch release.** 0.1.1 fixes defects. No new product surface beyond what
  the fixes require (SDLC §14: emergency/defect work creates a patch cycle).
- **BYOA is not negotiable.** The fix cannot assume Claude. Whatever `ama`
  does for a known agent must degrade to "touch nothing" for an unknown one,
  exactly as ADR-003's adapter table already does.
- **RISK-01 stays accepted, not widened.** Terminal content reaches the agent
  unfiltered. Granting the agent new authority interacts directly with that,
  so any grant is an owner decision, not an implementer default.
- **The 0.1.0 contract holds.** `adapter: false` still means "touch nothing";
  a config that works today still works.

## Non-goals

- Redaction or filtering of captured pane content (deferred from 0.1.0,
  still deferred).
- Windows support.
- Any change to the answer-rendering, session, or context-selection layers.
- Fixing the prompt-injection exposure that RISK-01 describes. This cycle must
  not *widen* it, which is a different and weaker requirement.

## Primary success measure

Both reported commands, run verbatim on a fresh install in the reporter's own
terminal, produce a useful `🤖:` answer.

## Known risks at intake

- Making the agent more willing to use tools is a behavioural change driven by
  prompt text and by permission flags. It needs evidence from repeated runs,
  not a single passing example (SDLC §12: agent configuration is production
  behaviour and needs a versioned eval).
- Rebinding another key to the Enter hook touches the single most
  safety-critical line of the product. REQ-03 — "an untriggered line behaves
  exactly as it would in an uninstrumented shell" — is non-negotiable.

## Open questions at intake

| ID | Question | Resolution |
|---|---|---|
| Q-01 | What tool authority should `ama` grant a one-shot agent by default, given RISK-01? | **Closed at SG1 by the owner: WebSearch only.** See [02-discovery-and-risk.md](02-discovery-and-risk.md#5-owner-decisions). |
| Q-02 | Should that grant be configurable? | **Closed at SG1 by the owner: yes, an `agent.tools` key.** |

## SG0 — Intent accepted

Accepted 2026-09-26 by the owner, who reported both defects and directed that
they be fixed in a 0.1.1 cycle following this SDLC.
