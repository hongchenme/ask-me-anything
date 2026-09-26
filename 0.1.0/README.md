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
| **Stage** | S2 — specify requirements and design |
| **Gate** | SG2, awaiting owner review of the artifacts below |
| **Risk tier** | R1 (see [risk register](02-discovery-and-risk.md#4-risk-register)) |
| **Next action** | Owner reviews S0–S2; on acceptance, S3 produces `05-implementation-plan.md` |
| **Blockers** | None |

## Artifact map

| Stage | Artifact | Status |
|---|---|---|
| S0 | [01-intent.md](01-intent.md) | accepted |
| S1 | [02-discovery-and-risk.md](02-discovery-and-risk.md) | accepted |
| S2 | [03-product-requirements.md](03-product-requirements.md) | in review |
| S2 | [04-design.md](04-design.md) | in review |
| S3 | `05-implementation-plan.md` | not started |
| S4 | `06-build-record.md` | not started |
| S5 | `07-verification.md` | not started |
| S6 | `08-release.md` | not started |
| S7 | `09-operations.md` | not started |

## Gate log

| Gate | Date | Decision | Note |
|---|---|---|---|
| SG0 — Intent accepted | 2026-09-26 | Accepted | Problem worth discovery |
| SG1 — Discovery accepted | 2026-09-26 | Accepted | R1 tier and RISK-01 acceptance approved |
| SG2 — Specification accepted | — | Pending | Awaiting owner review |

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

## Known documentation debt

The root [README.md](../README.md) needs two corrections before release, both
consequences of accepted decisions rather than changes of scope:

1. Examples render the executed line unquoted (`@@ what's this project all about?`).
   Under ADR-001 it executes as `@@ 'what'\''s this project all about?'`.
2. The tool is named `aka` throughout; ADR-005 renames it `ama`.

Tracked as a task in S3 so the docs and the binary ship consistent.
