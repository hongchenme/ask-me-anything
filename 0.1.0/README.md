---
software_version: 0.1.0
process_version: 1.0.0
stage: S2-specification
status: in-review
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [../README.md, ../AGENT_NATIVE_SDLC.md]
updated: 2026-09-26
---

# Cycle 0.1.0 — `ama`, ask me anything from the shell

Ship `@@ <question>` as a shell prefix that hands the question, plus the visible
terminal, to the user's own agent CLI and streams the answer back inline.

## Current state

| | |
|---|---|
| **Stage** | S4 — build and integrate (complete) |
| **Gate** | SG4, pending the independent whole-branch review |
| **Risk tier** | R1 (see [risk register](02-discovery-and-risk.md#4-risk-register)) |
| **Next action** | Whole-branch review, then S5 verification |
| **Blockers** | None |
| **Evidence** | `./check.sh` green · 110 tests · NFR-01 measured at 7.0 ms median vs a 50 ms budget |

## Artifact map

| Stage | Artifact | Status |
|---|---|---|
| S0 | [01-intent.md](01-intent.md) | accepted |
| S1 | [02-discovery-and-risk.md](02-discovery-and-risk.md) | accepted |
| S2 | [03-product-requirements.md](03-product-requirements.md) | accepted (amended A-01) |
| S2 | [04-design.md](04-design.md) | accepted (amended A-02) |
| S3 | [05-implementation-plan.md](05-implementation-plan.md) | accepted |
| S4 | [06-build-record.md](06-build-record.md) | complete |
| S5 | `07-verification.md` | not started |
| S6 | `08-release.md` | not started |
| S7 | `09-operations.md` | not started |

## Gate log

| Gate | Date | Decision | Note |
|---|---|---|---|
| SG0 — Intent accepted | 2026-09-26 | Accepted | Problem worth discovery |
| SG1 — Discovery accepted | 2026-09-26 | Accepted | R1 tier and RISK-01 acceptance approved |
| SG2 — Specification accepted | 2026-09-26 | Accepted | Amended twice during S3, below |
| SG3 — Plan approved | 2026-09-26 | Accepted | Owner chose subagent-driven execution |
| SG4 — Build complete | — | Pending | All 10 tasks implemented and task-reviewed; awaiting the independent whole-branch review |

## Amendments

Recorded under SDLC §14. Both were found while planning against the real spec and are
returns to S2, not silent scope changes.

| ID | Date | Change | Impact |
|---|---|---|---|
| A-01 | 2026-09-26 | **REQ-28 added.** README example 3 is `clear && @@ show me the joke of the day`, so the trigger is not always at the start of the line — an assumption every capture requirement had made. ADR-006 defines the recognition order. | One extra branch in the shell hook and a unit-tested split function. No architecture, tier, or other requirement affected. |
| A-03 | 2026-09-26 | **ADR-006 rule 2 inverted: the earliest trigger wins.** Task 1's review showed the original "last operator" rule was incoherent — `@@ compare a && @@ b` was one prompt, but a leading `clear && ` made the same text split at the *second* trigger, pushing `@@ compare a &&` back into the user's buffer as executable text. Rule 1 exists so the earliest trigger wins, so the old rule 2 contradicted its own decision. | One branch reimplemented with parameter expansion instead of a regex, plus a fixture. Found by review, not shipped. |
| A-02 | 2026-09-26 | **Rust `escape` module removed.** Nothing in Rust ever escapes: `@@` receives arguments the shell already parsed. Splitting and quoting must happen in the hook, since routing every Enter press through a subprocess would violate NFR-01. Both now live in the shell scripts, property-tested from Rust by driving the real shell. | Strictly better evidence — the oracle is now the shell that actually runs the code, not a Rust reimplementation of it. |

## Decisions carried into this cycle

| ID | Decision | Owner call |
|---|---|---|
| ADR-001 | Readline Enter hook rewrites the buffer; the agent does not run inside the hook | Owner chose shell keybinding over a plain PATH binary |
| ADR-002 | Hybrid context: tmux `capture-pane`, else own transcript | Owner chose hybrid over scrape-only or transcript-only |
| ADR-003 | Generic stdin pipe plus a one-shot-flag adapter table | Owner chose adapters over a pure pipe or structured-only config |
| ADR-004 | One binary, installed as `ama` and `@@`, dispatched on `argv[0]` | — |
| ADR-005 | Canonical name `ama`; trigger stays `@@`; `~/.qmx2/` retained | Owner renamed `aka` → `ama` |
| RISK-01 | Terminal content reaches the agent unfiltered | Owner cut redaction from 0.1.0 and accepted the risk |

## Evidence already on file

Feasibility was established by execution, not argument, before SG1. Probes ran against
real `bash -i` inside real `tmux` on the target machine:

- A PATH binary named `@@` **cannot** serve the README's own first example — bash dies
  on the apostrophe before the binary is looked up (F-01).
- A readline Enter hook captures `what's this project all about?` and
  `list *.rs files | grep foo && echo $HOME` completely unexpanded (F-02).
- Running the agent inside that hook erases the user's question from the screen and
  from history; rewriting the buffer instead preserves both and keeps Ctrl-C working
  (F-03).
- `tmux capture-pane -p` returns the visible screen including other commands' output,
  and returns nothing after `clear` (F-04).

## Documentation debt — cleared

Both corrections landed in Task 10 (`1e01087`): the root [README.md](../README.md) now
says `ama` throughout, and its examples show the quoted line that actually executes.
A third correction was found during that task and fixed at the same time — the README
had documented `@@ --no-context`, which does not work, because the trigger path joins
all arguments into the question. It now documents `ama ask --no-context -- <question>`.
