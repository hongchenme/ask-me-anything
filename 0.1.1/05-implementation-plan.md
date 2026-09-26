---
software_version: 0.1.1
process_version: 1.0.0
stage: S3-plan
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [04-design.md]
updated: 2026-09-26
---

# Implementation plan — 0.1.1

Four slices. TASK-1 and TASK-3 are independent (different files, no shared
state) and were the natural parallel pair; TASK-2 depends on TASK-1's
signature; TASK-4 depends on all three. Executed in order 1 → 2 → 3 → 4 by a
single implementer, since the whole change is ~150 lines and the coordination
cost of splitting it exceeds the benefit.

Every slice writes its failing test first (`superpowers:test-driven-
development`), then the smallest change that passes it.

## TASK-1 — grant search authority (REQ-31, ADR-007)

| | |
|---|---|
| **Files** | `crates/ama/src/adapter.rs` |
| **Forbidden** | changing insertion of the one-shot token; touching argv the user wrote |
| **Interfaces produced** | `adapter::Tools`, `normalize(&[String], Tools)` |
| **Risk** | R1 |
| **Rollback** | `tools: none` in config restores 0.1.0 argv exactly |

**Tests first** (`tests/unit_config.rs`, adapter section):

- `claude` gains `--allowedTools WebSearch`; keeps `-p` insertion.
- `claude` with a user `--allowedTools` (either spelling) is untouched.
- `codex` becomes `codex --search exec` — flag *before* the subcommand.
- `codex --search` already present is not doubled.
- `ollama`, `agy`, unknown agents: unchanged.
- `Tools::None` reproduces every 0.1.0 expectation.

**Completion:** every pre-existing adapter test still passes with
`Tools::None`, and its `Tools::Research` counterpart asserts the new argv.

## TASK-2 — make the grant configurable (REQ-32)

| | |
|---|---|
| **Files** | `crates/ama/src/config.rs` |
| **Depends on** | TASK-1 |
| **Forbidden** | changing `adapter:` semantics; accepting `tools:` on the bare-string form |
| **Risk** | R1 |
| **Rollback** | omit the key |

**Tests first** (`tests/unit_config.rs`, config section):

- Absent `tools:` behaves as `research` — 0.1.0 configs keep working.
- `tools: none` + `[claude]` resolves to exactly `claude -p`.
- `tools: bogus` is a `ConfigError` naming the key and the accepted values,
  exit 2 (`integration_cli.rs`).
- `adapter: false` still inserts nothing regardless of `tools:`.
- The `{prompt}` placeholder index is still resolved *after* normalisation,
  now that normalisation can insert two more tokens.

## TASK-3 — bind both Return keys (REQ-33, NFR-09, ADR-009)

| | |
|---|---|
| **Files** | `crates/ama/src/shell/ama.bash` |
| **Forbidden** | touching `ama.zsh`; changing `__ama_split`; changing Ctrl-L |
| **Risk** | R2 — this is the Enter key |
| **Rollback** | revert the file; it is self-contained |

**Tests first** (`tests/e2e_shell.rs`, real bash in real tmux):

1. `a_question_submitted_with_line_feed_reaches_the_agent` — the reporter's
   verbatim line, submitted with `C-j`. Must show `🤖:` and must not leave
   the pane at `PS2`. **Must fail before the change** — this is the
   regression test for the reported defect.
2. `an_untriggered_line_submitted_with_line_feed_is_unaffected` — REQ-03 on
   the new key.
3. `an_existing_c_j_binding_is_chained_not_clobbered` — RISK-03 on the new
   key.
4. `a_chained_third_party_macro_does_not_double_quote_a_triggered_line` —
   plants a `\C-m` macro ending in `\C-j`, then sends a triggered line.
   Pins RISK-09 / F-11. **Must fail** with the binding change but without
   the idempotence guard.
5. zsh counterpart of (1), asserting F-10 — passes before *and* after, and
   exists to stop a future "port the fix to zsh".

**Completion:** `bash -n`, `shellcheck`, and all five tests green.

## TASK-4 — preamble, version, docs (REQ-34, ADR-008)

| | |
|---|---|
| **Files** | `prompt.rs`, `tests/unit_prompt.rs` (new), `Cargo.toml`, `Cargo.lock`, `README.md` |
| **Depends on** | TASK-1…3 |
| **Risk** | R1 |

- New `PREAMBLE` per ADR-008; a unit test pins the four load-bearing clauses
  so a future edit cannot quietly drop one.
- Version `0.1.0` → `0.1.1` in `crates/ama/Cargo.toml` and `Cargo.lock`.
- README: document `tools:`, and state that the trigger works whichever byte
  the terminal sends for Return.

## Test matrix

| Requirement | Level | File |
|---|---|---|
| REQ-31 | unit | `unit_config.rs` |
| REQ-32 | unit + integration | `unit_config.rs`, `integration_cli.rs` |
| REQ-33 | e2e (bash CR + LF, zsh both) | `e2e_shell.rs` |
| REQ-34 | unit + eval | `unit_prompt.rs`, `07-verification.md` |
| NFR-09 | e2e | `e2e_shell.rs` |
| No regression | full suite | `./check.sh` |

## Feedback loop

`./check.sh` — rustfmt, clippy (`-D warnings`, no unwrap/expect in lib/bins),
`bash -n`, `zsh -n`, shellcheck, build, and the whole test suite single-
threaded. It is the gate for SG4.

The REQ-34 eval is *not* in `check.sh`: it spends money and needs a live
agent. It runs by hand and is recorded in 07-verification.md, per SDLC §12.

## SG3 — Plan approved

Accepted 2026-09-26.
