---
software_version: 0.1.0
process_version: 1.0.0
stage: S5-verification
status: in-review
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [03-product-requirements.md, 04-design.md, 05-implementation-plan.md, 06-build-record.md]
updated: 2026-09-26
---

# Verification

> **Superseded in part.** This records S5 as it stood at 124 tests, before
> amendments A-05…A-09. The suite is now larger and the config path, adapter
> rules and install story have all changed. The cycle index lists re-verification
> as the next action; §5's defect log and §6's residual risks still stand.

Reproduce everything below with one command from the repository root:

```
./check.sh
```

## 1. Result

| | |
|---|---|
| **Tests** | 124 passing, 0 failing, 0 skipped on this machine |
| **`./check.sh`** | Exit 0. Nine steps: rustfmt, clippy (lib+bins, `unwrap_used`/`expect_used` denied), clippy (tests), `bash -n` on `ama.bash`, `bash -n` on `install.sh`, `zsh -n` on `ama.zsh`, shellcheck, build, tests |
| **Skipped locally** | `shellcheck` only — not installed and no root access here. CI installs it via apt and runs it for real |
| **NFR-01** | 7.0 ms median per invocation over 20 runs, against a 50 ms budget |
| **Independent review** | Whole-branch review on the most capable model, plus a scoped re-review of the fix wave. Verdict: ready to merge |

## 2. Test layers and what each one can catch

The three layers exist because the risks differ in kind, not to pad a number.

| Layer | Count | Catches |
|---|---|---|
| Unit | 67 | Adapter insert-only semantics, config validation and error types, session-key derivation and sanitisation, pane slicing and capping, prompt composition, render state machine, lossy decoding of malformed bytes |
| Shell property (TEST-P01) | 20 | Escape correctness, by quoting a string with the **real** shell function, letting **the shell itself** parse it back, and requiring a byte-identical round trip — run against both bash and zsh, over a proptest sweep plus hand fixtures |
| Integration | 23 | The binary end to end against fake agents: streaming, silence, failure, cwd, exit codes, transcript permissions, every subcommand |
| End-to-end (tmux) | 19 | A real `bash -i` and `zsh -i` driven by `send-keys`, asserted against `capture-pane` |

The end-to-end layer is not optional. It is the only layer that can catch the defect
class that eliminated design approach (A) during discovery, and it is where the two
most serious defects of this cycle were found — one by a test, one by a reviewer
running the acceptance criterion by hand.

## 3. Evidence for the requirements the product lives or dies on

| Requirement | Evidence |
|---|---|
| REQ-02 — the prompt reaches the agent byte-for-byte | `@@ list *.rs and $HOME \| grep foo && done?` arrives unexpanded. The assertion matches the agent's *mirrored* question line, because the obvious assertions do not discriminate — see §5 finding V-4 |
| REQ-03 — non-trigger lines behave as in an uninstrumented shell | Ordinary commands, pipelines, quoted strings, failing commands, and a multi-line `for` loop all behave identically. `$?` is preserved through the hook (verified: `(exit 42); echo $?` → 42) |
| REQ-11 — context is the visible pane | The acceptance criterion run verbatim: `ls /nonexistent`, then `@@ why did that fail` as the **first** trigger; the `ls` error reaches the agent. This failed before amendment A-04 |
| NFR-05 — exit code 130 on interrupt | Only provable in a real shell: `ExitStatus::code()` returns `None` on signal death and 130 is a convention an interactive shell computes into `$?`. A tmux test interrupts `fake-agent-slow` mid-sleep and asserts the pane shows `rc=130` |
| RISK-02 — the escape boundary | TEST-P01 in both shells, plus a composition test, because TEST-P01 alone passes against a hook that stops *calling* the escape function |

## 4. The product, exercised for real

Beyond the suite, the shipped binary was driven against the **real** `claude` CLI in a
real tmux pane, typing README example 1 verbatim:

```
user@host:~/project-x$ @@ in one sentence, what does this repo's ama tool do?
🤖: `ama` lets you type `@@ <prompt>` directly at your shell prompt to invoke your own
    agent CLI inline (BYOA) and get answers or tasks done without leaving the terminal
    or switching to a separate agent session.
```

The apostrophe in `repo's` survived, the pane was scraped for context, and the answer
streamed back under the robot prefix.

## 5. Defects found by verification, not by users

Recorded because the point of the process is that these did not ship. Each was found
by an independent reader, and each was reproduced before being fixed.

