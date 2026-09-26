---
software_version: 0.1.2
process_version: 1.0.0
stage: S6-release
status: in-review
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [../AGENT_NATIVE_SDLC.md, ../0.1.1/README.md]
updated: 2026-09-26
---

# Cycle 0.1.2 — the moon while the agent thinks

A patch cycle carrying one owner-requested feature:

> when a user enters a prompt `@@ whats weather?`, ama will immediately print
> 🤖 followed by a animation that cycles through 8 unicode moon phases before
> the answer is returned by an underlying agent.

## Current state

| | |
|---|---|
| **Stage** | S6 — release and deploy |
| **Gate** | SG4 passed. SG5/SG6: the owner directed the release on 2026-09-26 |
| **Risk tier** | R2: every interactive turn prints something new, and the agent's stderr is routed while the moon shows |
| **Next action** | Execute [08-release.md](08-release.md)'s plan: PR, Linux CI, admin merge, tag `v0.1.2`. Owner confirms the three decisions in [07 §7](07-verification.md#7-release-recommendation) whenever convenient |
| **Blockers** | None |
| **Evidence** | `./check.sh` green · 193 tests (162 at 0.1.1, +31) · verbatim prompt verified live against claude (15 s think, 8 phases, 0 glyphs left) and codex (stderr intact) · NFR-10 p90 51.8 ms against 100 ms |
| **Committed** | Yes, on `cycle/0.1.2-moon-spinner`, as the release's first step |

## What changed

| | |
|---|---|
| **Feature** | `🤖 🌑🌒🌓🌔🌕🌖🌗🌘`, 100 ms a frame, on the line the answer will replace. It appears ~50 ms after `ama` starts, after the pane capture, so it is never part of its own context |
| **Where** | The binary (`spinner.rs`), not the shell hook, which is untouched |
| **When** | Only when stdout is a terminal and `TERM` is not `dumb`, and not with `spinner: false` |
| **Agent stderr** | While the moon shows on a terminal, stderr is routed through `ama`: every line lands intact on its own line, with the moon below it |
| **Context** | Frames the user's own keys leave on screen (Enter or Ctrl-C mid-think) are stripped from every capture |

## Artifact map

| Stage | Artifact | Status |
|---|---|---|
| S0 | [01-intent.md](01-intent.md) | accepted |
| S1 | [02-discovery-and-risk.md](02-discovery-and-risk.md) | accepted, amended after the SG4 review |
| S2 | [03-product-requirements.md](03-product-requirements.md) | accepted, amended after the SG4 review |
| S2 | [04-design.md](04-design.md) | accepted, amended after the SG4 review (ADR-014 added) |
| S3 | [05-implementation-plan.md](05-implementation-plan.md) | accepted |
| S4 | [06-build-record.md](06-build-record.md) | complete |
| S5 | [07-verification.md](07-verification.md) | complete; owner directed release |
| S6 | [08-release.md](08-release.md) | authorized; executing |
| S7 | `09-operations.md` | not started |

## Gate log

| Gate | Date | Decision | Note |
|---|---|---|---|
| SG0 — Intent accepted | 2026-09-26 | Accepted | Owner stated the feature and directed a 0.1.2 cycle |
| SG1 — Discovery accepted | 2026-09-26 | Recorded under the exception | F-13…F-16, RISK-13…17, R2 |
| SG2 — Specification accepted | 2026-09-26 | Recorded under the exception | REQ-35…39, NFR-10…12, ADR-010…013 |
| SG3 — Plan approved | 2026-09-26 | Recorded under the exception | Five slices, tests first |
| SG4 — Build complete | 2026-09-26 | Passed | Independent review "With fixes": 0 Critical, 1 Important, 10 Minor. Nine fixed or documented, one declined with reason. A re-review of the fixes found **no blocking issues** and confirmed all eight dispositions; its five Minor findings are fixed (07 §8) |
| SG5 — Verification accepted | 2026-09-26 | Owner directed release | The implementer did not self-approve (SDLC §11). The three implementer decisions stay open for confirmation (07 §7) |
| SG6 — Release authorized | 2026-09-26 | Authorized | The owner's directive (08). Executed as PR → Linux CI → admin merge → tag `v0.1.2`; CI can still stop it |

