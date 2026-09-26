---
software_version: 0.1.0
process_version: 1.0.0
stage: S4-build
status: complete
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [05-implementation-plan.md]
updated: 2026-09-26
---

# Build record

> **Historical record.** This build record is frozen as executed. It still says
> `question-mark-x2` and `~/.qmx2/`, and describes the pre-A-06 adapter rule.
> Current truth lives in [03-product-requirements.md](03-product-requirements.md)
> and [04-design.md](04-design.md); the changes are listed under Amendments in
> the [cycle index](README.md).

## 1. Summary

All ten tasks of `0.1.0/05-implementation-plan.md` are built on `cycle/0.1.0-ama`.
`ama` answers `@@ <question>` inline from the user's own agent CLI in both bash and
zsh, backed by 110 tests (unit, property, integration, and tmux-driven end-to-end),
all green under a single reproducible command, `./check.sh`. This task (Task 10)
closes the loop: it adds that command, wires it into CI, corrects the root README
against the shipped binary rather than the original placeholder text, applies one
outstanding review ruling (R22), and writes this record.

Build completion is not release approval (SG4 exit criterion, `AGENT_NATIVE_SDLC.md`
S4); SG5 verification is the next stage.

## 2. Slices implemented (Tasks 1-9)

Commit hashes are from `git log --oneline` on this branch. Test counts are the
`cargo test` total immediately after each task's own commit(s); where a review fix
round changed the count, both are shown. Sourced from the S3 execution ledger
(`progress.md`, gitignored) and cross-checked against `git log`; the final row (110)
is freshly re-run for this record, not carried over.

| Task | Slice | Commits | Tests after |
|---|---|---|---|
| 1 | Workspace, and the escape boundary proved correct | `211b6f0`, `4d8b332` | 8/8 |
| 2 | Configuration and the agent adapter table | `544e028`, `91dfef8` | 24/24 |
| 3 | Sessions and the transcript | `a511db0` | 30/30 |
| 4 | Context gathering and prompt composition | `45aa936`, `e675d5c` | 44/44 |
| 5 | Streaming the agent's answer | `1febf9c`, `1a7d5a8` | 57/57 -> 58/58 |
| 6 | Wire the CLI so `@@` answers a question | `ec573a5` | 72/72 |
| 7 | The bash hook, proved in a real terminal | `862e54a`, `63c8a71` | 97/97 -> 99/99 |
| 8 | The zsh widget | `129d98a` | 106/106 |
| 9 | Installation and `doctor` completeness | `e62ece2`, `bf851a0` | 109/109 -> 110/110 |

Each task's `feat:` commit is the implementer's delivery; a following `fix:` commit
(where present) closes findings from that task's independent review round. A
spec-amendment commit (`4907454`, `SDLC amendment A-03`) sits between Tasks 1 and 2 —
see §5.

## 3. This task (Task 10)

Delivered in this same change, committed together (see the commit that introduces
this file):

- `check.sh` — the repository's feedback loop: `cargo fmt --check`, two scoped
  `clippy` steps, `bash -n` on both shell scripts and `install.sh`, `zsh -n`
  (skipped visibly if absent), `shellcheck` (skipped visibly if absent), a full
  `cargo build --all-targets`, and `cargo test -- --test-threads=1`.
- `.github/workflows/ci.yml` — runs `./check.sh` on `ubuntu-latest` after installing
  `tmux`, `zsh`, and `shellcheck`, so CI gets the one step this machine cannot run
  locally (shellcheck; see §6).
