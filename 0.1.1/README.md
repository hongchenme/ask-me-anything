---
software_version: 0.1.1
process_version: 1.0.0
stage: S5-verification
status: in-review
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [../AGENT_NATIVE_SDLC.md, ../0.1.0/README.md]
updated: 2026-09-26
---

# Cycle 0.1.1 — two first-contact defects

A patch cycle (SDLC §14). Two defects reported from ordinary use of 0.1.0,
both hit within minutes of installing:

> `@@ what's the weather` returns `🤖: I don't have live internet access`; or
> `@@ what's the files listed under this dir` starts a new edit line with
> zero output conversation.

## Current state

| | |
|---|---|
| **Stage** | S5 — verify and evaluate |
| **Gate** | SG4 passed. SG5 pending owner review |
| **Risk tier** | R2 — raised from 0.1.0's R1: this cycle changes the authority handed to a subprocess and binds a second key to Enter |
| **Next action** | Owner reviews [07-verification.md](07-verification.md) and decides go/no-go |
| **Blockers** | None |
| **Evidence** | `./check.sh` green · 162 tests (143 at 0.1.0, +19) · both defects reproduced fixed against a live agent in a real terminal · REQ-34 eval 14/14 |

## The two defects

| | Defect 1 — weather | Defect 2 — directory |
|---|---|---|
| **Symptom** | Confidently denies a capability it has | No output at all; shell stuck at PS2 |
| **Root cause** | **Two, both required.** A one-shot agent cannot answer a permission prompt, so WebSearch is auto-denied (F-07) — *and* even once granted, the agent's coding-assistant persona does not reach for it (F-08) | The bash hook binds only `\C-m`. Terminals that send LF for Return bypass it entirely, so the raw line reaches bash and the apostrophe in `what's` opens a quote that never closes (F-09) |
| **Decisive evidence** | `claude -p`: *"I don't have permission to use WebSearch right now"* | `~/.bash_history:500` stores the line **raw**; every other `@@` entry is stored rewritten |
| **Why it looked flaky** | 1 of 6 recorded turns did search successfully — the agent's choice, not the flag's | Terminal-dependent: iTerm2 hit it, macOS Terminal never did |
| **Fix** | ADR-007 grant + ADR-008 preamble | ADR-009 private accept-line on both keys |

Neither defect was visible to the 0.1.0 suite: every e2e test submits with
`Enter`, which tmux sends as CR, and no test could see an agent decline to
use a tool.

## Artifact map

| Stage | Artifact | Status |
|---|---|---|
| S0 | [01-intent.md](01-intent.md) | accepted |
| S1 | [02-discovery-and-risk.md](02-discovery-and-risk.md) | accepted |
| S2 | [03-product-requirements.md](03-product-requirements.md) | accepted |
| S2 | [04-design.md](04-design.md) | accepted |
| S3 | [05-implementation-plan.md](05-implementation-plan.md) | accepted |
| S4 | [06-build-record.md](06-build-record.md) | complete |
| S5 | [07-verification.md](07-verification.md) | in review |
| S6 | [08-release.md](08-release.md) | prepared, not authorized |
| S7 | `09-operations.md` | not started |

## Gate log

| Gate | Date | Decision | Note |
|---|---|---|---|
| SG0 — Intent accepted | 2026-09-26 | Accepted | Owner reported both defects and directed a 0.1.1 cycle |
| SG1 — Discovery accepted | 2026-09-26 | Accepted | Root causes evidenced; Q-01/Q-02 closed by owner; R2 tier set |
| SG2 — Specification accepted | 2026-09-26 | Accepted | REQ-31…34, NFR-08/09, ADR-007…009 |
| SG3 — Plan approved | 2026-09-26 | Accepted | Four slices, tests first |
| SG4 — Build complete | 2026-09-26 | Passed | `./check.sh` green, 162 tests, 2 deviations recorded |
| SG5 — Verification accepted | — | **Pending** | Implementer cannot self-approve (SDLC §11) |
| SG6 — Release authorized | — | Not reached | |

## Owner decisions

Taken at SG1, before any code was written, because they change security
posture rather than implementation detail.

| ID | Question | Decision |
|---|---|---|
| Q-01 | What tool authority should `ama` grant by default, given that unfiltered terminal content is in every prompt (RISK-01)? | **WebSearch only.** WebFetch withheld: a search goes to a search engine, a fetch goes to a URL the prompt can choose, which is the exfiltration primitive this repository's own injection fixture targets |
| Q-02 | Configurable? | **Yes** — `agent.tools: research \| none`, default `research` |

## Decisions added this cycle

| ID | Decision |
|---|---|
| ADR-007 | The adapter grants search authority, under the rule it already follows: insert only what is missing, never rewrite the user's argv |
| ADR-008 | The preamble tells the agent it is a general question box and must not claim an absent capability without trying |
| ADR-009 | A private `accept-line` (`\C-x\C-am`) lets both `\C-m` and `\C-j` chain to the hook without the macro recursing |

## Risks added this cycle

| ID | Risk | Disposition |
|---|---|---|
| RISK-09 | Binding `\C-j` makes a third-party `\C-m` macro ending in `\C-j` run the hook twice, corrupting a triggered line | **Mitigated**, and demonstrated first: the corruption was reproduced on a real pane before the guard was written |
| RISK-10 | A granted search can carry prompt-injected text to a search provider | **Accepted, narrowed** — no attacker-chosen destination |
| RISK-11 | Inserting a flag an agent version rejects | **Mitigated** — only flags verified against the installed version |
| RISK-12 | Longer preamble costs tokens each turn | Accepted (~60 words) |

## Carried forward to the next cycle

| ID | Finding |
|---|---|
| F-12 | The agent occasionally copies the `🤖:` marker into its own answer, producing `🤖: 🤖: …`. Seen once in live verification, 0/3 on a controlled re-run. Cosmetic, mechanism unchanged from 0.1.0, deliberately not fixed here — see [07-verification.md §5](07-verification.md#5-residual-risk-and-one-new-finding) |

## Notes for a reader who was not here

- **`shellcheck` was silently skipping.** `check.sh` degrades to a skip when
  the binary is missing, and it was missing on the build machine, so that
  gate had not actually run. It is installed (0.10.0) and green.
- **The e2e suite needs `--test-threads=1`.** Real tmux sessions with fixed
  names contend otherwise. `check.sh` and CI already do this; a bare
  `cargo test` fails two e2e tests for that reason alone.
- **zsh was never affected and was not changed.** It rebinds the
  `accept-line` *widget*, which both `^M` and `^J` point to. A regression
  test pins that, so a future "port the bash fix to zsh" fails loudly
  instead of quietly breaking it.