| ID | Found by | Defect |
|---|---|---|
| V-1 | Task 1 review | The RISK-02 property sweep never generated a newline: proptest parses `.` with `dot_matches_new_line = false`, so the security proof excluded input bracketed paste delivers routinely |
| V-2 | Task 5 review | Three early returns in `agent::run` skipped both the thread join and `child.wait()`. `Child::Drop` neither waits nor kills, so a broken pipe on `ama`'s own stdout could leave the agent blocked forever on a full pipe with no reader and no killer |
| V-3 | Task 7 implementation | `tmux capture-pane` pads every capture to full pane height, so dozens of blank lines were being sent to the agent on a fresh pane and right after `clear` — the two most common real scenarios. No unit test could have seen it |
| V-4 | Whole-branch review | The REQ-02 metacharacter test passed against a hook performing **no escaping at all**; under that mutant the agent received `$HOME` expanded and `$(id -un)` **executed** out of a typed question |
| V-5 | Whole-branch review | The first question in a pane got no terminal context, so the headline use case — "why did that build fail?" — returned a useless answer. REQ-11's statement contradicted its own acceptance criterion (amendment A-04) |
| V-6 | Whole-branch review | `@@ --no-context` silently sent context *and* mangled the question, on a path the design names as one of only two RISK-01 mitigations |
| V-7 | Task 7 review | `eval "$(ama init bash)"` — the documented install path — aborted the caller. In a `.bashrc` it silently skipped every later line for non-interactive shells |
| V-8 | Task 7 review | Ctrl-L destroyed a half-typed command |
| V-9 | Whole-branch review | A quoted operator before the trigger mangled an ordinary command, with a silent variant: `echo 'clear && @@ joke'` printed corrupted output |
| V-10 | Whole-branch review | REQ-10's "agent stderr appears" was asserted by nothing; `Stdio::inherit()` → `Stdio::null()` would have passed the entire suite while swallowing every diagnostic a real agent emits |

**Seven tests that could not fail** were found and fixed across the cycle. Three
originated in the implementation plan. The lesson is recorded here rather than left
implicit: a test is not finished until it has been *seen* to fail against the defect it
names. Several were caught only because an implementer mutated the code and re-ran
rather than banking a green.

## 6. Residual risk

| ID | Residual risk | Disposition |
|---|---|---|
| RISK-01 | Terminal content reaches the agent unfiltered | **Accepted by the owner.** Redaction was cut from 0.1.0 scope. Mitigated only by `max_context_lines` and `--no-context`, and documented in the README's "what it sends" section |
| V-11 | The unbalanced-quote guard also rejects valid trigger lines whose head contains an apostrophe inside double quotes | Parked. Fall-through means the user's own shell handles the line, which is REQ-03's contract, but for a prompt containing `$(…)` the shell expands it. A tighter guard needs real quote parsing — v0.2.0 |
| V-12 | Trigger *counting* is fooled by ordinary output containing `@@ ` (e.g. `cat README.md` in this repo), so A-04's widening silently does not apply | Parked. Not a regression; makes A-04's guarantee conditional |
| V-13 | ADR-006 and REQ-28 were not amended for the quote guard | Parked as **A-05 for the next cycle**. The authority docs promise a split the code now declines |
| V-14 | `is_answer_line` treats any line starting with three spaces as an answer continuation, and terminal wrapping already defeats the marker in the other direction | Parked. Both holes are bounded and fail safely — one gives slightly less context, one a stray answer fragment. Closing the wrap hole needs pane geometry the module does not have |
| V-15 | `06-build-record.md` D-2 records `@@ --no-context` as not working, which ruling R24 reversed | Parked. A frozen S4 artifact; the cycle index carries the correction |

## 7. Verification gaps

Named rather than hidden:

- `shellcheck` has never executed in this environment. CI is its first real run.
- The zsh evidence was produced against a real, unmodified zsh 5.9 extracted without
  root under `~/.local/opt/`, reached through a wrapper that fixes only `module_path`.
  The repository contains no reference to that wrapper and CI installs stock zsh, but
  a stock-install run has not happened yet.
- zsh end-to-end coverage is 3 scenarios against bash's 13. The four behavioural
  divergences are covered; "ordinary commands survive untouched" is not, on zsh.
- NFR-01's 7.0 ms is recorded in the build record but has no reproducible artifact in
  the tree — `check.sh` does not benchmark.
- Several acceptance criteria are satisfied by behaviour that is correct but unexecuted
  by any test: REQ-14's two-distinct-keys case, REQ-13's Ctrl-L reset clause, REQ-19's
  "nothing on stdin", NFR-04's wrong-type config case.

## SG5 — Verification accepted

Pending owner review. The recommendation is to release 0.1.0: no Critical findings
survive, every Important finding from the independent review is fixed and re-verified,
and the residual risks above are bounded and owned.
