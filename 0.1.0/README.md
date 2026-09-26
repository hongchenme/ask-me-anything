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
| **Stage** | S5 — verify and evaluate (re-entered after the A-05…A-09 refactor) |
| **Gate** | SG4 passed. SG5 pending owner review of the verification record |
| **Risk tier** | R1 (see [risk register](02-discovery-and-risk.md#4-risk-register)) |
| **Next action** | Re-verify after amendments A-05…A-09, then owner decides on release |
| **Blockers** | None |
| **Evidence** | `./check.sh` green · 143 tests (110 at S4, +14 from the whole-branch review's fix wave, +19 from the A-05…A-09 refactor and its review) · NFR-01 measured at 7.0 ms median vs a 50 ms budget |

## Artifact map

| Stage | Artifact | Status |
|---|---|---|
| S0 | [01-intent.md](01-intent.md) | accepted |
| S1 | [02-discovery-and-risk.md](02-discovery-and-risk.md) | accepted |
| S2 | [03-product-requirements.md](03-product-requirements.md) | accepted (amended A-01, A-04) |
| S2 | [04-design.md](04-design.md) | accepted (amended A-02) |
| S3 | [05-implementation-plan.md](05-implementation-plan.md) | accepted |
| S4 | [06-build-record.md](06-build-record.md) | complete |
| S5 | [07-verification.md](07-verification.md) | in review |
| S6 | `08-release.md` | not started |
| S7 | `09-operations.md` | not started |

## Gate log

| Gate | Date | Decision | Note |
|---|---|---|---|
| SG0 — Intent accepted | 2026-09-26 | Accepted | Problem worth discovery |
| SG1 — Discovery accepted | 2026-09-26 | Accepted | R1 tier and RISK-01 acceptance approved |
| SG2 — Specification accepted | 2026-09-26 | Accepted | Amended twice during S3, below |
| SG3 — Plan approved | 2026-09-26 | Accepted | Owner chose subagent-driven execution |
| SG4 — Build complete | 2026-09-26 | Passed | Whole-branch review returned 0 Critical, 5 Important; all fixed and re-verified in one fix wave |
| SG5 — Verification accepted | — | Pending | Owner reviews the verification record |

## Amendments

Recorded under SDLC §14. A-01 through A-03 were found while planning against the real
spec; A-04 was found by the whole-branch review. All are returns to S2, not silent
scope changes.

| ID | Date | Change | Impact |
|---|---|---|---|
| A-01 | 2026-09-26 | **REQ-28 added.** README example 3 is `clear && @@ show me the joke of the day`, so the trigger is not always at the start of the line — an assumption every capture requirement had made. ADR-006 defines the recognition order. | One extra branch in the shell hook and a unit-tested split function. No architecture, tier, or other requirement affected. |
| A-03 | 2026-09-26 | **ADR-006 rule 2 inverted: the earliest trigger wins.** Task 1's review showed the original "last operator" rule was incoherent — `@@ compare a && @@ b` was one prompt, but a leading `clear && ` made the same text split at the *second* trigger, pushing `@@ compare a &&` back into the user's buffer as executable text. Rule 1 exists so the earliest trigger wins, so the old rule 2 contradicted its own decision. | One branch reimplemented with parameter expansion instead of a regex, plus a fixture. Found by review, not shipped. |
| A-04 | 2026-09-26 | **REQ-11's statement amended: context is the whole visible pane when the current question is the only trigger on it.** The statement said "from the first trigger line to the bottom", but the question is already echoed on the pane when `ama` captures it, so its own acceptance criterion (`ls /nonexistent`, then `@@ …`) needed content from *above* that line and failed verbatim. Found by the whole-branch review; the behaviour was fixed rather than the requirement re-scoped, since ADR-002 justifies the tmux path as exactly this case. | One branch in `context::select_context`, four unit fixtures, and an e2e test running the criterion verbatim. No architecture, tier, or other requirement affected. |
| A-05 | 2026-09-26 | **Project renamed `question-mark-x2` → `ask-me-anything`.** The repository was still named after the original `??` trigger, which ADR-005 had already replaced with `@@` because `??` is a bash glob. | References updated; the binary was already `ama`. The GitHub repository and the local directory are renamed by the owner. |
| A-06 | 2026-09-26 | **Adapters apply to the structured `command:` form, and insert a token rather than a flag.** Dropping the bare-string form from the documented config made the old bypass a trap: `command: [claude]` would have hung. `codex`'s one-shot mode is a subcommand, and `codex exec -p` means `--profile`. | `adapter: false` becomes the explicit escape hatch; `codex` and `agy` join the table; the `{prompt}` index is resolved after normalisation. Amends ADR-003 and REQ-18. |
| A-07 | 2026-09-26 | **`ama setup` added (REQ-29); `install.sh` delegates to it.** rustc rejects a crate named `@@`, so cargo-dist cannot ship the alias as a second `[[bin]]` — it has to be created after the binary lands, and after a `curl` install there is no repository to run a script from. | One subcommand now owns the alias, the starter config, and the rc wiring, so the clone path and the curl path cannot drift. |
| A-08 | 2026-09-26 | **Config directory `~/.qmx2/` → `~/.ama/`.** ADR-005 had kept the old name on the grounds that renaming would break the README; with A-05 done and nothing released, there is no installed base to break. | Amends ADR-005 and REQ-16. |
| A-09 | 2026-09-26 | **cargo-dist packaging (REQ-30).** Installation required a clone, a toolchain, and a release build. | `dist-workspace.toml` plus a generated release workflow produce `ama-installer.sh` for macOS and Linux on tag. Windows is excluded: the headline feature is a bash/zsh Enter hook, so a Windows build could offer `ama ask` but never `@@`. |
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
says `ama` throughout, and the rewrite the shell integration performs is documented
there. Examples 1–3 still show the line as **typed** — which is what the user sees
before pressing Enter — with the quoted line that actually executes explained in a note
beneath them. That resolution is fine; the earlier claim here that "its examples show
the quoted line that actually executes" was not, and is corrected (R35).

A third correction was found during that task — the README documented
`@@ --no-context`, which did not work, because the trigger path joined all arguments
into the question. The whole-branch review ruled the other way (R24): the flag is one
of only two mitigations `04-design.md` names for the owner-accepted RISK-01, so the
trigger path now honours it, and the README documents both `@@ --no-context …` and
`ama ask --no-context -- …` again.
