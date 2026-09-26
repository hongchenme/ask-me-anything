---
software_version: 0.1.1
process_version: 1.0.0
stage: S5-verification
status: in-review
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [06-build-record.md]
updated: 2026-09-26
---

# Verification — 0.1.1

**Environment.** Linux 7.0.0-34-generic · rustc 1.98.1 · bash 5.3.9 ·
zsh 5.9 · tmux 3.6 · shellcheck 0.10.0 · Claude Code 2.1.274 ·
codex-cli 0.155.1. Live runs used the owner's own `~/.ama/config.yml`
(`claude --model sonnet --effort medium`) and the 0.1.1 binary.

## 1. The reported defects, end to end

The point of this section is that both were reproduced against a **real
agent in a real terminal**, running the reporter's verbatim commands — not
against a fixture.

`ama doctor` first, since it is how a user sees what was inserted:

```console
$ ama doctor
config       /home/hong/.ama/config.yml
agent        claude -p --allowedTools WebSearch --model sonnet --effort medium
status       ok
```

### Defect 1 — `@@ what's the weather`

```console
V$ @@ 'what'\''s the weather'
🤖: I don't have a location — which city (or zip) do you want the weather for?

V$ @@ 'Austin TX'
🤖: Austin, TX today (Sat, Sep 26): mostly cloudy, currently 81°F (feels like 90°),
   high 95°, low 71° tonight. Partly sunny, light ESE wind 5-10 mph.
   Sources:
   - [AccuWeather](https://www.accuweather.com/en/us/austin/78701/weather-today/351193)
   - [Weather.com](https://weather.com/us/texas/city/austin/today)
```

Before: *"I don't have live internet access."* After: a request for the one
missing detail, then a real, sourced, correctly dated forecast — and the
follow-up proves the conversational path carries context through the fix,
not just the single-shot path.

### Defect 2 — `@@ what's the files listed under this dir`, submitted with LF

```console
V$ @@ 'what'\''s the files listed under this dir'
🤖: Files/dirs in `/home/hong/github/question-mark-x2`:
   - `.git`, `.github`, `.gitignore`, `.superpowers`
   - `0.1.0/`, `0.1.1/` (the latter is untracked, per git status)
   - `AGENT.md`, `AGENT_NATIVE_SDLC.md`
   ...
```

Submitted with `C-j`, the key that reproduced the defect. The line is
rewritten and quoted, the agent answers, and a `grep -c` for the
continuation prompt over the whole pane returns **0** — the PS2 hang is
gone.

## 2. REQ-34 eval — agent disposition

Agent behaviour is production behaviour (SDLC §12), and one passing example
would not be evidence. Runs are counted, and the "before" numbers come from
the same machine and model.

| Question | Build | Runs | Claimed no internet/tools | Useful answer |
|---|---|---|---|---|
| `what's the weather` | 0.1.0 | 3 | **3** | 0 |
| `what's the weather` | 0.1.1 | 8 | **0** | 8 — each asks for the city |
| `what's the weather in Austin TX` | 0.1.0 | 1 | 1 | 0 |
| `what's the weather in Austin TX` | 0.1.1 | 6 | **0** | 6 — sourced forecast |

The 0.1.1 runs are 4 through the raw agent with the shipped preamble, 5
through the `ama` binary (`ama ask`), and 3 through a live tmux terminal —
covering the three ways the prompt can reach the agent. No run in any
configuration reproduced the defect.

Asking for the city is the correct answer to the bare question, which
contains no location; the defect was never "it did not know the weather",
it was "it invented an incapacity".

### Isolating the two causes

Recorded because the fix only works as a pair, and a future reader who
removes half of it should find out here rather than from a user.

| Grant | Preamble | Result |
|---|---|---|
| no | 0.1.0 | "I don't have access to live weather data" |
| no | 0.1.1 | "I can't check live weather without … working web access here" |
| yes | 0.1.0 | "I don't have live weather access" |
| **yes** | **0.1.1** | **asks for the city; answers with sources once given** |

Direct proof the grant is what unblocks the tool — same flag, tool named
explicitly in the question:

```console
$ printf 'Use your WebSearch tool right now ...' | claude -p --allowedTools WebSearch
The WebSearch tool returned current conditions for Austin, TX ...

$ printf 'Use your WebSearch tool ...' | claude -p          # no grant
I don't have permission to use WebSearch right now.
```

## 3. Automated suite

`./check.sh` — every gate green, `shellcheck` included (it had been
skipping for want of the binary; installed and now enforced).

**162 tests pass**, `cargo test -- --test-threads=1`, 0 failed, 0 ignored.
143 at 0.1.0, so 19 are new.