- `README.md` — corrected against the shipped binary (§4 covers the one place the
  plan's own suggested text was itself wrong).
- `.gitignore` — added `.qmx2/`.
- `crates/ama/tests/integration_cli.rs` — ruling R22 applied (§4).
- `crates/ama/src/lib.rs`, `crates/ama/src/main.rs` — `#![warn(clippy::unwrap_used,
  clippy::expect_used)]` (§4 explains why this replaced the brief's literal
  `Cargo.toml` mechanism).
- This file.

Test count after Task 10: **110/110** (R22 added an assertion to an existing test,
not a new test function, so the total is unchanged from Task 9's final count).

## 4. Deviations from the plan

Three deviations, all evidence-based rather than stylistic. Per `AGENT_NATIVE_SDLC.md`
S4 ("record deviations before continuing"), each is recorded here rather than applied
silently.

**D-1 — Clippy's lint gate is scoped via source attributes, not the
`Cargo.toml [lints.clippy]` table the brief's Step 2 shows.** Cargo's `[lints]` table
applies to every target in the package, including the integration-test crates under
`tests/`. Adding it as written and then running the brief's own two check.sh steps
produced 33 real `clippy` errors against existing, intentional `.expect()` calls in
`tests/integration_cli.rs` under `cargo clippy --tests -- -D warnings` — because
`-D warnings` denies every lint currently at `warn`, which now included
`clippy::unwrap_used`/`expect_used` crate-wide, contradicting the brief's own stated
reason for splitting the tests step out ("Tests may use them"). Verified by running
both commands with the table in place before deciding to change anything.
**Fix:** `#![warn(clippy::unwrap_used, clippy::expect_used)]` was added to the crate
roots that actually carry NFR-07's obligation — `src/lib.rs` and `src/main.rs` — the
same place `#![forbid(unsafe_code)]` already lives. Source attributes don't cross the
crate boundary into `tests/*.rs` (each is its own crate), so this reaches exactly
`--lib --bins` and leaves test crates untouched, with no test-file edits needed (`src/`
had zero pre-existing `unwrap`/`expect` calls, verified by grep). Both check.sh clippy
steps are green (§5).

**D-2 — The README's "what it sends" section names `ama ask --no-context --
<question>`, not the brief's `@@ --no-context`.** Verified by direct execution: the
`@@` trigger's dispatch path (`cli::run`, `invoked_as_trigger` branch) hardcodes
`no_context: false` and joins **everything** after `@@ ` into the question, with no
flag parsing at all — this is intentional per ADR-004, not a bug, but it means typing
`@@ --no-context ...` sends full context anyway and puts the literal text
`--no-context` into the question. A live run with a planted secret in scrollback
confirmed the secret was sent to the agent even when `--no-context` was typed after
`@@`. Suppressing context actually requires bypassing the trigger and invoking `ama
ask --no-context --` directly, which the README now says. This is the one place a
brief-supplied README correction would itself have shipped a new inaccuracy, so it was
not applied literally. The title/prose `aka` -> `ama` rename (Step 5.1) and the rest
of Step 5 were already accurate or were applied as written.

**D-3 — Ruling R22 applied.** `doctor_reports_every_field_and_succeeds_when_healthy`
in `crates/ama/tests/integration_cli.rs` asserted only that the word `integration`
appears in `doctor`'s stdout — true even for a stubbed `integration_status()` that
always returns "not loaded", since that string itself contains the field label. Added
an assertion that the `integration`-prefixed line contains `loaded (` when
`$AMA_SESSION` is set (the healthy-path fixture already sets it). Verified
discriminating by temporarily inverting `integration_status()` to always return the
not-loaded branch, confirming the new assertion fails with a clear message, then
reverting; `git diff` on `src/cli.rs` is empty post-revert. Full transcript in
`task-10-report.md`.

## 5. Verification

### 5.1 NFR-01 (measured in Task 6, Step 6; not re-measured here)

Debug build, `/bin/true` as the agent, 20 invocations:

- Naive total: 0.153s / 20 = ~7.7ms mean per invocation.
- Proper per-invocation median (bash `time` around each of the 20 runs individually):
  18 samples at 0.007s, 2 at 0.008s -> **median 7.0ms**.

Budget is 50ms/invocation. Measured median is **~7x under budget**.

### 5.2 `./check.sh` (this task, fresh run)

```
ama -- checks
rustc 1.98.1 (48a229cea 2026-09-01)

  rustfmt                           ok
  clippy                            ok
  clippy (tests)                    ok
  bash syntax                       ok
  install.sh syntax                 ok
  zsh syntax                        ok
  shellcheck                        skipped (shellcheck not installed)
  build                             ok
  tests                             ok

  All checks passed.
```

Exit code: 0. Full test breakdown (`cargo test -- --test-threads=1`), 110 total:

| Suite | Passed |
|---|---|
| `ama` (lib unit tests, incl. `context.rs`'s inline module) | 6 |
| `ama` (bin unit tests) | 0 |
| `tests/e2e_shell.rs` (tmux-driven, bash + zsh) | 16 |
| `tests/integration_agent.rs` | 9 |
| `tests/integration_cli.rs` | 20 |
| `tests/shell_functions.rs` (property + split tests) | 18 |
| `tests/unit_config.rs` | 16 |
| `tests/unit_context.rs` | 15 |
| `tests/unit_render.rs` | 5 |
| `tests/unit_session.rs` | 5 |

### 5.3 Environment notes affecting verification

- **shellcheck** is not installed on this machine and there is no root access to add
  it. `check.sh` detects this with `command -v` and prints an explicit
  `skipped (shellcheck not installed)` line rather than silently omitting the step or
  failing; `.github/workflows/ci.yml` installs it, so CI runs the check this
  environment cannot.
- **zsh** here is a non-root, user-local install at `~/.local/bin/zsh` (disclosed by
  Task 8's implementer) wrapping a genuine, unmodified zsh 5.9 extracted from the
  distro package without root. `zsh -n` and the zsh e2e suite both ran against it
  successfully; CI installs zsh via `apt-get` for a fully stock verification path.

## 6. Amendments referenced (recorded in S2; not restated here)

- **A-01** (2026-09-26) — REQ-28 added: the trigger is recognised after a leading
  command and operator, not only at line start. See `0.1.0/README.md`'s amendment
  table and ADR-006.
- **A-02** (2026-09-26) — the planned Rust `escape` module was removed; splitting and
  quoting live only in the shell scripts, property-tested by driving a real shell. See
  `04-design.md` §2.
- **A-03** (2026-09-26, during S4) — ADR-006 rule 2 inverted so the *earliest* trigger
  always wins, matching rule 1's own principle. See `04-design.md`'s ADR-006 section.

## 7. Residual items carried to S5

- RISK-01 (terminal content reaches the agent unfiltered) remains an owner-accepted
  risk; the README's new "what it sends" section states it plainly rather than
  leaving it implicit.
- Each task's independent review deferred a number of Minor findings "to final
  review" rather than fixing them inline (catalogued per-task in the gitignored S3
  ledger). None are blocking per their own review verdicts; triaging them is S5 work,
  not repeated here.
- `check.sh`'s `shellcheck` step is unverified on this machine (see §5.3); CI is the
  first place it actually runs.

## SG4 — Build complete

Fresh evidence for every slice is above and reproducible via `./check.sh`. Pending
independent review (SG4 exit also requires a reviewer finding no unresolved blocking
issue, which is outside this task's scope).
