---
software_version: 0.1.0
process_version: 1.0.0
stage: S2-specification
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [01-intent.md, 02-discovery-and-risk.md, 03-product-requirements.md]
updated: 2026-09-26
---

# Design

## 1. Decision records

### ADR-001 — Capture the line with a readline hook that rewrites the buffer

**Status:** Accepted · **Drivers:** REQ-01, REQ-02, REQ-04, REQ-05 · **Evidence:** F-01, F-02, F-03

A PATH binary named `@@` cannot receive an unquoted natural-language question because
bash parses the line first (F-01). The shell integration therefore binds Enter to a
hook that inspects the raw buffer. When the buffer starts with the trigger, the hook
rewrites it to an equivalent, correctly quoted command and lets `accept-line` execute
it normally:

```
typed:    @@ what's this project all about?
rewritten: @@ 'what'\''s this project all about?'
```

Executing the agent *inside* the hook was built and rejected: readline's redisplay
erases the typed question and the line never enters history (F-03A). Rewriting keeps
the question on screen, in history, and interruptible, and matches what `fzf` and
`atuin` already do to the same binding.

**Consequence:** the executed line renders quoted. README examples are corrected to
match. **Consequence:** the rewrite target must be a real executable, since bash
rejects a function named `@@` (F-03).

### ADR-002 — Hybrid context source: scrape the pane, fall back to a transcript

**Status:** Accepted · **Drivers:** REQ-11, REQ-12, REQ-13, NFR-03 · **Evidence:** F-04

Inside tmux or screen, context is the captured pane, which includes the output of
ordinary commands — the difference between answering "why did that build fail?" and
not. Outside a multiplexer no portable screen read exists, so `ama` falls back to a
transcript of its own turns.

The two paths differ in how a conversation ends. Scraping needs no reset logic at all:
once the screen is cleared, no trigger line is visible, so no prior turn is in scope.
The transcript path has no such signal, so the shell hook calls `ama session reset`
when it sees `clear`, `reset`, `tput clear`, or Ctrl-L.

**Consequence:** context fidelity differs by environment. `ama doctor` names the active
source so the difference is never silent (RISK-05).

### ADR-003 — Generic stdin pipe, with adapters that only add a missing one-shot flag

**Status:** Accepted · **Drivers:** REQ-17, REQ-18, REQ-19 · **Evidence:** F-05

The default contract is the most portable one: spawn the configured command, write the
composed prompt to its stdin, stream its stdout. Any agent that reads a prompt and
writes an answer works with no code change.

On top of that sits a small adapter table, because the README's documented config
(`agent: claude --model opus --effort high`) launches an interactive session and never
returns (F-05). An adapter may *only* insert a missing one-shot flag; it never removes,
reorders, or rewrites user arguments.

| Agent | Condition | Action |
|---|---|---|
| `claude` | no `-p` / `--print` | insert `-p` after the program |
| `ollama` | no subcommand | insert `run` |
| anything else | — | unchanged |

The structured form (`command: [...]`, optional `{prompt}` placeholder) bypasses
adapters entirely and is the documented escape hatch.

### ADR-004 — One binary, two names, dispatched on `argv[0]`

**Status:** Accepted · **Drivers:** REQ-27, ADR-001

`ama` is the canonical CLI. `@@` is the same binary installed under a second name; when
`argv[0]` is `@@`, the arguments are the prompt and the call is equivalent to
`ama ask --`. One build artifact, no shim to keep in sync, and `@@` remains usable
directly in scripts where the caller does its own quoting.

### ADR-005 — Canonical name `ama`, trigger `@@`

**Status:** Accepted · **Drivers:** INT-04

The product is "ask me anything", so the binary is `ama`. The trigger stays `@@`: the
repository's namesake `??` is a bash glob and would be expanded or mangled before
reaching any handler, whereas `@` carries no special meaning to bash or zsh. The
existing `~/.qmx2/` configuration directory is retained; renaming it would break the
README for no benefit.

### ADR-006 — The trigger is recognised at any command position, and only the tail is rewritten

