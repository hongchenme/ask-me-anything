---
software_version: 0.1.1
process_version: 1.0.0
stage: S4-build
status: complete
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [05-implementation-plan.md]
updated: 2026-09-26
---

# Build record — 0.1.1

Branch `cycle/0.1.1-fixes`. 11 files, +632 / −56.

| File | Change |
|---|---|
| `crates/ama/src/adapter.rs` | `Tools` enum; `normalize` takes it and grants search (ADR-007) |
| `crates/ama/src/config.rs` | `agent.tools`, validated at load (REQ-32) |
| `crates/ama/src/prompt.rs` | second preamble paragraph (ADR-008) |
| `crates/ama/src/shell/ama.bash` | `\C-j` chained via a private accept-line; hook made idempotent (ADR-009) |
| `crates/ama/src/cli.rs` | starter config explains the grant |
| `crates/ama/Cargo.toml`, `Cargo.lock` | 0.1.0 → 0.1.1 |
| `README.md` | `tools:`, updated adapter table, "Web search" section |
| `tests/unit_config.rs` | +12 tests |
| `tests/unit_context.rs` | +2 tests |
| `tests/e2e_shell.rs` | +5 tests |

## Slices

### TASK-1 / TASK-2 — the grant and its config key

Written test-first; the suite failed to compile against the old
single-argument `normalize`, which is the intended red state for a
signature change. Every pre-existing adapter assertion was re-pointed at
`Tools::None` — those tests pin 0.1.0 argv, and 0.1.0 argv is now exactly
what `tools: none` produces — and a `Tools::Research` counterpart was added
beside each.

Three pre-existing config tests assert the *default*, and the default
changed. They were updated rather than pinned, because the behaviour change
is the fix:

| Test | Was | Now |
|---|---|---|
| `a_bare_string_line_still_gains_the_one_shot_flag` | `claude -p …` | `claude -p --allowedTools WebSearch …` |
| `a_structured_command_gets_adapters_too` | `claude -p --model opus` | `claude -p --allowedTools WebSearch --model opus` |
| `a_placeholder_index_survives_an_inserted_one_shot_token` | `Arg(3)` | `Arg(5)` — the grant is two more tokens |

That last one earned its keep: resolving `{prompt}` *after* normalisation
was a 0.1.0 decision (C2), and this cycle inserts two more tokens in front
of the placeholder. Had the index been computed first, the question would
have been substituted into the wrong slot.

**Deviation D-01.** The plan said `tools:` would be deserialised straight
into `Tools`. It is held as a `String` and parsed by `parse_tools` instead.
`AgentConfig` is `#[serde(untagged)]`, and an untagged enum reports any
failed variant as *"data did not match any variant of untagged enum
AgentConfig"* — which names neither the bad value nor the alternatives, and
REQ-32 requires both. Validating at load keeps the error early and specific.
No requirement changed; recorded under SDLC §14.

### TASK-3 — both Return keys

Built in two deliberate steps so the hazard was demonstrated rather than
asserted.

**Step 1, bindings only.** `__ama_install` gained `\C-x\C-am` (a private
accept-line), a loop over `\C-m` and `\C-j`, and `__ama_bound_macro` to read
an existing macro. `__ama_bound_macro` replaces a `sed` one-liner: the key
name is now a parameter, and passing `\C-m` through a BRE needs it
double-escaped — a silent-mismatch trap in a function whose failure mode is
"quietly stops chaining". Bash pattern matching has no such escaping layer.

`a_third_party_enter_macro_does_not_double_quote_a_triggered_line` then
failed, exactly as RISK-09 predicted, with the corruption captured verbatim
from the pane:

```text
V$ @@ ''\''what'\''\'\'''\''s it'\'''
```

**Step 2, the guard.** `__ama_hook` records the buffer it writes and returns
untouched when handed the same string again. The test passes, and the
mutation evidence above is what its doc comment cites.

### TASK-4 — preamble, version, docs

**Deviation D-02.** The plan named a new `tests/unit_prompt.rs`.
`prompt::compose` is already tested in `tests/unit_context.rs`, so the two
new tests went there rather than splitting one function's coverage across
two files. No requirement changed.

The preamble text is the exact string measured in S1 — not a paraphrase of
it — so the eval in [07-verification.md](07-verification.md) and the shipped
constant are the same words.

## Evidence

`./check.sh` — all green. `shellcheck` was not installed on the build
machine and `check.sh` had been skipping it; it was installed (0.10.0) and
the gate now runs for real.

```text
  rustfmt ok · clippy ok · clippy (tests) ok · bash syntax ok
  install.sh syntax ok · zsh syntax ok · shellcheck ok · build ok · tests ok
  All checks passed.
```

**162 tests pass** (`cargo test -- --test-threads=1`), up from 143 at 0.1.0:

| Suite | Tests |
|---|---|
| `e2e_shell` | 24 |
| `integration_cli` | 33 |
| `unit_config` | 37 |
| `unit_context` | 21 |
| `shell_functions` | 20 |
| `integration_agent` | 9 |
| lib · `unit_render` · `unit_session` | 8 · 5 · 5 |

The e2e suite drives real interactive shells in real tmux panes with
fixed session names, so it requires `--test-threads=1` — which is what
`check.sh` runs and what CI runs. A bare parallel `cargo test` fails two
e2e tests on session-name contention; that predates this cycle and is not
a product defect.

## SG4 — Build complete

Passed. Deviations D-01 and D-02 recorded above; neither changes a
requirement. Independent review and reproduction of the evidence belong to
S5.