## Process exception

Recorded under SDLC §14.

| | |
|---|---|
| **Control** | Owner approval at SG1, SG2 and SG3 |
| **Scope** | This cycle only |
| **Reason** | The owner directed that the cycle run without pausing for approval |
| **Compensating controls** | Every decision is written down with the alternatives it beat. Each decision the owner did not make is listed for confirmation below. An independent review ran before SG4, and a second one re-checked its fixes. SG5 and SG6 still need the owner |
| **Expiry** | SG5/SG6: the owner's release directive (2026-09-26) closes it |

## Decisions for the owner to confirm

Taken by the implementer because the owner asked not to be interrupted.
Each one is reversible:

| ID | Decision | If rejected |
|---|---|---|
| Q-03 / ADR-013 | Add `spinner: false`. It was not asked for; the reasons are accessibility (WCAG 2.2.2 for motion lasting over 5 s), rollback, and RISK-13's escape hatch | Delete the key; the moon is then always on |
| Q-04 / ADR-012 | Route the agent's stderr through `ama` while the moon shows, so codex's progress lines are not garbled | Inherit stderr always; codex users get a moon glued to each progress line |
| ADR-014 | Strip stranded frames from captured context, and accept them staying on screen | The alternative is turning off terminal echo during the moon (`unsafe` or a new dependency; Ctrl-C hazard) |
| — | Number it 0.1.2 | A feature is a SemVer minor change, but this was the owner's own instruction |

## Decisions added this cycle

| ID | Decision |
|---|---|
| ADR-010 | The moon lives in the binary, not the shell hook |
| ADR-011 | Draw on stdout, after the pane capture, with CR+EL, only on a terminal; never hide the cursor |
| ADR-012 | While the moon shows on a terminal, route the agent's stderr through it, and wait for the tail sensibly |
| ADR-013 | `spinner: false` |
| ADR-014 | Every capture strips moon frames |

## Risks added this cycle

| ID | Risk | Disposition |
|---|---|---|
| RISK-13 | An agent behaves differently on a piped stderr | Mitigated; live-checked; `spinner: false` |
| RISK-14 | A grandchild holding the stderr pipe | Mitigated; idle 250 ms, cap 3 s |
| RISK-15 | Ctrl-C leaves a frame on screen | Screen residue accepted; context stripped |
| RISK-16 | A backgrounded `ama` animates over the prompt | Accepted |
| RISK-17 | The moon lands in context | Mitigated, two layers |
| RISK-18 | An Enter mid-think strands frames | Context stripped; screen residue accepted |
| RISK-19 | The first frame erases a partial line | Accepted; `@@` unaffected |
| RISK-20 | The agent's Ctrl-C messages are lost while its stderr is piped | Accepted |

## Carried forward

| ID | Finding |
|---|---|
| F-12 (0.1.1) | The agent sometimes copies `🤖:` into its own answer. Still open; this cycle was the owner's feature, not this finding |
| — | GNU screen is untested end to end. macOS's bundled screen 4.00.03 writes non-BMP glyphs as 0xFD in `hardcopy`, so neither the moon filter nor `🤖:` answer detection works there (07 §6) |
| — | `.gitignore` does not cover `.DS_Store`, which macOS drops at the repo root. First seen here because this is the first macOS cycle. Not changed: out of scope |
| — | `0.1.1/README.md` still shows SG5 pending and SG6 not reached, although 0.1.1 is merged to `main` and installed on this machine. That record could be brought up to date |

## Notes for a reader who was not here

- **This was built on macOS.** The first Linux run of the new tests will be
  CI's. They use only tmux, bash, zsh and POSIX tools, all present on the
  ubuntu runner, but no Linux run has been observed yet.
- **Three old e2e tests were de-flaked, not just re-run.** Each failure was
  reproduced under load on 0.1.1 as well as 0.1.2 before its fixed sleep was
  replaced by a wait for the event itself (06, D-04/D-06/D-07).
- **The verification transcript was deleted** with `ama session reset`. The
  owner's other `~/.ama/sessions` files were left as they were.