**Status:** Accepted · **Drivers:** REQ-28 · **Added by:** Amendment A-01 during S3

README example 3 is `clear && @@ show me the joke of the day`. The trigger is therefore
not always the first thing on the line, and the commands before it must still run as
ordinary shell commands. The hook splits the buffer into a **prefix** and a **prompt**
and rewrites only the tail:

```
clear && @@ show me the joke of the day
└─ prefix ─┘   └──────── prompt ───────┘
            ↓
clear && @@ 'show me the joke of the day'
```

Recognition order is deterministic, because the alternatives are ambiguous for a prompt
that itself contains `&&`:

1. Buffer begins with the trigger, ignoring leading whitespace → prefix is empty.
2. Otherwise, take the text before the **first** trigger. If it ends with `;`, `&&`,
   `||`, or `|` and optional whitespace, that text is the prefix and the remainder is
   the prompt.
3. Otherwise not a trigger line; chain to the previous binding.

**The earliest trigger always wins.** `@@ compare a && @@ b` is one prompt by rule 1,
and `clear && @@ compare a && @@ b` splits at the *first* trigger by rule 2 — prefix
`clear && `, prompt `compare a && @@ b`. A prompt that merely contains an operator
(`@@ list *.rs | grep foo`) is caught by rule 1 and never reaches rule 2.

**Amendment A-03, 2026-09-26 (during S4).** Rule 2 originally said "the **last**
operator", which a greedy regex implements naturally. Task 1's review demonstrated the
result is incoherent: `@@ compare a && @@ b` was one prompt, but adding a leading
`clear && ` made the same text split at the *second* trigger, pushing `@@ compare a &&`
back into the user's buffer as text to execute while the agent received only `b`. Rule 1
exists precisely so the earliest trigger wins, so the original rule 2 contradicted the
decision's own principle. Rule 2 now matches it.

Implementation note: POSIX ERE has no lazy quantifier, so rule 2 is implemented with
parameter expansion (`${line%%"@@ "*}`) rather than a regex, guarded by a `*"@@ "*`
test — without that guard the expansion returns the whole line and an unrelated
`echo a; ` would split spuriously. The trailing-operator test keeps its pattern in a
single-quoted variable: inlining `[;&|]` after `=~` fails because bash's lexer
tokenizes `;&` as the case-fallthrough operator before the regex engine ever sees it.

**Consequence:** `clear && @@ …` needs no special handling for context reset on the
tmux path — `clear` runs first, so the pane is already empty when `@@` scrapes it.

## 2. Architecture

```
                    ┌──────────────────────────────────────┐
   Enter key ──────▶│  shell integration (emitted by        │
                    │  `ama init bash|zsh`)                 │
                    │  buffer starts with "@@ "?            │
                    │    yes → rewrite to  @@ '<escaped>'   │
                    │    no  → chain to previous binding    │
                    └──────────────────┬───────────────────┘
                                       │ accept-line
                                       ▼
   ┌───────────────────────────── ama (single binary) ─────────────────────────────┐
   │                                                                               │
   │  cli ──▶ config ──▶ context ──▶ prompt ──▶ agent ──▶ render                   │
   │           │            │                     │                                │
   │           │            └── session ◀─────────┴── append turn                   │
   │           │                                                                    │
   │      ~/.qmx2/config.yml        tmux capture-pane  |  ~/.qmx2/sessions/<key>    │
   └───────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       ▼
                         user's agent CLI (cwd = user's cwd,
                         prompt on stdin, answer on stdout)
```

### Module boundaries

Each module has one purpose, a narrow interface, and is testable without the others.

