---
software_version: 0.1.1
process_version: 1.0.0
stage: S2-specification
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [01-intent.md, 02-discovery-and-risk.md]
updated: 2026-09-26
---

# Product requirements — 0.1.1

Numbering continues 0.1.0 (which ended at REQ-30, NFR-07). Requirements from
0.1.0 remain in force; this document adds to them and amends none.

## Functional requirements

### REQ-31 — a known agent is authorized to search the web

`ama` inserts the web-search authorization a known one-shot agent needs, in
the same way and under the same rules ADR-003 already established for the
one-shot token: **insert only when absent, never remove, reorder or rewrite a
user's arguments, and touch nothing for an agent not in the table.**

| Agent | Inserted | Position | Suppressed when |
|---|---|---|---|
| `claude` | `--allowedTools WebSearch` | after the program | any `--allowedTools` / `--allowed-tools` already present |
| `codex` | `--search` | before `exec` — `codex exec --search` is rejected (AS-02) | `--search` already present |
| others | nothing | — | always |

**Acceptance**

- `agent: {command: [claude]}` resolves to
  `claude -p --allowedTools WebSearch`.
- `agent: {command: [claude, --allowedTools, "WebSearch WebFetch"]}` is left
  exactly as written — the user's own grant wins, wider or narrower.
- `agent: {command: [codex]}` resolves to `codex --search exec`.
- `agent: {command: [my-bot]}` resolves to `my-bot`.
- `adapter: false` inserts nothing, as in 0.1.0.

### REQ-32 — the grant is configurable

`agent.tools` accepts:

| Value | Meaning |
|---|---|
| `research` | the REQ-31 grant. **Default** when the key is absent |
| `none` | insert no authorization; the one-shot token is still inserted |

**Acceptance**

- Omitting `tools:` behaves as `research`, so 0.1.0 configs keep working.
- `tools: none` with `command: [claude]` resolves to `claude -p` — the 0.1.0
  argv exactly.
- An unrecognised value is a configuration error naming the key and the
  accepted values, exiting 2. It is not silently treated as `none`.
- `tools:` is accepted only on the structured form, alongside `adapter:`.

### REQ-33 — Return triggers the hook whatever byte the terminal sends

In bash, `@@ <question>` followed by Return is rewritten and run whether the
terminal sends CR (`\C-m`) or LF (`\C-j`).

**Acceptance**

- Reporter's verbatim line `@@ what's the files listed under this dir`,
  submitted with LF, produces a `🤖:` answer and leaves the shell at a normal
  prompt — never at `PS2`.
- The same line submitted with CR behaves exactly as in 0.1.0.
- zsh answers under both, unchanged and unmodified (F-10).
- REQ-03 holds for both keys: an untriggered line behaves exactly as it would
  in an uninstrumented shell.
- RISK-03 holds for both keys: a pre-existing macro on either key is chained,
  not clobbered.

### REQ-34 — the agent answers the question it was asked

The prompt preamble states that the question is general, that the agent may
use its tools to answer, and that it must not claim an absent capability
without having tried.

**Acceptance**

- A question needing the live web returns either a real answer or a request
  for the single missing detail.
- Across repeated runs of the reporter's verbatim question, the agent never
  claims it lacks internet access or tools. This is an eval, not a single
  example: see [07-verification.md](07-verification.md).
- Questions about the current directory still answer from read-only tools, as
  in 0.1.0 (no regression from the grant — AS-01).

## Non-functional requirements

### NFR-08 — the Enter path stays imperceptible on both keys

NFR-01's budget (50 ms, measured at 7.0 ms in 0.1.0) applies unchanged to
`\C-j`. The fix adds no subprocess to the hook: it is two extra `bind` calls
at install time and one string comparison per keypress.

### NFR-09 — the hook is idempotent within a keypress

Invoking `__ama_hook` twice on one keypress must produce the same buffer as
invoking it once (RISK-09 / F-11).

## Traceability

| Defect | Cause | Requirement | Decision | Test |
|---|---|---|---|---|
| Weather | F-07 tool denied | REQ-31, REQ-32 | ADR-007 | `unit_config.rs`, `integration_cli.rs` |
| Weather | F-08 agent reluctant | REQ-34 | ADR-008 | `unit_prompt.rs`, eval in 07 |
| Directory | F-09 only `\C-m` bound | REQ-33 | ADR-009 | `e2e_shell.rs` (LF) |
| — | F-11 double invocation | NFR-09 | ADR-009 | `e2e_shell.rs` (chain + trigger) |

## SG2 — Specification accepted

Accepted 2026-09-26. Q-01 and Q-02 were closed by the owner before this
document was written; no blocking decision remains open.