| Requirement | Test | Level |
|---|---|---|
| REQ-31 | `claude_is_granted_web_search` | unit |
| REQ-31 | `the_grant_never_includes_web_fetch` | unit — pins RISK-10 |
| REQ-31 | `a_users_own_tool_grant_is_left_alone` | unit |
| REQ-31 | `codex_gains_a_search_flag_before_its_subcommand` | unit |
| REQ-31 | `a_users_own_codex_search_flag_still_precedes_an_inserted_exec` | unit |
| REQ-31 | `agents_with_no_verified_search_flag_gain_nothing` | unit — RISK-11 |
| REQ-32 | `tools_none_reproduces_the_0_1_0_invocation` | unit — rollback path |
| REQ-32 | `tools_research_is_the_same_as_omitting_the_key` | unit |
| REQ-32 | `an_unknown_tools_value_is_rejected_rather_than_ignored` | unit |
| REQ-32 | `adapter_false_outranks_the_tools_key` | unit |
| REQ-32 | `the_bare_string_form_gets_the_default_grant_too` | unit |
| REQ-33 | `a_question_submitted_with_line_feed_reaches_the_agent` | e2e |
| REQ-33 | `an_ordinary_command_submitted_with_line_feed_is_unaffected` | e2e — REQ-03 |
| REQ-33 | `an_existing_c_j_binding_is_chained_not_clobbered` | e2e — RISK-03 |
| REQ-33 | `a_question_submitted_with_line_feed_works_in_zsh_too` | e2e — F-10 |
| NFR-09 | `a_third_party_enter_macro_does_not_double_quote_a_triggered_line` | e2e — RISK-09 |
| REQ-34 | `the_preamble_tells_the_agent_to_use_its_tools` | unit |
| REQ-34 | `the_preamble_names_capabilities_not_vendor_tools` | unit — BYOA |

### Mutation evidence

Two tests were confirmed to fail without their fix, not merely to pass with
it — which is the only way to know a regression test would catch the
regression:

| Test | Without the fix |
|---|---|
| `a_question_submitted_with_line_feed_reaches_the_agent` | FAILS — this is defect 2 |
| `a_third_party_enter_macro_does_not_double_quote_a_triggered_line` | FAILS with the bindings but without the guard, printing `@@ ''\''what'\''\'\'''\''s it'\'''` |

## 4. Regression checks

| Check | Result |
|---|---|
| All 143 pre-existing tests | pass, unmodified except the 3 that assert the changed default (06-build-record) |
| CR Return path | unchanged — 24 e2e tests, all but one submit with CR |
| zsh | unmodified; passes under both CR and LF |
| RISK-03 chaining | pinned on both keys now, was one |
| Read-only questions under the grant (AS-01) | pass — the directory question answers from read-only tools |
| `adapter: false` | inserts nothing, including no grant |
| 0.1.0 configs | load unchanged; `tools:` defaults to `research` |
| Enter latency (NFR-08) | no subprocess added; one string comparison per keypress |

## 5. Residual risk and one new finding

### F-12 — the agent sometimes copies the `🤖:` marker into its own answer

Observed **once** during this cycle's live verification, on the second turn
of a tmux conversation:

```text
🤖: 🤖: Austin, TX today (Sat, Sep 26): mostly cloudy ...
```

The captured pane carries the previous turn's rendered `🤖:` line into the
prompt, and the agent imitated the format; `Robot` then added the real
marker. Cosmetic, and it also leaves a stray marker in the stored
transcript.

Measured rather than guessed at: three further two-turn conversations on
the transcript path produced **0/3** doubled prefixes, so it is
intermittent and appears to need the pane-scrape path.

**Not fixed in this cycle, and deliberately so.** It is not one of the two
reported defects, the mechanism (`Robot` prefixing unconditionally while
the context shows rendered answers) is unchanged from 0.1.0, and the
candidate fix is another preamble clause — which would perturb the
disposition this cycle just measured across 14 runs and would require
re-running that eval. Recommended as the next cycle's intent.

### Carried risk

| ID | State |
|---|---|
| RISK-09 | Mitigated and tested; mutation-verified |
| RISK-10 | Accepted by the owner. WebSearch can carry text to a search provider; WebFetch withheld, so no attacker-chosen destination |
| RISK-11 | Mitigated — only flags verified against installed versions, only when absent |
| RISK-01 | Unchanged from 0.1.0. Re-examined rather than assumed (02 §4) |
| AS-03 | A terminal sending neither CR nor LF for Return would still bypass the hook. No such terminal is known; `ama doctor` would not reveal it |

## 6. Release recommendation

**Recommend release.** Both reported defects are fixed and verified against
a live agent in the reporter's own terminal; 162 automated tests and every
`check.sh` gate are green; the one new finding (F-12) is cosmetic,
intermittent, pre-dates this cycle, and has a recorded follow-up.

The verifier is the same agent that implemented the change. Under SDLC §11
that is **not** sufficient for SG5: the owner is the independent approver
here, and the evidence above is written to be reproducible without this
conversation.

## SG5 — Verification accepted

**Pending owner review.**