| Module | Purpose | Depends on | Key interface |
|---|---|---|---|
| `cli` | Parse argv, dispatch on `argv[0]` and subcommand, map errors to exit codes | all | `fn run(argv) -> ExitCode` |
| `config` | Load and validate `~/.qmx2/config.yml`; resolve `agent:` to an `AgentSpec` | — | `fn load(path) -> Result<Config>`, `fn resolve(&Config) -> AgentSpec` |
| `adapter` | The one-shot-flag table from ADR-003 | — | `fn normalize(argv) -> Vec<String>` (pure) |
| `session` | Derive the session key; read, append, and delete the transcript | — | `fn key() -> SessionKey`, `fn load/append/reset` |
| `context` | Produce context lines from pane or transcript; apply `max_context_lines` | `session` | `fn gather(&Source, cap) -> Vec<String>` |
| `prompt` | Compose context + question into the agent's input | — | `fn compose(ctx, q) -> String` (pure) |
| `agent` | Spawn, write stdin, stream stdout, propagate signals and exit status | `adapter` | `fn run(&AgentSpec, input, out) -> Result<Status>` |
| `render` | `🤖: ` prefixing over a streaming writer | — | `impl Write` |
| `shellinit` | Emit the bash and zsh integration scripts; owns `src/shell/*.{bash,zsh}` | — | `fn script(Shell) -> &'static str` |

`adapter` and `prompt` are pure functions, which is where the correctness-critical
Rust logic deliberately lives.

**Amendment A-02, 2026-09-26 (during S3).** An `escape` module in Rust was specified
and is now removed: nothing in Rust ever escapes. The `@@` process receives arguments
the shell has already parsed, so trigger splitting (ADR-006) and single-quote escaping
must happen inside the shell hook — routing every Enter press through a subprocess to
ask Rust would violate NFR-01. Both therefore live in `src/shell/ama.bash` and
`ama.zsh` as shell functions, and are property-tested from Rust by driving the real
shell (see §4). No change to requirements, risk tier, or any other module.

## 3. Data flow for one turn

1. **Dispatch.** `argv[0] == "@@"` → prompt is `argv[1..]`, joined. A leading
   `--no-context` token is peeled off that joined string first (R24): the hook rewrites
   the typed line to `@@ '<whole prompt>'`, one quoted word, so there is no argv token
   left to match on by the time this process starts. Empty prompt exits 0 (REQ-06).
2. **Session key.** `$TMUX_PANE` if set, else `$STY` plus tty, else the tty device path;
   hashed to a filesystem-safe name.
3. **Context** (skipped under `--no-context`):
   - tmux/screen → `tmux capture-pane -p` (or `screen -X hardcopy`), then select by
     *prior* conversation rather than by first trigger (A-04): two or more trigger
     lines → from the first of them to the bottom; exactly one → the whole visible
     pane, since that one is this turn's own already-echoed question; none → nothing.
   - otherwise → read `~/.qmx2/sessions/<key>.jsonl`.
   - Truncate to the last `max_context_lines` (NFR-02).
4. **Compose.** Context under a `## Terminal` heading, then the question. A short system
   preamble states that the answer goes to a terminal and should be brief.
5. **Spawn.** `adapter::normalize` the argv, spawn in the user's cwd, write the composed
   prompt to stdin, close it.
6. **Stream.** Read stdout incrementally through `render`, which writes `🤖: ` once and
   indents continuation lines. First bytes are visible before the agent exits (REQ-08).
7. **Record.** Append `{ts, question, answer}` to the transcript, mode `0600`.
8. **Exit.** 0 / 1 / 2 / 130 per NFR-05.

## 4. The escape boundary

This is the one place where a bug is more than an inconvenience: `__ama_sq` produces
text that the user's shell will execute.

Single-quote escaping is chosen because it is *total*. Inside `'…'` bash and zsh treat
every byte literally, including newline; `'` is the only character that can terminate
the quote, and it is handled by closing, emitting `\'`, and reopening:

```
sq(s) = "'" + s.replace("'", "'\\''") + "'"
```

There is no character class to enumerate and therefore no class to get wrong.

TEST-P01 verifies it the only way that proves anything: a Rust property test generates
arbitrary strings, sources the real `ama.bash` under `bash -c`, applies `__ama_sq`,
lets **bash itself** parse the result, and asserts the recovered string is byte-identical
to the input. The oracle is the shell that will actually run the code, not a Rust
reimplementation of what it is believed to do. The same harness runs against `zsh`.

Rejected alternative: escaping only "dangerous" characters, to keep the rendered line
tidier. That requires an allow-list of safe characters, which is exactly the enumerable
surface that single-quoting avoids.

## 5. Error handling

| Condition | Behaviour | Exit |
|---|---|---|
| No config file | Message naming `~/.qmx2/config.yml` and a valid two-line example | 2 |
| Malformed YAML or wrong field type | Diagnostic naming the field (NFR-04) | 2 |
| Empty `agent:` | Diagnostic naming the field | 2 |
| Agent binary not found | Message naming the resolved program and suggesting `ama doctor` | 2 |
| Agent exits non-zero | Agent stderr surfaced verbatim | 1 |
| Agent produces no output | Note that the agent exited silently | 1 |
| Ctrl-C during a turn | Partial output kept, newline emitted, turn not recorded | 130 |
| `tmux capture-pane` fails | Silent fall back to transcript; `ama doctor` reports it | — |
| Not interactive during `ama init` | Emit nothing (REQ-24) | 0 |

No fallible runtime path uses `unwrap`/`expect`; `#![forbid(unsafe_code)]` is crate-wide
(NFR-07).

## 6. Configuration schema

```yaml
# ~/.qmx2/config.yml

# Form 1 — bare string. Adapters supply a missing one-shot flag.
agent: claude --model opus --effort high

# Form 2 — structured. Bypasses adapters entirely.
# agent:
#   command: [my-bot, --ask, "{prompt}"]   # {prompt} optional; omit for stdin

max_context_lines: 200    # default 200
```

Unknown top-level keys are rejected with a diagnostic (`deny_unknown_fields`) so typos
surface immediately instead of silently defaulting.

## 7. Repository layout

```
Cargo.toml                 # workspace; edition 2024, deps pinned as in learn/
crates/ama/
  src/main.rs              # #![forbid(unsafe_code)]; thin, calls cli::run
  src/cli.rs  config.rs  adapter.rs  session.rs  context.rs
      prompt.rs  agent.rs  render.rs  shellinit.rs
  src/shell/ama.bash  ama.zsh        # __ama_sq + __ama_split live here
                                     # (A-02); embedded via include_str!
  tests/unit_*.rs                    # pure-function and config tests
  tests/shell_functions.rs           # TEST-P01: property-tests the shell
                                     # functions by driving real bash/zsh
  tests/integration_*.rs             # assert_cmd + fake agent scripts
  tests/e2e_shell.rs                 # tmux-driven; skipped if tmux absent
  tests/fixtures/fake-agent*         # echo / slow / failing / cwd-printing
```

Keeping the shell scripts as real files under `src/shell/` (rather than Rust string
literals) means they are lintable with `bash -n` and `shellcheck` in CI, and embedded
with `include_str!` so the binary stays self-contained.

## 8. Verification strategy

Three layers, each with a feedback loop an agent can run unprompted (SDLC §12):

1. **Unit** — `escape` (property test over arbitrary bytes), `adapter` (fixture table),
   `config` (valid, malformed, unknown-field, empty-agent), `context` (cap and
   first-trigger slicing against captured-pane fixtures), `prompt`, `session` keys.
2. **Integration** — `assert_cmd` against fake agent scripts covering streaming,
   failure, cwd, exit codes, transcript permissions, and every `ama` subcommand.
3. **End-to-end shell** — the tmux harness that produced F-02 through F-04: launch a
   real `bash -i`/`zsh -i` with the integration loaded, send keys, assert on
   `capture-pane`. This is the only layer that can catch the failure mode that
   eliminated approach (A), so it is not optional. Tests skip cleanly when tmux is
   absent rather than failing.

A `./check.sh` at the repository root mirrors the existing `learn/check.sh` convention
and runs fmt, clippy, `bash -n`/`shellcheck`, and all three layers.

## SG2 — Specification accepted

Pending owner review of this cycle's S0–S2 artifacts.
