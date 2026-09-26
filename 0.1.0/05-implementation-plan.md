---
software_version: 0.1.0
process_version: 1.0.0
stage: S3-plan
status: in-review
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [01-intent.md, 02-discovery-and-risk.md, 03-product-requirements.md, 04-design.md]
updated: 2026-09-26
---

# `ama` 0.1.0 Implementation Plan

> **Historical record.** This implementation plan is frozen as executed. It still says
> `question-mark-x2` and `~/.qmx2/`, and describes the pre-A-06 adapter rule.
> Current truth lives in [03-product-requirements.md](03-product-requirements.md)
> and [04-design.md](04-design.md); the changes are listed under Amendments in
> the [cycle index](README.md).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `ama`, a Rust CLI that turns `@@ <question>` typed at a bash or zsh prompt into an inline answer from the user's own agent, with the visible terminal as context.

**Architecture:** A shell hook bound to Enter rewrites a triggered line into a correctly quoted `@@ '<prompt>'` command and lets the shell execute it normally (ADR-001). A single Rust binary, installed as both `ama` and `@@`, gathers context (tmux pane scrape, else its own transcript), composes a prompt, spawns the user's configured agent with the prompt on stdin, and streams stdout back prefixed `🤖: `.

**Tech Stack:** Rust edition 2024 · `clap` (derive) · `serde` + `serde_yaml_ng` · `thiserror` + `anyhow` · dev: `assert_cmd`, `predicates`, `proptest`, `tempfile` · `tmux` for end-to-end tests.

**Spec:** [03-product-requirements.md](03-product-requirements.md) and [04-design.md](04-design.md) in this directory. Read both before starting. Requirement IDs (`REQ-nn`, `NFR-nn`) and decision IDs (`ADR-nnn`) below refer to those documents.

## Global Constraints

- Rust **edition 2024**; workspace root declares `[workspace.dependencies]`, member crates use `dep.workspace = true`. Mirrors the existing `learn/` convention.
- `#![forbid(unsafe_code)]` at crate root (NFR-07).
- No `.unwrap()` or `.expect()` on any fallible runtime path outside `#[cfg(test)]` code (NFR-07). Clippy gate enforces this.
- Exit codes are exactly: `0` success, `1` agent failure, `2` configuration error, `130` interrupted (NFR-05).
- The canonical binary name is **`ama`**; the trigger is **`@@`**; the config directory stays **`~/.qmx2/`** (ADR-005). Never write `aka`.
- Answers are prefixed `🤖: ` — U+1F916 followed by a colon and one space (REQ-07).
- `ama` must never read, store, or forward agent credentials (REQ-21).
- Config path is `$AMA_CONFIG` when set, else `~/.qmx2/config.yml` (REQ-16).
- Default `max_context_lines` is `200` (NFR-02).
- Every task ends with a commit.

## Review Focus

Five input classes the spec implies but does not enumerate. Each has a test pinned to the task that owns the code.

1. **Trigger recognition is over- or under-eager.** ` @@ x` (leading whitespace), `@@x` (no space), `@@@ x`, `@@` alone, and `echo @@ x` (trigger not at a command position) must each do the right thing. A false positive hijacks a command the user meant to run. → Task 1 Step 6, Task 7 Step 9.
2. **Pane slicing is fooled by the agent's own answer.** The pane contains the *rewritten, quoted* line, and an answer may itself contain `@@ `. Slicing on a naive `"@@ "` search starts context in the middle of a previous answer and silently truncates it. → Task 4 Step 2.
3. **Non-UTF-8 bytes and multi-byte characters.** Emoji and CJK in the prompt, and invalid UTF-8 in captured pane output (common with progress bars and `ls` of odd filenames). Must not panic and must not mangle. → Task 1 Step 4, Task 4 Step 6.
4. **Embedded newlines from bracketed paste.** Pasting two lines into the prompt yields a `READLINE_LINE` containing `\n`. Single-quoting survives it; the split and the `@@` argv handling must too. → Task 1 Step 4, Task 7 Step 9.
5. **The agent says nothing.** Exits 0 with no stdout, or emits output with no trailing newline, or writes only to stderr. The user must not be left with a bare `🤖: ` and no explanation. → Task 5 Step 8.

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | Workspace root; pinned `[workspace.dependencies]` |
| `crates/ama/Cargo.toml` | Binary crate; declares both `ama` and `@@` install names |
| `crates/ama/src/main.rs` | `#![forbid(unsafe_code)]`, module declarations, calls `cli::run()` |
| `crates/ama/src/cli.rs` | argv parsing, `argv[0]` dispatch, subcommands, error→exit-code mapping |
| `crates/ama/src/config.rs` | Load and validate `config.yml`; resolve to `AgentSpec` |
| `crates/ama/src/adapter.rs` | One-shot-flag table (ADR-003); pure |
| `crates/ama/src/session.rs` | Session key; transcript read/append/reset |
| `crates/ama/src/context.rs` | Source detection, pane capture, trigger slicing, line cap |
| `crates/ama/src/prompt.rs` | Compose context + question into agent input; pure |
| `crates/ama/src/agent.rs` | Spawn agent, feed stdin, stream stdout, map exit status |
| `crates/ama/src/render.rs` | `🤖: ` prefixing over a streaming writer |
| `crates/ama/src/shellinit.rs` | Embed and emit the shell integration scripts |
| `crates/ama/src/shell/ama.bash` | bash hook: `__ama_sq`, `__ama_split`, `__ama_hook`, bindings |
| `crates/ama/src/shell/ama.zsh` | zsh equivalent via a ZLE widget |
| `crates/ama/tests/*.rs` | Unit, shell-property, integration, and tmux e2e suites |
| `check.sh` | Repository feedback loop: fmt, clippy, shell lint, all test layers |

---

### Task 1: Workspace, and the escape boundary proved correct

The riskiest code in the project is the function that puts the user's typed text back into their shell for execution (RISK-02). It ships first, proved by driving the real shell.

**Files:**
- Create: `Cargo.toml`, `crates/ama/Cargo.toml`, `crates/ama/src/main.rs`
- Create: `crates/ama/src/shell/ama.bash`
- Test: `crates/ama/tests/shell_functions.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: shell functions `__ama_sq <string>` (prints the single-quoted form) and `__ama_split <line>` (on a trigger line, prints `prefix`, `\x1f`, `prompt` and returns 0; otherwise returns 1). Task 7 and Task 8 build the hook on top of these.

- [ ] **Step 1: Create the workspace**

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/ama"]

[workspace.package]
version = "0.1.0"
edition = "2024"
publish = false

[workspace.dependencies]
anyhow = "1.0.104"
clap = { version = "4.6.7", features = ["derive"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_yaml_ng = "0.10"
thiserror = "2.0.21"
assert_cmd = "2.2.2"
predicates = "3.1.4"
proptest = "1"
tempfile = "3"
```

`crates/ama/Cargo.toml`:

```toml
[package]
name = "ama"
version.workspace = true
edition.workspace = true
publish.workspace = true

[[bin]]
name = "ama"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
clap.workspace = true
serde.workspace = true
serde_yaml_ng.workspace = true
thiserror.workspace = true

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
proptest.workspace = true
tempfile.workspace = true
```

`crates/ama/src/main.rs`:

```rust
#![forbid(unsafe_code)]

fn main() {
    println!("ama 0.1.0");
}
```

- [ ] **Step 2: Verify it builds**

Run: `cargo build`
Expected: compiles clean.

- [ ] **Step 3: Write the shell functions**

`crates/ama/src/shell/ama.bash` — only the two pure functions for now; the hook arrives in Task 7.

```bash
# ama shell integration (bash). Sourced via: eval "$(ama init bash)"

# Single-quote a string so the shell parses it back byte-for-byte.
# Inside '...' every byte is literal; ' is the only character that can end
# the quote, so it is the only one needing treatment.
__ama_sq() {
    printf "'%s'" "${1//\'/\'\\\'\'}"
}

# Split a line into prefix and prompt at the trigger.
# Prints "<prefix><US><prompt>" and returns 0 when the line is triggered,
# where <US> is the 0x1f unit separator. Returns 1 otherwise.
# Recognition order is fixed (ADR-006): line start wins over an operator,
# so "@@ compare a && @@ b" is one prompt, not two.
__ama_split() {
    local line=$1 lead trimmed
    lead=${line%%[![:space:]]*}
    trimmed=${line#"$lead"}

    if [[ $trimmed == '@@ '* ]]; then
        printf '%s\x1f%s' "$lead" "${trimmed#'@@ '}"
        return 0
    fi
    if [[ $line =~ ^(.*[\;\&\|][[:space:]]*)@@\ (.*)$ ]]; then
        printf '%s\x1f%s' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
        return 0
    fi
    return 1
}
```

- [ ] **Step 4: Write the failing property test for the escape boundary (TEST-P01)**

`crates/ama/tests/shell_functions.rs`:

```rust
//! TEST-P01 and the split fixtures. The oracle is the real shell, not a
//! Rust reimplementation: we ask bash to quote a string, then ask bash to
//! parse the result, and require the round trip to be lossless.

use std::process::Command;

const BASH_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shell/ama.bash");

/// Quote `s` with `__ama_sq`, let the shell parse the result, recover it.
fn roundtrip(s: &str) -> String {
    let script = format!(
        r#"source '{BASH_SRC}'
           q=$(__ama_sq "$1")
           eval "printf '%s' $q""#
    );
    let out = Command::new("bash")
        .args(["-c", &script, "ama-test", s])
        .output()
        .expect("spawn bash");
    assert!(out.status.success(), "bash failed: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn roundtrips_the_readme_example() {
    let s = "what's this project all about?";
    assert_eq!(roundtrip(s), s);
}

#[test]
fn roundtrips_shell_metacharacters_literally() {
    for s in [
        "list *.rs files | grep foo && echo $HOME",
        "rm -rf / # not really",
        "back\\slash and \"double\" and 'single'",
        "$(whoami) `id` ${PATH}",
        "newline\nin\nthe\nmiddle",
        "trailing backslash \\",
        "emoji 🤖 and CJK 日本語 and accents éàü",
        "!histexpand !!",
    ] {
        assert_eq!(roundtrip(s), s, "failed to round-trip {s:?}");
    }
}

proptest::proptest! {
    #[test]
    fn roundtrips_arbitrary_strings(s in ".{0,200}") {
        proptest::prop_assert_eq!(roundtrip(&s), s);
    }
}
```

- [ ] **Step 5: Run it and watch it fail**

Run: `cargo test --test shell_functions`
Expected: FAIL — `ama.bash` exists but the test harness path or quoting is unproven. Fix until green. A failure here is a real finding, not a formality: it means the escape is wrong.

- [ ] **Step 6: Add the split fixtures (Review Focus 1 and 4)**

Append to `crates/ama/tests/shell_functions.rs`:

```rust
/// Returns Some((prefix, prompt)) when `__ama_split` recognises a trigger.
fn split(line: &str) -> Option<(String, String)> {
    let script = format!(r#"source '{BASH_SRC}'; __ama_split "$1""#);
    let out = Command::new("bash")
        .args(["-c", &script, "ama-test", line])
        .output()
        .expect("spawn bash");
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).into_owned();
    let (prefix, prompt) = s.split_once('\u{1f}')?;
    Some((prefix.to_string(), prompt.to_string()))
}

#[test]
fn recognises_triggers_at_command_positions() {
    assert_eq!(split("@@ hello"), Some((String::new(), "hello".into())));
    assert_eq!(split("   @@ hello"), Some(("   ".into(), "hello".into())));
    assert_eq!(
        split("clear && @@ show me the joke of the day"),
        Some(("clear && ".into(), "show me the joke of the day".into()))
    );
    assert_eq!(split("cd /tmp; @@ where am i"), Some(("cd /tmp; ".into(), "where am i".into())));
}

#[test]
fn line_start_wins_over_a_later_operator() {
    // The prompt merely contains "&& @@"; it is not a prefix.
    assert_eq!(
        split("@@ compare a && @@ b"),
        Some((String::new(), "compare a && @@ b".into()))
    );
}

#[test]
fn a_prompt_containing_operators_is_not_split() {
    assert_eq!(
        split("@@ list *.rs | grep foo"),
        Some((String::new(), "list *.rs | grep foo".into()))
    );
}

#[test]
fn rejects_non_triggers() {
    assert_eq!(split("echo hello"), None);
    assert_eq!(split("@@hello"), None, "trigger requires a following space");
    assert_eq!(split("@@"), None, "bare trigger is not a prompt");
    assert_eq!(split("@@@ hello"), None);
    assert_eq!(split("echo @@ hello"), None, "not at a command position");
}

#[test]
fn survives_an_embedded_newline_from_bracketed_paste() {
    assert_eq!(
        split("@@ first line\nsecond line"),
        Some((String::new(), "first line\nsecond line".into()))
    );
}
```

- [ ] **Step 7: Run and make all of it pass**

Run: `cargo test --test shell_functions`
Expected: PASS. If `rejects_non_triggers` fails on `echo @@ hello`, the operator regex is too loose — it must require a `;`, `&`, or `|` before the trigger.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/ama
git commit -m "feat: workspace and the shell escape boundary, property-tested against bash"
```

---

### Task 2: Configuration and the agent adapter table

**Files:**
- Create: `crates/ama/src/config.rs`, `crates/ama/src/adapter.rs`
- Modify: `crates/ama/src/main.rs`
- Test: `crates/ama/tests/unit_config.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `adapter::normalize(argv: &[String]) -> Vec<String>`
  - `config::config_path() -> std::path::PathBuf`
  - `config::Config { agent: AgentConfig, max_context_lines: usize }`
  - `config::Config::load(path: &Path) -> Result<Config, ConfigError>`
  - `config::Config::agent_spec(&self) -> Result<AgentSpec, ConfigError>`
  - `config::AgentSpec { argv: Vec<String>, prompt_via: PromptVia }`
  - `config::PromptVia::{Stdin, Arg(usize)}`
  - `config::ConfigError` (implements `std::error::Error`)

- [ ] **Step 1: Write the failing adapter tests**

`crates/ama/tests/unit_config.rs`:

```rust
use ama::adapter;

fn v(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

#[test]
fn claude_gains_the_one_shot_flag() {
    assert_eq!(
        adapter::normalize(&v("claude --model opus --effort high")),
        v("claude -p --model opus --effort high")
    );
}

#[test]
fn claude_keeps_an_existing_one_shot_flag() {
    assert_eq!(adapter::normalize(&v("claude -p --model opus")), v("claude -p --model opus"));
    assert_eq!(adapter::normalize(&v("claude --print")), v("claude --print"));
}

#[test]
fn adapters_match_on_the_file_name_not_the_whole_path() {
    assert_eq!(
        adapter::normalize(&v("/home/u/.local/bin/claude --model opus")),
        v("/home/u/.local/bin/claude -p --model opus")
    );
}

#[test]
fn ollama_gains_its_subcommand() {
    assert_eq!(adapter::normalize(&v("ollama llama3")), v("ollama run llama3"));
    assert_eq!(adapter::normalize(&v("ollama run llama3")), v("ollama run llama3"));
}

#[test]
fn unknown_agents_are_untouched() {
    assert_eq!(adapter::normalize(&v("llm -m gpt-4o")), v("llm -m gpt-4o"));
    assert_eq!(adapter::normalize(&v("my-bot")), v("my-bot"));
}

#[test]
fn an_empty_argv_is_returned_unchanged() {
    assert_eq!(adapter::normalize(&[]), Vec::<String>::new());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test unit_config`
Expected: FAIL — `unresolved import ama::adapter`.

- [ ] **Step 3: Make the crate a library as well as a binary**

`crates/ama/Cargo.toml`, add above `[[bin]]`:

```toml
[lib]
name = "ama"
path = "src/lib.rs"
```

Create `crates/ama/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod adapter;
pub mod config;
```

Replace `crates/ama/src/main.rs`:

```rust
#![forbid(unsafe_code)]

fn main() {
    println!("ama 0.1.0");
}
```

- [ ] **Step 4: Implement the adapter**

`crates/ama/src/adapter.rs`:

```rust
//! Known-agent adapters (ADR-003).
//!
//! An adapter may only *insert a missing one-shot flag*. It never removes,
//! reorders, or rewrites a user's arguments, so a config that already works
//! keeps working when this table changes.

/// Insert the one-shot flag a known agent needs, if it is absent.
pub fn normalize(argv: &[String]) -> Vec<String> {
    let mut out = argv.to_vec();
    let Some(program) = argv.first() else {
        return out;
    };
    // Match on the file name so an absolute path still resolves.
    let name = program.rsplit('/').next().unwrap_or(program);
    let rest = &argv[1..];

    match name {
        "claude" if !rest.iter().any(|a| a == "-p" || a == "--print") => {
            out.insert(1, "-p".to_string());
        }
        "ollama" if rest.first().map(String::as_str) != Some("run") => {
            out.insert(1, "run".to_string());
        }
        _ => {}
    }
    out
}
```

- [ ] **Step 5: Run the adapter tests**

Run: `cargo test --test unit_config`
Expected: PASS.

- [ ] **Step 6: Write the failing config tests**

Append to `crates/ama/tests/unit_config.rs`:

```rust
use ama::config::{AgentSpec, Config, ConfigError, PromptVia};
use std::io::Write;

fn write_cfg(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.yml");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(body.as_bytes()).expect("write");
    (dir, path)
}

#[test]
fn loads_the_readme_config_verbatim() {
    let (_d, p) = write_cfg("agent: claude --model opus --effort high\n");
    let cfg = Config::load(&p).expect("load");
    assert_eq!(cfg.max_context_lines, 200, "default cap");
    let spec = cfg.agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude -p --model opus --effort high"));
    assert_eq!(spec.prompt_via, PromptVia::Stdin);
}

#[test]
fn honours_an_explicit_line_cap() {
    let (_d, p) = write_cfg("agent: llm\nmax_context_lines: 40\n");
    assert_eq!(Config::load(&p).expect("load").max_context_lines, 40);
}

#[test]
fn a_structured_command_bypasses_adapters() {
    let (_d, p) = write_cfg("agent:\n  command: [claude, --model, opus]\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude --model opus"), "no -p inserted");
    assert_eq!(spec.prompt_via, PromptVia::Stdin);
}

#[test]
fn a_prompt_placeholder_becomes_an_argument() {
    let (_d, p) = write_cfg("agent:\n  command: [my-bot, --ask, \"{prompt}\"]\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.prompt_via, PromptVia::Arg(2));
}

#[test]
fn a_missing_file_says_where_it_should_be() {
    let err = Config::load(std::path::Path::new("/nonexistent/ama.yml")).unwrap_err();
    assert!(matches!(err, ConfigError::Missing(_)));
    assert!(err.to_string().contains("/nonexistent/ama.yml"));
}

#[test]
fn malformed_yaml_names_the_problem_and_does_not_panic() {
    let (_d, p) = write_cfg("agent: [unclosed\n");
    assert!(matches!(Config::load(&p).unwrap_err(), ConfigError::Parse { .. }));
}

#[test]
fn an_unknown_key_is_rejected_rather_than_ignored() {
    let (_d, p) = write_cfg("agent: llm\nmax_contxt_lines: 40\n");
    let err = Config::load(&p).unwrap_err();
    assert!(err.to_string().contains("max_contxt_lines"), "typo must be named: {err}");
}

#[test]
fn an_empty_agent_is_a_configuration_error() {
    let (_d, p) = write_cfg("agent: \"\"\n");
    assert!(matches!(Config::load(&p).unwrap_err(), ConfigError::EmptyAgent));
}

#[test]
fn an_empty_file_is_a_configuration_error_not_a_panic() {
    let (_d, p) = write_cfg("");
    assert!(Config::load(&p).is_err());
}

#[test]
fn config_path_prefers_the_environment_override() {
    // Serialised with other env-mutating tests by running single-threaded in check.sh.
    unsafe_free_env_set("AMA_CONFIG", "/tmp/custom.yml");
    assert_eq!(ama::config::config_path(), std::path::PathBuf::from("/tmp/custom.yml"));
    unsafe_free_env_remove("AMA_CONFIG");
}

// std::env::set_var is unsafe in edition 2024; the crate forbids unsafe, so
// tests shell out rather than mutate this process's environment.
fn unsafe_free_env_set(k: &str, val: &str) {
    ENV_OVERRIDE.with(|c| c.borrow_mut().insert(k.to_string(), val.to_string()));
}
fn unsafe_free_env_remove(k: &str) {
    ENV_OVERRIDE.with(|c| { c.borrow_mut().remove(k); });
}
thread_local! {
    static ENV_OVERRIDE: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}
```

Note on the last test: `std::env::set_var` is `unsafe` in edition 2024 and the crate
forbids unsafe. Replace that test with an integration-level check in Task 6 (which runs
`ama` as a subprocess and can set the variable on the child) and delete the thread-local
helper. It is written here only to flag the trap — **do not** keep a test that cannot
work.

- [ ] **Step 7: Delete the env-override test from this file**

Remove `config_path_prefers_the_environment_override` and both helpers and the
`thread_local!` block. `config_path()` is covered by Task 6 Step 5.

- [ ] **Step 8: Run to verify the config tests fail**

Run: `cargo test --test unit_config`
Expected: FAIL — `unresolved import ama::config`.

- [ ] **Step 9: Implement config**

`crates/ama/src/config.rs`:

```rust
//! Load and validate `~/.qmx2/config.yml` and resolve it to a runnable agent.

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::adapter;

/// Where the prompt is handed to the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptVia {
    /// Written to the agent's stdin (the default contract, ADR-003).
    Stdin,
    /// Substituted into `argv[n]`, which held the `{prompt}` placeholder.
    Arg(usize),
}

/// A resolved, runnable agent invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub argv: Vec<String>,
    pub prompt_via: PromptVia,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AgentConfig {
    /// `agent: claude --model opus` — adapters apply.
    Line(String),
    /// `agent: { command: [...] }` — adapters are bypassed.
    Structured { command: Vec<String> },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub agent: AgentConfig,
    #[serde(default = "default_max_context_lines")]
    pub max_context_lines: usize,
}

fn default_max_context_lines() -> usize {
    200
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(
        "no config at {0}\n\nCreate it with:\n\n  mkdir -p ~/.qmx2\n  \
         printf 'agent: claude --model opus --effort high\\n' > ~/.qmx2/config.yml"
    )]
    Missing(PathBuf),
    #[error("could not read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("{path} is not valid: {source}")]
    Parse { path: PathBuf, source: serde_yaml_ng::Error },
    #[error("`agent:` is empty; it must name a command, e.g. `agent: claude`")]
    EmptyAgent,
}

/// `$AMA_CONFIG` when set, else `~/.qmx2/config.yml`.
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("AMA_CONFIG") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".qmx2").join("config.yml")
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::Missing(path.to_path_buf()));
        }
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let cfg: Config =
            serde_yaml_ng::from_str(&text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        match &self.agent {
            AgentConfig::Line(s) if s.trim().is_empty() => Err(ConfigError::EmptyAgent),
            AgentConfig::Structured { command } if command.is_empty() => {
                Err(ConfigError::EmptyAgent)
            }
            _ => Ok(()),
        }
    }

    /// Resolve the configuration into an invocation, applying adapters only
    /// to the bare-string form.
    pub fn agent_spec(&self) -> Result<AgentSpec, ConfigError> {
        match &self.agent {
            AgentConfig::Line(line) => {
                let argv: Vec<String> =
                    line.split_whitespace().map(String::from).collect();
                if argv.is_empty() {
                    return Err(ConfigError::EmptyAgent);
                }
                Ok(AgentSpec { argv: adapter::normalize(&argv), prompt_via: PromptVia::Stdin })
            }
            AgentConfig::Structured { command } => {
                let via = command
                    .iter()
                    .position(|a| a.contains("{prompt}"))
                    .map_or(PromptVia::Stdin, PromptVia::Arg);
                Ok(AgentSpec { argv: command.clone(), prompt_via: via })
            }
        }
    }
}
```

- [ ] **Step 10: Run the full unit suite**

Run: `cargo test --test unit_config`
Expected: PASS.

- [ ] **Step 11: Commit**

```bash
git add crates/ama
git commit -m "feat: config loading, validation, and the agent adapter table"
```

---

### Task 3: Sessions and the transcript

**Files:**
- Create: `crates/ama/src/session.rs`
- Modify: `crates/ama/src/lib.rs`
- Test: `crates/ama/tests/unit_session.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `session::SessionKey` (wraps a filesystem-safe `String`, `Display`)
  - `session::key_from(tmux_pane: Option<&str>, ama_session: Option<&str>, tty: Option<&str>) -> SessionKey` (pure, for tests)
  - `session::current_key() -> SessionKey`
  - `session::Turn { question: String, answer: String }` (serde)
  - `session::transcript_path(&SessionKey) -> PathBuf`
  - `session::load_turns(&SessionKey) -> Vec<Turn>`
  - `session::append_turn(&SessionKey, &Turn) -> std::io::Result<()>`
  - `session::reset(&SessionKey) -> std::io::Result<()>`

- [ ] **Step 1: Write the failing tests**

`crates/ama/tests/unit_session.rs`:

```rust
use ama::session::{self, SessionKey, Turn};

#[test]
fn tmux_pane_wins_and_is_made_filesystem_safe() {
    let k = session::key_from(Some("%3"), Some("4711"), Some("/dev/pts/2"));
    assert_eq!(k.to_string(), "tmux-3");
}

#[test]
fn falls_back_to_the_shell_supplied_session_id() {
    let k = session::key_from(None, Some("4711"), Some("/dev/pts/2"));
    assert_eq!(k.to_string(), "sh-4711");
}

#[test]
fn falls_back_to_the_tty_path() {
    let k = session::key_from(None, None, Some("/dev/pts/2"));
    assert_eq!(k.to_string(), "tty-dev-pts-2");
}

#[test]
fn has_a_last_resort_key() {
    assert_eq!(session::key_from(None, None, None).to_string(), "default");
}

#[test]
fn strips_characters_that_are_not_safe_in_a_file_name() {
    let k = session::key_from(None, Some("../../etc/passwd"), None);
    let s = k.to_string();
    assert!(!s.contains('/'), "no separators: {s}");
    assert!(!s.contains(".."), "no traversal: {s}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test unit_session`
Expected: FAIL — `unresolved import ama::session`.

- [ ] **Step 3: Implement the session key**

Add `pub mod session;` to `crates/ama/src/lib.rs`, then `crates/ama/src/session.rs`:

```rust
//! Per-terminal session identity and the fallback transcript (ADR-002).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{BufRead, BufWriter, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionKey(String);

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Keep only characters that are unambiguous in a file name. Everything else
/// collapses to `-`, which also defeats `..` traversal from a hostile value.
fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() { "default".to_string() } else { trimmed }
}

/// Pure key derivation, so precedence is testable without a terminal.
pub fn key_from(
    tmux_pane: Option<&str>,
    ama_session: Option<&str>,
    tty: Option<&str>,
) -> SessionKey {
    let pick = |prefix: &str, v: &str| SessionKey(format!("{prefix}-{}", sanitize(v)));
    if let Some(p) = tmux_pane.filter(|v| !v.is_empty()) {
        return pick("tmux", p);
    }
    if let Some(s) = ama_session.filter(|v| !v.is_empty()) {
        return pick("sh", s);
    }
    if let Some(t) = tty.filter(|v| !v.is_empty()) {
        return pick("tty", t);
    }
    SessionKey("default".to_string())
}

pub fn current_key() -> SessionKey {
    let tmux = std::env::var("TMUX_PANE").ok();
    let sh = std::env::var("AMA_SESSION").ok();
    let tty = std::fs::read_link("/proc/self/fd/2")
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    key_from(tmux.as_deref(), sh.as_deref(), tty.as_deref())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Turn {
    pub question: String,
    pub answer: String,
}

fn sessions_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".qmx2").join("sessions")
}

pub fn transcript_path(key: &SessionKey) -> PathBuf {
    sessions_dir().join(format!("{key}.jsonl"))
}

/// Missing or corrupt transcripts read as empty rather than failing a turn:
/// losing context is a nuisance, refusing to answer is a bug.
pub fn load_turns(key: &SessionKey) -> Vec<Turn> {
    let Ok(file) = std::fs::File::open(transcript_path(key)) else {
        return Vec::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str::<Turn>(&l).ok())
        .collect()
}

pub fn append_turn(key: &SessionKey, turn: &Turn) -> std::io::Result<()> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir)?;
    let path = transcript_path(key);
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    set_owner_only(&mut opts);
    let file = opts.open(&path)?;
    let mut w = BufWriter::new(file);
    let line = serde_json::to_string(turn)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(w, "{line}")?;
    w.flush()
}

#[cfg(unix)]
fn set_owner_only(opts: &mut std::fs::OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    opts.mode(0o600);
}

#[cfg(not(unix))]
fn set_owner_only(_opts: &mut std::fs::OpenOptions) {}

pub fn reset(key: &SessionKey) -> std::io::Result<()> {
    match std::fs::remove_file(transcript_path(key)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
```

Add `serde_json = "1.0.151"` to `[workspace.dependencies]` and `serde_json.workspace = true` to the crate's `[dependencies]`.

- [ ] **Step 4: Run to verify the tests pass**

Run: `cargo test --test unit_session`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/ama
git commit -m "feat: per-terminal session keys and the fallback transcript"
```

---

### Task 4: Context gathering and prompt composition

This task owns Review Focus 2 and 3.

**Files:**
- Create: `crates/ama/src/context.rs`, `crates/ama/src/prompt.rs`
- Modify: `crates/ama/src/lib.rs`
- Test: `crates/ama/tests/unit_context.rs`

**Interfaces:**
- Consumes: `session::{SessionKey, load_turns}`.
- Produces:
  - `context::Source::{Tmux, Screen, Transcript}`
  - `context::detect_source() -> Source`
  - `context::slice_from_first_trigger(lines: &[String]) -> &[String]` (pure)
  - `context::cap(lines: &[String], max: usize) -> &[String]` (pure)
  - `context::gather(source: Source, key: &SessionKey, max: usize) -> Vec<String>`
  - `prompt::compose(context: &[String], question: &str) -> String` (pure)
  - `render::ANSWER_PREFIX` and `render::CONT_INDENT` are defined in Task 5 but referenced here; define them in `context.rs` for now as `pub const` and re-export, to avoid a forward dependency:
    - `context::ANSWER_PREFIX: &str = "🤖: "`
    - `context::CONT_INDENT: &str = "   "`

- [ ] **Step 1: Write the failing slicing tests**

`crates/ama/tests/unit_context.rs`:

```rust
use ama::context;
use ama::prompt;

fn lines(s: &str) -> Vec<String> {
    s.lines().map(String::from).collect()
}

#[test]
fn context_is_everything_from_the_first_trigger_line_down() {
    let pane = lines(
        "some earlier output\n\
         user@host:~$ ls\n\
         a.rs  b.rs\n\
         user@host:~$ @@ 'what is here'\n\
         🤖: two rust files\n\
         user@host:~$ @@ 'and now'",
    );
    let got = context::slice_from_first_trigger(&pane);
    assert_eq!(got.len(), 3);
    assert!(got[0].contains("@@ 'what is here'"));
    assert!(got[2].contains("and now"));
}

#[test]
fn an_empty_pane_yields_no_context() {
    assert!(context::slice_from_first_trigger(&[]).is_empty());
}

#[test]
fn a_pane_with_no_trigger_yields_no_context() {
    // After `clear` there is no trigger on screen, so the conversation is new.
    assert!(context::slice_from_first_trigger(&lines("user@host:~$ ")).is_empty());
}
```

- [ ] **Step 2: Write the failing test for Review Focus 2**

Append to `crates/ama/tests/unit_context.rs`:

```rust
#[test]
fn a_trigger_inside_the_agents_own_answer_does_not_start_the_context() {
    // The agent explained the tool, so its answer contains "@@ ". Slicing must
    // not treat that as the start of the conversation -- doing so would cut the
    // real first question out of the context.
    let pane = lines(
        "user@host:~$ @@ 'how do i use this'\n\
         🤖: type @@ followed by a space, like\n   \
         @@ what is this project\n\
         user@host:~$ @@ 'thanks'",
    );
    let got = context::slice_from_first_trigger(&pane);
    assert!(
        got[0].contains("how do i use this"),
        "context must start at the real first question, got {:?}",
        got.first()
    );
    assert_eq!(got.len(), 4);
}
```

- [ ] **Step 3: Run to verify both fail**

Run: `cargo test --test unit_context`
Expected: FAIL — `unresolved import ama::context`.

- [ ] **Step 4: Implement context**

Add `pub mod context;` and `pub mod prompt;` to `crates/ama/src/lib.rs`, then
`crates/ama/src/context.rs`:

```rust
//! Where the conversation's context comes from (ADR-002).

use crate::session::{self, SessionKey};

/// Rendered answers are marked so that slicing can tell the agent's words
/// apart from the user's commands.
pub const ANSWER_PREFIX: &str = "🤖: ";
pub const CONT_INDENT: &str = "   ";

const TRIGGER_IN_LINE: &str = "@@ ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Tmux,
    Screen,
    Transcript,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Source::Tmux => "tmux pane",
            Source::Screen => "screen window",
            Source::Transcript => "transcript (no multiplexer)",
        })
    }
}

pub fn detect_source() -> Source {
    let set = |k: &str| std::env::var(k).is_ok_and(|v| !v.is_empty());
    if set("TMUX") {
        Source::Tmux
    } else if set("STY") {
        Source::Screen
    } else {
        Source::Transcript
    }
}

/// True when the line is part of a rendered answer rather than a command.
fn is_answer_line(line: &str) -> bool {
    line.trim_start().starts_with(ANSWER_PREFIX.trim_end())
        || line.starts_with(CONT_INDENT)
}

/// Everything from the first *command* line bearing the trigger to the end.
/// Answer lines are skipped during the search (Review Focus 2).
pub fn slice_from_first_trigger(lines: &[String]) -> &[String] {
    let start = lines
        .iter()
        .position(|l| !is_answer_line(l) && l.contains(TRIGGER_IN_LINE));
    match start {
        Some(i) => &lines[i..],
        None => &[],
    }
}

/// Keep the most recent `max` lines (NFR-02).
pub fn cap(lines: &[String], max: usize) -> &[String] {
    if lines.len() <= max { lines } else { &lines[lines.len() - max..] }
}

fn capture_tmux() -> Option<Vec<String>> {
    let out = std::process::Command::new("tmux")
        .args(["capture-pane", "-p"])
        .output()
        .ok()?;
    out.status.success().then(|| decode_lines(&out.stdout))
}

fn capture_screen() -> Option<Vec<String>> {
    let dir = std::env::temp_dir().join(format!("ama-hardcopy-{}", std::process::id()));
    let ok = std::process::Command::new("screen")
        .args(["-X", "hardcopy", &dir.to_string_lossy()])
        .status()
        .ok()?
        .success();
    let text = ok.then(|| std::fs::read(&dir).ok()).flatten();
    let _ = std::fs::remove_file(&dir);
    text.map(|b| decode_lines(&b))
}

/// Pane output is arbitrary bytes: progress bars, odd file names, partial
/// escape sequences. Decode lossily rather than failing the turn
/// (Review Focus 3).
fn decode_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect()
}

fn from_transcript(key: &SessionKey) -> Vec<String> {
    let mut out = Vec::new();
    for t in session::load_turns(key) {
        out.push(format!("$ @@ '{}'", t.question));
        for (i, l) in t.answer.lines().enumerate() {
            out.push(if i == 0 {
                format!("{ANSWER_PREFIX}{l}")
            } else {
                format!("{CONT_INDENT}{l}")
            });
        }
    }
    out
}

/// Gather context for this turn, falling back silently when a capture fails
/// (RISK-05 surfaces the active source through `ama doctor` instead).
pub fn gather(source: Source, key: &SessionKey, max: usize) -> Vec<String> {
    let captured = match source {
        Source::Tmux => capture_tmux(),
        Source::Screen => capture_screen(),
        Source::Transcript => None,
    };
    let lines = match captured {
        Some(pane) => slice_from_first_trigger(&pane).to_vec(),
        None => from_transcript(key),
    };
    cap(&lines, max).to_vec()
}
```

- [ ] **Step 5: Run to verify the slicing tests pass**

Run: `cargo test --test unit_context`
Expected: PASS.

- [ ] **Step 6: Add the cap and lossy-decode tests (Review Focus 3)**

Append to `crates/ama/tests/unit_context.rs`:

```rust
#[test]
fn the_cap_keeps_the_most_recent_lines() {
    let many: Vec<String> = (0..1000).map(|i| format!("line {i}")).collect();
    let got = context::cap(&many, 200);
    assert_eq!(got.len(), 200);
    assert_eq!(got[0], "line 800");
    assert_eq!(got[199], "line 999");
}

#[test]
fn the_cap_is_a_no_op_below_the_limit() {
    let few: Vec<String> = (0..5).map(|i| format!("line {i}")).collect();
    assert_eq!(context::cap(&few, 200).len(), 5);
}

#[test]
fn multibyte_content_survives_slicing_and_capping() {
    let pane = lines("$ @@ 'なにこれ 🤖'\n🤖: 日本語です");
    let got = context::slice_from_first_trigger(&pane);
    assert!(got[0].contains("なにこれ 🤖"));
    assert!(got[1].contains("日本語です"));
}
```

- [ ] **Step 7: Write the failing prompt-composition tests**

Append to `crates/ama/tests/unit_context.rs`:

```rust
#[test]
fn a_composed_prompt_carries_the_question_and_the_terminal() {
    let out = prompt::compose(&lines("$ ls\na.rs"), "what is here?");
    assert!(out.contains("what is here?"));
    assert!(out.contains("$ ls"));
    assert!(out.contains("a.rs"));
    assert!(out.contains("## Terminal"));
}

#[test]
fn an_empty_context_omits_the_terminal_section_entirely() {
    let out = prompt::compose(&[], "hello");
    assert!(!out.contains("## Terminal"), "no empty section: {out}");
    assert!(out.contains("hello"));
}
```

- [ ] **Step 8: Implement prompt composition**

`crates/ama/src/prompt.rs`:

```rust
//! Compose the text handed to the agent on stdin.

const PREAMBLE: &str = "\
You are answering inside a terminal, inline, while the user works. Be brief and \
concrete: a few lines, plain prose, no headings and no preamble. If the terminal \
transcript below is relevant to the question, use it; if it is not, ignore it.";

pub fn compose(context: &[String], question: &str) -> String {
    let mut s = String::with_capacity(PREAMBLE.len() + question.len() + 256);
    s.push_str(PREAMBLE);
    if !context.is_empty() {
        s.push_str("\n\n## Terminal\n\n```\n");
        for line in context {
            s.push_str(line);
            s.push('\n');
        }
        s.push_str("```\n");
    }
    s.push_str("\n## Question\n\n");
    s.push_str(question);
    s.push('\n');
    s
}
```

- [ ] **Step 9: Run the whole unit suite**

Run: `cargo test --test unit_context`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/ama
git commit -m "feat: context gathering from pane or transcript, and prompt composition"
```

---

### Task 5: Streaming the agent's answer

This task owns Review Focus 5.

**Files:**
- Create: `crates/ama/src/render.rs`, `crates/ama/src/agent.rs`
- Modify: `crates/ama/src/lib.rs`
- Create: `crates/ama/tests/fixtures/fake-agent-echo`, `fake-agent-slow`, `fake-agent-fail`, `fake-agent-silent`, `fake-agent-cwd`
- Test: `crates/ama/tests/unit_render.rs`, `crates/ama/tests/integration_agent.rs`

**Interfaces:**
- Consumes: `config::{AgentSpec, PromptVia}`.
- Produces:
  - `render::Robot<W: Write>` with `Robot::new(w: W) -> Self` and `impl Write`
  - `render::Robot::wrote_anything(&self) -> bool`
  - `agent::AgentError` (implements `std::error::Error`)
  - `agent::run(spec: &AgentSpec, input: &str, out: &mut dyn Write) -> Result<i32, AgentError>`

- [ ] **Step 1: Write the failing render tests**

`crates/ama/tests/unit_render.rs`:

```rust
use ama::render::Robot;
use std::io::Write;

fn render(chunks: &[&str]) -> String {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut r = Robot::new(&mut buf);
        for c in chunks {
            r.write_all(c.as_bytes()).expect("write");
        }
        r.flush().expect("flush");
    }
    String::from_utf8(buf).expect("utf8")
}

#[test]
fn the_first_line_gets_the_robot_prefix() {
    assert_eq!(render(&["hello\n"]), "🤖: hello\n");
}

#[test]
fn continuation_lines_are_indented_not_re_prefixed() {
    assert_eq!(render(&["one\ntwo\n"]), "🤖: one\n   two\n");
}

#[test]
fn the_prefix_is_written_once_across_chunk_boundaries() {
    // Streaming means a line can arrive in pieces; the prefix must not repeat.
    assert_eq!(render(&["hel", "lo\nwor", "ld\n"]), "🤖: hello\n   world\n");
}

#[test]
fn output_without_a_trailing_newline_still_ends_the_line() {
    assert_eq!(render(&["no newline"]), "🤖: no newline\n");
}

#[test]
fn nothing_in_means_nothing_out() {
    assert_eq!(render(&[]), "");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test unit_render`
Expected: FAIL — `unresolved import ama::render`.

- [ ] **Step 3: Implement the renderer**

Add `pub mod render;` and `pub mod agent;` to `crates/ama/src/lib.rs`, then
`crates/ama/src/render.rs`:

```rust
//! Prefix streamed agent output with the robot marker.

use crate::context::{ANSWER_PREFIX, CONT_INDENT};
use std::io::{self, Write};

/// Writes `🤖: ` before the first byte and indents every later line, so a
/// multi-line answer reads as one block and `context::slice_from_first_trigger`
/// can tell answers from commands.
pub struct Robot<W: Write> {
    inner: W,
    started: bool,
    at_line_start: bool,
}

impl<W: Write> Robot<W> {
    pub fn new(inner: W) -> Self {
        Self { inner, started: false, at_line_start: true }
    }

    /// False when the agent produced no output at all (Review Focus 5).
    pub fn wrote_anything(&self) -> bool {
        self.started
    }

    /// Terminate a final line that arrived without a newline.
    pub fn finish(&mut self) -> io::Result<()> {
        if self.started && !self.at_line_start {
            self.inner.write_all(b"\n")?;
            self.at_line_start = true;
        }
        self.inner.flush()
    }
}

impl<W: Write> Write for Robot<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            if self.at_line_start {
                if !self.started {
                    self.inner.write_all(ANSWER_PREFIX.as_bytes())?;
                    self.started = true;
                } else {
                    self.inner.write_all(CONT_INDENT.as_bytes())?;
                }
                self.at_line_start = false;
            }
            self.inner.write_all(&[b])?;
            if b == b'\n' {
                self.at_line_start = true;
                // Flush per line so the user sees the answer as it arrives.
                self.inner.flush()?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.finish()
    }
}
```

- [ ] **Step 4: Run to verify the render tests pass**

Run: `cargo test --test unit_render`
Expected: PASS. Note `render(&[])` returns `""` because `finish` is a no-op when nothing started.

- [ ] **Step 5: Create the fake agents**

`crates/ama/tests/fixtures/fake-agent-echo`:

```bash
#!/usr/bin/env bash
# Echoes the prompt it received on stdin, so tests can assert on composition.
cat
```

`crates/ama/tests/fixtures/fake-agent-slow`:

```bash
#!/usr/bin/env bash
cat >/dev/null
echo "first"
sleep 3
echo "second"
```

`crates/ama/tests/fixtures/fake-agent-fail`:

```bash
#!/usr/bin/env bash
cat >/dev/null
echo "agent exploded" >&2
exit 7
```

`crates/ama/tests/fixtures/fake-agent-silent`:

```bash
#!/usr/bin/env bash
cat >/dev/null
exit 0
```

`crates/ama/tests/fixtures/fake-agent-cwd`:

```bash
#!/usr/bin/env bash
cat >/dev/null
pwd
```

Then: `chmod +x crates/ama/tests/fixtures/fake-agent-*`

- [ ] **Step 6: Write the failing agent tests**

`crates/ama/tests/integration_agent.rs`:

```rust
use ama::agent;
use ama::config::{AgentSpec, PromptVia};

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn spec(name: &str) -> AgentSpec {
    AgentSpec { argv: vec![fixture(name)], prompt_via: PromptVia::Stdin }
}

#[test]
fn the_prompt_reaches_the_agent_on_stdin() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-echo"), "hello prompt", &mut out).expect("run");
    assert_eq!(code, 0);
    assert_eq!(String::from_utf8_lossy(&out), "🤖: hello prompt\n");
}

#[test]
fn a_prompt_placeholder_is_substituted_into_argv() {
    let s = AgentSpec {
        argv: vec!["/bin/echo".into(), "{prompt}".into()],
        prompt_via: PromptVia::Arg(1),
    };
    let mut out: Vec<u8> = Vec::new();
    agent::run(&s, "substituted", &mut out).expect("run");
    assert_eq!(String::from_utf8_lossy(&out), "🤖: substituted\n");
}

#[test]
fn a_failing_agent_reports_its_exit_code() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-fail"), "x", &mut out).expect("run");
    assert_eq!(code, 7);
}

#[test]
fn a_missing_agent_is_a_typed_error_naming_the_program() {
    let s = AgentSpec { argv: vec!["definitely-not-installed".into()], prompt_via: PromptVia::Stdin };
    let err = agent::run(&s, "x", &mut Vec::new()).unwrap_err();
    assert!(err.to_string().contains("definitely-not-installed"), "{err}");
}

#[test]
fn an_empty_argv_is_a_typed_error_not_a_panic() {
    let s = AgentSpec { argv: vec![], prompt_via: PromptVia::Stdin };
    assert!(agent::run(&s, "x", &mut Vec::new()).is_err());
}

#[test]
fn the_agent_inherits_the_working_directory() {
    let mut out: Vec<u8> = Vec::new();
    agent::run(&spec("fake-agent-cwd"), "x", &mut out).expect("run");
    let cwd = std::env::current_dir().expect("cwd");
    assert!(String::from_utf8_lossy(&out).contains(&cwd.to_string_lossy().to_string()));
}
```

- [ ] **Step 7: Implement the agent runner**

`crates/ama/src/agent.rs`:

```rust
//! Spawn the user's agent, feed it the prompt, stream its answer back.

use crate::config::{AgentSpec, PromptVia};
use crate::render::Robot;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("no agent command configured")]
    NoCommand,
    #[error("could not start `{program}`: {source}\n\nCheck `agent:` in your config, or run `ama doctor`.")]
    Spawn { program: String, source: std::io::Error },
    #[error("failed while talking to `{program}`: {source}")]
    Io { program: String, source: std::io::Error },
}

/// Run the agent. Returns its exit code; streams stdout through `out`.
/// The agent's stderr is inherited so its diagnostics reach the user directly.
pub fn run(spec: &AgentSpec, input: &str, out: &mut dyn Write) -> Result<i32, AgentError> {
    let Some(program) = spec.argv.first().cloned() else {
        return Err(AgentError::NoCommand);
    };

    let mut argv = spec.argv.clone();
    let feed_stdin = match spec.prompt_via {
        PromptVia::Arg(i) if i < argv.len() => {
            argv[i] = argv[i].replace("{prompt}", input);
            false
        }
        _ => true,
    };

    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(if feed_stdin { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| AgentError::Spawn { program: program.clone(), source })?;

    // Write the prompt on a worker thread: an agent that emits output before
    // draining stdin would otherwise deadlock against a full pipe.
    let writer = feed_stdin.then(|| {
        let mut sink = child.stdin.take();
        let payload = input.to_string();
        std::thread::spawn(move || {
            if let Some(mut s) = sink.take() {
                let _ = s.write_all(payload.as_bytes());
            }
        })
    });

    let mut robot = Robot::new(out);
    if let Some(mut stdout) = child.stdout.take() {
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => robot
                    .write_all(&buf[..n])
                    .map_err(|source| AgentError::Io { program: program.clone(), source })?,
                Err(source) => return Err(AgentError::Io { program: program.clone(), source }),
            }
        }
    }
    robot
        .finish()
        .map_err(|source| AgentError::Io { program: program.clone(), source })?;
    let produced = robot.wrote_anything();

    if let Some(h) = writer {
        let _ = h.join();
    }
    let status = child
        .wait()
        .map_err(|source| AgentError::Io { program: program.clone(), source })?;

    // Review Focus 5: a silent, successful agent is reported, not mistaken
    // for a good answer.
    if status.success() && !produced {
        eprintln!("ama: `{program}` exited without producing any output.");
        return Ok(1);
    }
    Ok(status.code().unwrap_or(1))
}
```

- [ ] **Step 8: Add the silence and streaming tests (Review Focus 5)**

Append to `crates/ama/tests/integration_agent.rs`:

```rust
#[test]
fn a_silent_agent_is_reported_rather_than_looking_like_success() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-silent"), "x", &mut out).expect("run");
    assert_eq!(code, 1, "silence is not success");
    assert!(out.is_empty(), "no bare robot prefix: {:?}", String::from_utf8_lossy(&out));
}

#[test]
fn output_is_streamed_not_buffered_until_exit() {
    use std::time::Instant;
    // fake-agent-slow prints, sleeps 3s, prints again. If output were buffered
    // until exit, nothing would be readable before the sleep completes.
    let (tx, rx) = std::sync::mpsc::channel::<std::time::Duration>();
    let started = Instant::now();
    std::thread::spawn(move || {
        struct Probe(std::sync::mpsc::Sender<std::time::Duration>, Instant, bool);
        impl std::io::Write for Probe {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if !self.2 {
                    self.2 = true;
                    let _ = self.0.send(self.1.elapsed());
                }
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let mut probe = Probe(tx, started, false);
        let _ = agent::run(&spec("fake-agent-slow"), "x", &mut probe);
    });
    let first = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("first byte must arrive before the agent exits");
    assert!(first < std::time::Duration::from_secs(2), "first byte took {first:?}");
}
```

- [ ] **Step 9: Run the agent suite**

Run: `cargo test --test integration_agent --test unit_render`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/ama
git commit -m "feat: stream agent output with the robot prefix; report silent agents"
```

---

### Task 6: Wire the CLI so `@@` answers a question

After this task the tool works end to end with a real agent; only the Enter hook is missing.

**Files:**
- Create: `crates/ama/src/cli.rs`
- Modify: `crates/ama/src/lib.rs`, `crates/ama/src/main.rs`
- Test: `crates/ama/tests/integration_cli.rs`

**Interfaces:**
- Consumes: everything from Tasks 2–5.
- Produces: `cli::run() -> std::process::ExitCode`; the `ama` binary supporting
  `ask`, `init <shell>`, `session reset`, `doctor`, and `argv[0] == "@@"` dispatch.

- [ ] **Step 1: Write the failing CLI tests**

`crates/ama/tests/integration_cli.rs`:

```rust
use assert_cmd::Command;
use std::io::Write;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// A config pointing at a fake agent, plus an isolated HOME so the real
/// ~/.qmx2 is never touched by tests.
fn env() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config.yml");
    let mut f = std::fs::File::create(&cfg).expect("create");
    writeln!(f, "agent:\n  command: [{}]", fixture("fake-agent-echo")).expect("write");
    (dir, cfg)
}

fn ama(cfg: &std::path::Path, home: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("ama").expect("binary");
    c.env("AMA_CONFIG", cfg).env("HOME", home).env_remove("TMUX").env_remove("STY");
    c
}

#[test]
fn ask_answers_with_the_robot_prefix() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--", "what is this"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("🤖: "));
}

#[test]
fn the_question_reaches_the_agent() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--", "what's this project all about?"])
        .assert()
        .success()
        .stdout(predicates::str::contains("what's this project all about?"));
}

#[test]
fn no_context_sends_the_question_alone() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--no-context", "--", "bare"])
        .assert()
        .success()
        .stdout(predicates::str::contains("## Terminal").not());
}

#[test]
fn a_second_turn_carries_the_first_in_its_context() {
    let (d, cfg) = env();
    ama(&cfg, d.path()).args(["ask", "--", "first question"]).assert().success();
    ama(&cfg, d.path())
        .args(["ask", "--", "second question"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first question"));
}

#[test]
fn session_reset_drops_the_conversation() {
    let (d, cfg) = env();
    ama(&cfg, d.path()).args(["ask", "--", "first question"]).assert().success();
    ama(&cfg, d.path()).args(["session", "reset"]).assert().success();
    ama(&cfg, d.path())
        .args(["ask", "--", "second question"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first question").not());
}

#[test]
fn the_transcript_is_readable_only_by_its_owner() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let (d, cfg) = env();
        ama(&cfg, d.path()).args(["ask", "--", "q"]).assert().success();
        let sessions = d.path().join(".qmx2/sessions");
        let entry = std::fs::read_dir(&sessions)
            .expect("sessions dir")
            .next()
            .expect("one transcript")
            .expect("entry");
        let mode = entry.metadata().expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "transcript must be 0600, got {:o}", mode & 0o777);
    }
}

#[test]
fn a_missing_config_exits_two_and_says_where_to_put_it() {
    let d = tempfile::tempdir().expect("tempdir");
    let missing = d.path().join("nope.yml");
    Command::cargo_bin("ama")
        .expect("binary")
        .env("AMA_CONFIG", &missing)
        .env("HOME", d.path())
        .args(["ask", "--", "x"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("nope.yml"));
}

#[test]
fn a_failing_agent_exits_one() {
    let d = tempfile::tempdir().expect("tempdir");
    let cfg = d.path().join("config.yml");
    std::fs::write(&cfg, format!("agent:\n  command: [{}]\n", fixture("fake-agent-fail")))
        .expect("write");
    ama(&cfg, d.path()).args(["ask", "--", "x"]).assert().code(7);
}

#[test]
fn an_empty_question_is_a_silent_no_op() {
    let (d, cfg) = env();
    ama(&cfg, d.path()).args(["ask", "--", "   "]).assert().success().stdout("");
}

#[test]
fn the_at_at_name_is_an_alias_for_ask() {
    let (d, cfg) = env();
    let bin = assert_cmd::cargo::cargo_bin("ama");
    let alias = d.path().join("@@");
    std::fs::copy(&bin, &alias).expect("copy");
    Command::new(&alias)
        .env("AMA_CONFIG", &cfg)
        .env("HOME", d.path())
        .env_remove("TMUX")
        .arg("hello from the alias")
        .assert()
        .success()
        .stdout(predicates::str::contains("hello from the alias"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --test integration_cli`
Expected: FAIL — `ama` has no subcommands yet.

- [ ] **Step 3: Implement the CLI**

Add `pub mod cli;` to `crates/ama/src/lib.rs`, then `crates/ama/src/cli.rs`:

```rust
//! Argument parsing, dispatch, and the exit-code contract (NFR-05).

use clap::{Parser, Subcommand};
use std::io::Write;
use std::process::ExitCode;

use crate::{agent, config, context, prompt, session, shellinit};

const EXIT_OK: u8 = 0;
const EXIT_AGENT: u8 = 1;
const EXIT_CONFIG: u8 = 2;

#[derive(Parser)]
#[command(name = "ama", version, about = "Ask your own agent, from the shell")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Ask a question.
    Ask {
        /// Send the question without any terminal context.
        #[arg(long)]
        no_context: bool,
        /// The question. Everything after `--` is taken literally.
        #[arg(trailing_var_arg = true)]
        question: Vec<String>,
    },
    /// Print shell integration for `eval`.
    Init { shell: String },
    /// Manage the current terminal's conversation.
    Session {
        #[command(subcommand)]
        action: SessionCmd,
    },
    /// Report configuration and integration state.
    Doctor,
}

#[derive(Subcommand)]
enum SessionCmd {
    /// Forget this terminal's conversation.
    Reset,
}

pub fn run() -> ExitCode {
    // ADR-004: invoked as `@@`, every argument is the question.
    let argv: Vec<String> = std::env::args().collect();
    let invoked_as_trigger = argv
        .first()
        .map(|p| p.rsplit('/').next().unwrap_or(p))
        .is_some_and(|n| n == "@@");

    if invoked_as_trigger {
        return ask(&argv[1..].join(" "), false);
    }

    match Cli::parse().command {
        Cmd::Ask { no_context, question } => ask(&question.join(" "), no_context),
        Cmd::Init { shell } => init(&shell),
        Cmd::Session { action: SessionCmd::Reset } => {
            match session::reset(&session::current_key()) {
                Ok(()) => ExitCode::from(EXIT_OK),
                Err(e) => fail(EXIT_AGENT, &format!("could not reset session: {e}")),
            }
        }
        Cmd::Doctor => doctor(),
    }
}

fn fail(code: u8, msg: &str) -> ExitCode {
    eprintln!("ama: {msg}");
    ExitCode::from(code)
}

fn ask(question: &str, no_context: bool) -> ExitCode {
    let question = question.trim();
    if question.is_empty() {
        return ExitCode::from(EXIT_OK); // REQ-06
    }

    let path = config::config_path();
    let cfg = match config::Config::load(&path) {
        Ok(c) => c,
        Err(e) => return fail(EXIT_CONFIG, &e.to_string()),
    };
    let spec = match cfg.agent_spec() {
        Ok(s) => s,
        Err(e) => return fail(EXIT_CONFIG, &e.to_string()),
    };

    let key = session::current_key();
    let ctx = if no_context {
        Vec::new()
    } else {
        context::gather(context::detect_source(), &key, cfg.max_context_lines)
    };
    let composed = prompt::compose(&ctx, question);

    // Tee the answer so it can be recorded for the transcript fallback.
    let mut captured: Vec<u8> = Vec::new();
    let mut sink = Tee { a: std::io::stdout(), b: &mut captured };

    match agent::run(&spec, &composed, &mut sink) {
        Ok(0) => {
            let answer = String::from_utf8_lossy(&captured).into_owned();
            let _ = session::append_turn(
                &key,
                &session::Turn { question: question.to_string(), answer: strip_render(&answer) },
            );
            ExitCode::from(EXIT_OK)
        }
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(EXIT_AGENT)),
        Err(e) => fail(EXIT_CONFIG, &e.to_string()),
    }
}

/// Recover the agent's own words from rendered output, so the transcript
/// stores the answer rather than its decoration.
fn strip_render(rendered: &str) -> String {
    rendered
        .lines()
        .map(|l| {
            l.strip_prefix(context::ANSWER_PREFIX)
                .or_else(|| l.strip_prefix(context::CONT_INDENT))
                .unwrap_or(l)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct Tee<A: Write, B: Write> {
    a: A,
    b: B,
}

impl<A: Write, B: Write> Write for Tee<A, B> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.a.write_all(buf)?;
        self.b.write_all(buf)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.a.flush()?;
        self.b.flush()
    }
}

fn init(shell: &str) -> ExitCode {
    match shellinit::Shell::parse(shell) {
        Some(s) => {
            print!("{}", shellinit::script(s));
            ExitCode::from(EXIT_OK)
        }
        None => fail(EXIT_CONFIG, &format!("unknown shell `{shell}`; expected bash or zsh")),
    }
}

fn doctor() -> ExitCode {
    let path = config::config_path();
    println!("config       {}", path.display());
    let key = session::current_key();
    println!("session      {key}");
    println!("context      {}", context::detect_source());
    println!("transcript   {}", session::transcript_path(&key).display());

    match config::Config::load(&path).and_then(|c| c.agent_spec()) {
        Ok(spec) => {
            println!("agent        {}", spec.argv.join(" "));
            let program = spec.argv.first().cloned().unwrap_or_default();
            if which(&program) {
                println!("status       ok");
                ExitCode::from(EXIT_OK)
            } else {
                fail(EXIT_CONFIG, &format!("`{program}` is not on PATH"))
            }
        }
        Err(e) => fail(EXIT_CONFIG, &e.to_string()),
    }
}

fn which(program: &str) -> bool {
    if program.contains('/') {
        return std::path::Path::new(program).is_file();
    }
    std::env::var("PATH").is_ok_and(|paths| {
        std::env::split_paths(&paths).any(|d| d.join(program).is_file())
    })
}
```

Replace `crates/ama/src/main.rs`:

```rust
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    ama::cli::run()
}
```

- [ ] **Step 4: Add a placeholder `shellinit` so the crate compiles**

`crates/ama/src/shellinit.rs` (completed in Task 7):

```rust
//! Shell integration scripts, embedded in the binary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
}

impl Shell {
    pub fn parse(s: &str) -> Option<Self> {
        match s.rsplit('/').next().unwrap_or(s) {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            _ => None,
        }
    }
}

pub fn script(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => include_str!("shell/ama.bash"),
        Shell::Zsh => include_str!("shell/ama.zsh"),
    }
}
```

Create an empty `crates/ama/src/shell/ama.zsh` for now (Task 8 fills it), and add
`pub mod shellinit;` to `crates/ama/src/lib.rs`.

- [ ] **Step 5: Run the CLI suite**

Run: `cargo test --test integration_cli`
Expected: PASS. This also covers `config_path()` honouring `AMA_CONFIG`, which the
child process receives as an environment variable.

- [ ] **Step 6: Measure the overhead budget (NFR-01)**

Run:

```bash
cat > /tmp/ama-nfr01.yml <<'EOF'
agent:
  command: [/bin/true]
EOF
time (for i in $(seq 20); do AMA_CONFIG=/tmp/ama-nfr01.yml ./target/debug/ama ask -- ping >/dev/null; done)
```

Expected: under 1.0 s total in a debug build (50 ms per invocation). Record the number
in `06-build-record.md`. If it exceeds the budget, profile before continuing — the
common cause is a `tmux capture-pane` call in an environment where tmux is slow.

- [ ] **Step 7: Commit**

```bash
git add crates/ama
git commit -m "feat: ama ask, session reset, doctor, and @@ alias dispatch"
```

---

### Task 7: The bash hook, proved in a real terminal

**Files:**
- Modify: `crates/ama/src/shell/ama.bash`
- Test: `crates/ama/tests/e2e_shell.rs`

**Interfaces:**
- Consumes: `__ama_sq` and `__ama_split` from Task 1; the `ama` and `@@` binaries.
- Produces: a sourceable bash integration; `ama init bash` emits it.

- [ ] **Step 1: Append the hook to `crates/ama/src/shell/ama.bash`**

```bash
# ---- interactive integration ------------------------------------------------
# Nothing to do in a non-interactive shell (REQ-24).
case $- in *i*) ;; *) return 0 2>/dev/null || exit 0 ;; esac

# A stable per-shell identity for the transcript fallback.
export AMA_SESSION="$$"

__ama_clears_screen() {
    case $1 in
        clear | reset | 'clear '* | 'reset '* | *'tput clear'*) return 0 ;;
        *) return 1 ;;
    esac
}

__ama_hook() {
    local split prefix prompt
    if split=$(__ama_split "$READLINE_LINE"); then
        prefix=${split%%$'\x1f'*}
        prompt=${split#*$'\x1f'}
        # A blank prompt is a no-op, not an agent call (REQ-06).
        if [[ -z ${prompt//[[:space:]]/} ]]; then
            READLINE_LINE=""
            READLINE_POINT=0
            return 0
        fi
        # `clear && @@ ...`: drop the transcript now, since the pane-scrape
        # path resets itself but the fallback path has no other signal.
        __ama_clears_screen "$prefix" && ama session reset >/dev/null 2>&1
        READLINE_LINE="${prefix}@@ $(__ama_sq "$prompt")"
        READLINE_POINT=${#READLINE_LINE}
        return 0
    fi
    __ama_clears_screen "$READLINE_LINE" && ama session reset >/dev/null 2>&1
    return 0
}

__ama_clear_screen_widget() {
    ama session reset >/dev/null 2>&1
    READLINE_LINE=""
    READLINE_POINT=0
    printf '\033[H\033[2J'
}

# Chain to whatever already owns Enter (RISK-03) instead of clobbering it.
__ama_install() {
    local existing
    existing=$(bind -s 2>/dev/null | sed -n 's/^"\\C-m": "\(.*\)"$/\1/p' | head -n1)
    bind -x '"\C-x\C-aq": __ama_hook' 2>/dev/null || return 0
    if [[ -n $existing && $existing != *'\C-x\C-aq'* ]]; then
        bind "\"\\C-m\": \"\\C-x\\C-aq${existing}\"" 2>/dev/null
    else
        # \C-j is also accept-line and is left unbound, so the macro terminates.
        bind '"\C-m": "\C-x\C-aq\C-j"' 2>/dev/null
    fi
    bind -x '"\C-l": __ama_clear_screen_widget' 2>/dev/null
}

__ama_install
```

- [ ] **Step 2: Verify the script is syntactically valid**

Run: `bash -n crates/ama/src/shell/ama.bash`
Expected: no output.

- [ ] **Step 3: Write the tmux end-to-end harness**

`crates/ama/tests/e2e_shell.rs`:

```rust
//! End-to-end tests that drive a real interactive shell inside tmux and read
//! the pane back. This is the only layer that can catch the class of defect
//! that eliminated design approach (A) -- see 02-discovery-and-risk.md F-03.

use std::process::Command;
use std::time::Duration;

fn have(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null")])
        .status()
        .is_ok_and(|s| s.success())
}

pub struct Pane {
    name: String,
    _dir: tempfile::TempDir,
}

impl Pane {
    /// Start `shell` interactively in a detached tmux session with the
    /// integration loaded and a fake agent configured.
    fn start(name: &str, shell: &str) -> Pane {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let bin_dir = home.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let ama = assert_cmd::cargo::cargo_bin("ama");
        std::fs::copy(&ama, bin_dir.join("ama")).expect("copy ama");
        std::fs::copy(&ama, bin_dir.join("@@")).expect("copy @@");

        let fixtures = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(
            home.join("config.yml"),
            format!("agent:\n  command: [{fixtures}/fake-agent-echo]\n"),
        )
        .expect("config");

        let rc = home.join("rc");
        let init = Command::new(&ama)
            .args(["init", shell])
            .output()
            .expect("ama init");
        let body = format!(
            "PS1='T$ '\nexport PATH={bin}:$PATH\nexport AMA_CONFIG={home}/config.yml\n\
             export HOME={home}\n{script}",
            bin = bin_dir.display(),
            home = home.display(),
            script = String::from_utf8_lossy(&init.stdout),
        );
        std::fs::write(&rc, body).expect("rc");

        let cmd = match shell {
            "zsh" => format!("ZDOTDIR={} zsh -i", home.display()),
            _ => format!("bash --rcfile {} -i", rc.display()),
        };
        if shell == "zsh" {
            std::fs::copy(&rc, home.join(".zshrc")).expect("zshrc");
        }

        let _ = Command::new("tmux").args(["kill-session", "-t", name]).status();
        Command::new("tmux")
            .args(["new-session", "-d", "-s", name, "-x", "100", "-y", "24", &cmd])
            .status()
            .expect("tmux new-session");
        std::thread::sleep(Duration::from_millis(900));
        Pane { name: name.to_string(), _dir: dir }
    }

    fn send(&self, keys: &str) {
        Command::new("tmux")
            .args(["send-keys", "-t", &self.name, keys, "Enter"])
            .status()
            .expect("send-keys");
    }

    fn send_raw(&self, keys: &str) {
        Command::new("tmux")
            .args(["send-keys", "-t", &self.name, keys])
            .status()
            .expect("send-keys");
    }

    /// Poll the pane until `needle` appears or the timeout expires.
    fn wait_for(&self, needle: &str, timeout: Duration) -> String {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let text = self.capture();
            if text.contains(needle) {
                return text;
            }
            if std::time::Instant::now() > deadline {
                panic!("timed out waiting for {needle:?}\n--- pane ---\n{text}");
            }
            std::thread::sleep(Duration::from_millis(120));
        }
    }

    fn capture(&self) -> String {
        let out = Command::new("tmux")
            .args(["capture-pane", "-t", &self.name, "-p"])
            .output()
            .expect("capture-pane");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        let _ = Command::new("tmux").args(["kill-session", "-t", &self.name]).status();
    }
}

macro_rules! require_tmux {
    () => {
        if !have("tmux") {
            eprintln!("skipping: tmux not installed");
            return;
        }
    };
}
```

- [ ] **Step 4: Write the failing bash end-to-end tests**

Append to `crates/ama/tests/e2e_shell.rs`:

```rust
#[test]
fn the_readme_first_example_works_verbatim() {
    require_tmux!();
    let p = Pane::start("ama-e2e-readme", "bash");
    p.send("@@ what's this project all about?");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(
        pane.contains("what's this project all about?"),
        "the apostrophe must survive the round trip\n{pane}"
    );
}

#[test]
fn shell_metacharacters_reach_the_agent_untouched() {
    require_tmux!();
    let p = Pane::start("ama-e2e-meta", "bash");
    p.send("@@ list *.rs and $HOME | grep foo && done?");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(pane.contains("list *.rs and $HOME | grep foo && done?"), "{pane}");
}

#[test]
fn the_question_stays_on_screen_and_in_history() {
    require_tmux!();
    let p = Pane::start("ama-e2e-history", "bash");
    p.send("@@ remember me");
    p.wait_for("🤖:", Duration::from_secs(10));
    p.send("history 3");
    let pane = p.wait_for("remember me", Duration::from_secs(5));
    assert!(pane.matches("remember me").count() >= 2, "question must persist\n{pane}");
}

#[test]
fn ordinary_commands_are_unaffected() {
    require_tmux!();
    let p = Pane::start("ama-e2e-passthrough", "bash");
    p.send("echo NORMAL-OK");
    p.wait_for("NORMAL-OK", Duration::from_secs(5));
    p.send("for i in 1 2; do echo L$i; done");
    let pane = p.wait_for("L2", Duration::from_secs(5));
    assert!(pane.contains("L1") && pane.contains("L2"), "{pane}");
}

#[test]
fn clear_and_trigger_on_one_line_starts_a_fresh_conversation() {
    require_tmux!();
    let p = Pane::start("ama-e2e-clear", "bash");
    p.send("@@ first question");
    p.wait_for("🤖:", Duration::from_secs(10));
    p.send("clear && @@ show me the joke of the day");
    let pane = p.wait_for("joke of the day", Duration::from_secs(10));
    assert!(
        !pane.contains("first question"),
        "clear must drop the earlier turn (REQ-13, REQ-28)\n{pane}"
    );
}

#[test]
fn terminal_output_from_other_commands_becomes_context() {
    require_tmux!();
    let p = Pane::start("ama-e2e-context", "bash");
    p.send("@@ start the conversation");
    p.wait_for("🤖:", Duration::from_secs(10));
    p.send("ls /nonexistent-marker-xyz");
    p.wait_for("nonexistent-marker-xyz", Duration::from_secs(5));
    p.send("@@ why did that fail");
    let pane = p.wait_for("why did that fail", Duration::from_secs(10));
    assert!(
        pane.contains("No such file") || pane.contains("cannot access"),
        "the failure output must be visible as context\n{pane}"
    );
}

#[test]
fn an_interrupt_returns_a_usable_shell() {
    require_tmux!();
    let p = Pane::start("ama-e2e-interrupt", "bash");
    p.send("@@ slow question");
    std::thread::sleep(Duration::from_millis(600));
    p.send_raw("C-c");
    std::thread::sleep(Duration::from_millis(400));
    p.send("echo SHELL-ALIVE");
    p.wait_for("SHELL-ALIVE", Duration::from_secs(5));
}
```

- [ ] **Step 5: Run the end-to-end tests**

Run: `cargo test --test e2e_shell -- --test-threads=1`
Expected: PASS. Run single-threaded; each test owns a named tmux session and they must
not race. If `clear_and_trigger_on_one_line...` fails, check that `__ama_split` returns
`clear && ` as the prefix.

- [ ] **Step 6: Write the failing integration tests for `ama init`**

Append to `crates/ama/tests/integration_cli.rs`:

```rust
#[test]
fn init_bash_emits_a_syntactically_valid_script() {
    let out = Command::cargo_bin("ama").expect("bin").args(["init", "bash"]).output().expect("run");
    assert!(out.status.success());
    let script = String::from_utf8_lossy(&out.stdout);
    assert!(script.contains("__ama_hook"), "hook missing");
    let mut check = Command::new("bash");
    check.args(["-n", "/dev/stdin"]).write_stdin(script.as_bytes().to_vec());
    check.assert().success();
}

#[test]
fn init_rejects_an_unknown_shell() {
    Command::cargo_bin("ama")
        .expect("bin")
        .args(["init", "fish"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("bash"));
}

#[test]
fn sourcing_the_script_in_a_non_interactive_shell_is_a_silent_success() {
    let out = Command::cargo_bin("ama").expect("bin").args(["init", "bash"]).output().expect("run");
    let script = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut c = Command::new("bash");
    c.arg("-c").arg(format!("{script}\nexit 0"));
    c.assert().success();
}
```

- [ ] **Step 7: Run and fix until green**

Run: `cargo test --test integration_cli`
Expected: PASS.

- [ ] **Step 8: Add the trigger-precision end-to-end tests (Review Focus 1)**

Append to `crates/ama/tests/e2e_shell.rs`:

```rust
#[test]
fn a_bare_trigger_does_not_call_the_agent() {
    require_tmux!();
    let p = Pane::start("ama-e2e-bare", "bash");
    p.send("@@ ");
    std::thread::sleep(Duration::from_millis(700));
    assert!(!p.capture().contains("🤖:"), "empty prompt must be a no-op (REQ-06)");
}

#[test]
fn a_trigger_that_is_not_at_a_command_position_is_left_alone() {
    require_tmux!();
    let p = Pane::start("ama-e2e-notrigger", "bash");
    p.send("echo @@ literal");
    let pane = p.wait_for("literal", Duration::from_secs(5));
    assert!(!pane.contains("🤖:"), "must not hijack a real command\n{pane}");
}
```

- [ ] **Step 9: Add the paste and precision tests (Review Focus 1 and 4)**

Append to `crates/ama/tests/e2e_shell.rs`:

```rust
#[test]
fn a_trigger_without_a_following_space_is_not_a_trigger() {
    require_tmux!();
    let p = Pane::start("ama-e2e-nospace", "bash");
    p.send("@@nosuchcommand");
    let pane = p.wait_for("command not found", Duration::from_secs(5));
    assert!(!pane.contains("🤖:"), "{pane}");
}

#[test]
fn leading_whitespace_still_triggers() {
    require_tmux!();
    let p = Pane::start("ama-e2e-indent", "bash");
    p.send("   @@ indented question");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(pane.contains("indented question"), "{pane}");
}
```

- [ ] **Step 10: Run everything and commit**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

```bash
git add crates/ama
git commit -m "feat: bash Enter hook with chaining, plus tmux end-to-end suite"
```

---

### Task 8: The zsh widget

**Files:**
- Modify: `crates/ama/src/shell/ama.zsh`
- Test: `crates/ama/tests/shell_functions.rs`, `crates/ama/tests/e2e_shell.rs`

**Interfaces:**
- Consumes: the same contract as Task 1 — `__ama_sq`, `__ama_split`.
- Produces: a sourceable zsh integration; `ama init zsh` emits it.

- [ ] **Step 1: Write the zsh integration**

`crates/ama/src/shell/ama.zsh`:

```zsh
# ama shell integration (zsh). Sourced via: eval "$(ama init zsh)"

__ama_sq() {
    printf "'%s'" "${1//\'/\'\\\'\'}"
}

__ama_split() {
    local line=$1 lead trimmed
    lead=${line%%[^[:space:]]*}
    trimmed=${line#$lead}

    if [[ $trimmed == '@@ '* ]]; then
        printf '%s\x1f%s' "$lead" "${trimmed#'@@ '}"
        return 0
    fi
    if [[ $line =~ '^(.*[;&|][[:space:]]*)@@ (.*)$' ]]; then
        printf '%s\x1f%s' "${match[1]}" "${match[2]}"
        return 0
    fi
    return 1
}

[[ -o interactive ]] || return 0
export AMA_SESSION="$$"

__ama_clears_screen() {
    case $1 in
        clear | reset | 'clear '* | 'reset '* | *'tput clear'*) return 0 ;;
        *) return 1 ;;
    esac
}

__ama_accept_line() {
    local split prefix prompt
    if split=$(__ama_split "$BUFFER"); then
        prefix=${split%%$'\x1f'*}
        prompt=${split#*$'\x1f'}
        if [[ -z ${prompt//[[:space:]]/} ]]; then
            BUFFER=""
            zle .accept-line
            return
        fi
        __ama_clears_screen "$prefix" && ama session reset >/dev/null 2>&1
        BUFFER="${prefix}@@ $(__ama_sq "$prompt")"
        CURSOR=${#BUFFER}
    else
        __ama_clears_screen "$BUFFER" && ama session reset >/dev/null 2>&1
    fi
    zle .accept-line
}

__ama_clear_screen() {
    ama session reset >/dev/null 2>&1
    zle .clear-screen
}

zle -N accept-line __ama_accept_line
zle -N clear-screen __ama_clear_screen
```

- [ ] **Step 2: Verify the script parses**

Run: `zsh -n crates/ama/src/shell/ama.zsh` (skip if zsh is not installed)
Expected: no output.

- [ ] **Step 3: Extend the property test to zsh**

Modify `crates/ama/tests/shell_functions.rs` — parameterise `roundtrip` and `split`
over the shell, defaulting to bash, and add:

```rust
const ZSH_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shell/ama.zsh");

fn zsh_available() -> bool {
    Command::new("sh").args(["-c", "command -v zsh >/dev/null"]).status().is_ok_and(|s| s.success())
}

fn roundtrip_in(shell: &str, src: &str, s: &str) -> String {
    let script = format!(
        r#"source '{src}'
           q=$(__ama_sq "$1")
           eval "printf '%s' $q""#
    );
    let out = Command::new(shell)
        .args(["-c", &script, "ama-test", s])
        .output()
        .expect("spawn shell");
    assert!(out.status.success(), "{shell} failed: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn zsh_quoting_round_trips_the_same_cases() {
    if !zsh_available() {
        eprintln!("skipping: zsh not installed");
        return;
    }
    for s in [
        "what's this project all about?",
        "list *.rs files | grep foo && echo $HOME",
        "$(whoami) `id` ${PATH}",
        "emoji 🤖 and CJK 日本語",
        "newline\nin\nthe\nmiddle",
    ] {
        assert_eq!(roundtrip_in("zsh", ZSH_SRC, s), s, "zsh failed on {s:?}");
    }
}
```

Refactor the existing bash `roundtrip` to call `roundtrip_in("bash", BASH_SRC, s)` so
there is one implementation.

- [ ] **Step 4: Run the shell suite**

Run: `cargo test --test shell_functions`
Expected: PASS, or a printed skip when zsh is absent.

- [ ] **Step 5: Add the zsh end-to-end test**

Append to `crates/ama/tests/e2e_shell.rs`:

```rust
#[test]
fn the_readme_first_example_works_in_zsh_too() {
    require_tmux!();
    if !have("zsh") {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let p = Pane::start("ama-e2e-zsh", "zsh");
    p.send("@@ what's this project all about?");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(pane.contains("what's this project all about?"), "{pane}");
}
```

- [ ] **Step 6: Run and commit**

Run: `cargo test -- --test-threads=1`
Expected: PASS.

```bash
git add crates/ama
git commit -m "feat: zsh ZLE widget integration"
```

---

### Task 9: Installation and `doctor` completeness

**Files:**
- Create: `crates/ama/src/../../install.sh` → `install.sh` at repository root
- Modify: `crates/ama/src/cli.rs`
- Test: `crates/ama/tests/integration_cli.rs`

**Interfaces:**
- Consumes: `cli::doctor`.
- Produces: `install.sh`; `doctor` output naming every field in REQ-25.

- [ ] **Step 1: Write the failing doctor test**

Append to `crates/ama/tests/integration_cli.rs`:

```rust
#[test]
fn doctor_reports_every_field_and_succeeds_when_healthy() {
    let (d, cfg) = env();
    let out = ama(&cfg, d.path()).arg("doctor").output().expect("run");
    let text = String::from_utf8_lossy(&out.stdout);
    for field in ["config", "session", "context", "transcript", "agent", "status"] {
        assert!(text.contains(field), "doctor must report `{field}`:\n{text}");
    }
    assert!(out.status.success());
}

#[test]
fn doctor_fails_when_the_agent_is_not_installed() {
    let d = tempfile::tempdir().expect("tempdir");
    let cfg = d.path().join("config.yml");
    std::fs::write(&cfg, "agent: definitely-not-installed\n").expect("write");
    ama(&cfg, d.path())
        .arg("doctor")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("definitely-not-installed"));
}

#[test]
fn doctor_names_the_context_source_it_would_use() {
    let (d, cfg) = env();
    let out = ama(&cfg, d.path()).arg("doctor").output().expect("run");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("transcript"), "outside tmux the source must be named:\n{text}");
}
```

- [ ] **Step 2: Run to verify it fails or passes**

Run: `cargo test --test integration_cli`
Expected: the three tests pass with the Task 6 implementation. If
`doctor_names_the_context_source_it_would_use` fails, `Source`'s `Display` text is
wrong — it must contain the word `transcript`.

- [ ] **Step 3: Write the installer**

`install.sh` at the repository root:

```bash
#!/usr/bin/env bash
# Build and install ama, plus the `@@` trigger, into ~/.local/bin.
set -euo pipefail

cd "$(dirname "$0")"
dest="${AMA_PREFIX:-$HOME/.local/bin}"

cargo build --release
mkdir -p "$dest"
install -m 0755 target/release/ama "$dest/ama"
install -m 0755 target/release/ama "$dest/@@"

shell_name=$(basename "${SHELL:-bash}")
case "$shell_name" in
    zsh) rc="$HOME/.zshrc" ;;
    *) rc="$HOME/.bashrc"; shell_name=bash ;;
esac

line="eval \"\$($dest/ama init $shell_name)\""
if ! grep -qF "ama init $shell_name" "$rc" 2>/dev/null; then
    printf '\n# ama -- ask me anything\n%s\n' "$line" >> "$rc"
    echo "Added integration to $rc"
else
    echo "Integration already present in $rc"
fi

mkdir -p "$HOME/.qmx2"
if [[ ! -f "$HOME/.qmx2/config.yml" ]]; then
    printf 'agent: claude --model opus --effort high\n' > "$HOME/.qmx2/config.yml"
    echo "Wrote a starter config to ~/.qmx2/config.yml"
fi

echo
echo "Installed. Start a new shell, then try:  @@ what is this project about"
"$dest/ama" doctor || true
```

Then: `chmod +x install.sh`

- [ ] **Step 4: Verify the installer is valid and idempotent**

Run:

```bash
bash -n install.sh
AMA_PREFIX=/tmp/ama-install-test HOME=/tmp/ama-install-home bash install.sh
AMA_PREFIX=/tmp/ama-install-test HOME=/tmp/ama-install-home bash install.sh
grep -c 'ama init' /tmp/ama-install-home/.bashrc
```

Expected: `bash -n` silent; second run prints "already present"; the grep prints `1`.

- [ ] **Step 5: Commit**

```bash
git add install.sh crates/ama
git commit -m "feat: installer and doctor coverage"
```

---

### Task 10: Feedback loop, documentation, and the build record

**Files:**
- Create: `check.sh`, `crates/ama/src/../../.github/workflows/ci.yml` → `.github/workflows/ci.yml`
- Modify: `README.md`, `.gitignore`
- Create: `0.1.0/06-build-record.md`

**Interfaces:**
- Consumes: everything.
- Produces: a single command that reproduces all evidence.

- [ ] **Step 1: Write `check.sh`**

```bash
#!/usr/bin/env bash
# Repository feedback loop. Run from the repository root:  ./check.sh
set -uo pipefail
cd "$(dirname "$0")"

BOLD=$'\033[1m'; GREEN=$'\033[32m'; RED=$'\033[31m'; DIM=$'\033[2m'; RESET=$'\033[0m'
fail=0

step() {
    local label=$1; shift
    printf '  %-34s' "$label"
    if output=$("$@" 2>&1); then
        printf '%sok%s\n' "$GREEN" "$RESET"
    else
        printf '%sFAILED%s\n' "$RED" "$RESET"
        printf '%s\n' "$output" | sed 's/^/      /'
        fail=1
    fi
}

printf '\n%s\n' "${BOLD}ama -- checks${RESET}"
printf '%s\n\n' "${DIM}$(rustc --version)${RESET}"

step "rustfmt"            cargo fmt --all -- --check
step "clippy"             cargo clippy --all-targets -- -D warnings \
                              -D clippy::unwrap_used -D clippy::expect_used
step "bash syntax"        bash -n crates/ama/src/shell/ama.bash
step "install.sh syntax"  bash -n install.sh
if command -v zsh >/dev/null; then
    step "zsh syntax"     zsh -n crates/ama/src/shell/ama.zsh
fi
if command -v shellcheck >/dev/null; then
    step "shellcheck"     shellcheck crates/ama/src/shell/ama.bash install.sh
fi
step "build"              cargo build --all-targets
step "tests"              cargo test -- --test-threads=1

printf '\n'
if [[ $fail -eq 0 ]]; then
    printf '  %sAll checks passed.%s\n\n' "$GREEN" "$RESET"
else
    printf '  %sSomething failed. See above.%s\n\n' "$RED" "$RESET"
fi
exit "$fail"
```

Then: `chmod +x check.sh`

- [ ] **Step 2: Allow clippy's lint gate in the crate**

Add to `crates/ama/Cargo.toml`:

```toml
[lints.clippy]
unwrap_used = "warn"
expect_used = "warn"
```

Tests may use them; `--all-targets` with `-D` would flag test code, so scope the deny in
`check.sh` to the library and binary only:

```bash
step "clippy"  cargo clippy --lib --bins -- -D warnings \
                   -D clippy::unwrap_used -D clippy::expect_used
step "clippy (tests)"  cargo clippy --tests -- -D warnings
```

Replace the single clippy line in Step 1 with these two.

- [ ] **Step 3: Run the full check**

Run: `./check.sh`
Expected: every step `ok`. Fix anything that is not before continuing.

- [ ] **Step 4: Add CI**

`.github/workflows/ci.yml`:

```yaml
name: ci
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: Install tmux, zsh, shellcheck
        run: sudo apt-get update && sudo apt-get install -y tmux zsh shellcheck
      - run: ./check.sh
```

- [ ] **Step 5: Correct the root README**

Apply exactly these changes to `README.md`:

1. Title and prose: replace every `aka` with `ama`.
2. Rewrite each example's typed line to show what actually executes. Example 1 becomes:

```
user@host:~/project-x$ @@ what's this project all about?
🤖: This is a toy project that builds a quantum core to find ALL analytical solutions for Navier-Stokes Equation under any given boundary conditions!
```

with a note directly beneath the examples:

```
> The shell integration rewrites your line into a correctly quoted command before
> running it, so `@@ what's this project all about?` executes as
> `@@ 'what'\''s this project all about?'`. Your question is passed through
> byte-for-byte: no globbing, no variable expansion, no quote handling.
```

3. Add an **Install** section:

```markdown
## install

    git clone <this repo> && cd question-mark-x2 && ./install.sh

Then open a new shell. `ama doctor` reports what it found.
```

4. Add to the config section, below the existing YAML block:

```markdown
`ama` adds the one-shot flag a known agent needs, so the line above runs as
`claude -p --model opus --effort high`. For full control, give an explicit command:

    agent:
      command: [my-bot, --ask, "{prompt}"]

    max_context_lines: 200
```

5. Add a **What it sends** section, stating RISK-01 plainly:

```markdown
## what it sends

Inside tmux or screen, `ama` sends your agent the visible pane from the first `@@`
line down -- including the output of other commands, which is what lets it answer
"why did that build fail?". Outside a multiplexer it sends only its own previous
questions and answers.

It does not filter that content. If a secret is visible on your screen, it goes to
your agent. Clear the screen, or use `@@ --no-context`, before asking about anything
sensitive. `ama` never reads or forwards your agent's credentials.
```

- [ ] **Step 6: Update `.gitignore`**

The existing file already ignores `target` and `learn/`. Add:

```
# ama session state, if ever created inside the repo
.qmx2/
```

- [ ] **Step 7: Write the build record**

Create `0.1.0/06-build-record.md` with the SDLC frontmatter (`stage: S4-build`,
`status: complete`), and record: the slices implemented with their commit hashes, the
measured NFR-01 number from Task 6 Step 6, the `./check.sh` output, any deviation from
this plan, and the amendments A-01 and A-02 already recorded in S2.

- [ ] **Step 8: Run the whole loop once more and commit**

Run: `./check.sh`
Expected: all `ok`.

```bash
git add -A
git commit -m "chore: feedback loop, CI, corrected README, and the S4 build record"
```

---

## Self-Review

**Spec coverage.** Every requirement maps to a task:

| Requirement | Task |
|---|---|
| REQ-01, REQ-02, REQ-03, REQ-04, REQ-05 | 7 (e2e), 1 (escape) |
| REQ-06 | 7 Step 8 |
| REQ-07, REQ-08 | 5 |
| REQ-09, REQ-10 | 5 |
| REQ-11, REQ-13 | 7 Steps 4–5 |
| REQ-12, REQ-14, REQ-15 | 4, 6 |
| REQ-16 | 2 Step 9, 6 Step 5 |
| REQ-17, REQ-18 | 2 Steps 1–5 |
| REQ-19 | 2 Step 6, 5 Step 6 |
| REQ-20 | 2 Step 6, 6 Step 1 |
| REQ-21 | 2 Step 9 (no credential path exists), reviewed in S5 |
| REQ-22, REQ-23, REQ-24 | 7 Step 6 |
| REQ-25 | 9 |
| REQ-26, REQ-27 | 6 |
| REQ-28 | 1 Step 6, 7 Step 5 |
| NFR-01 | 6 Step 6 |
| NFR-02 | 4 Step 6 |
| NFR-03 | 6 (tests run with `TMUX` removed) |
| NFR-04 | 2 Step 6 |
| NFR-05 | 6 Step 1 |
| NFR-06 | 6 Step 1 |
| NFR-07 | 1 Step 1, 10 Step 2 |

**Placeholder scan.** No `TBD`/`TODO`. Task 2 Step 6 deliberately shows a test that
cannot work under edition 2024 and Step 7 deletes it; that is a trap being flagged, not
a placeholder, and the requirement it covered is picked up in Task 6 Step 5.

**Type consistency.** `AgentSpec`/`PromptVia` are defined in Task 2 and consumed
unchanged in Tasks 5 and 6. `SessionKey`/`Turn` from Task 3 are consumed in Tasks 4
and 6. `ANSWER_PREFIX`/`CONT_INDENT` are defined once in `context.rs` (Task 4) and used
by `render.rs` (Task 5) and `cli::strip_render` (Task 6), so slicing and rendering can
never disagree. `Shell::parse`/`script` from Task 6 Step 4 are filled by Tasks 7 and 8.

**Review Focus coverage.** All five have owning tests: 1 → Task 1 Step 6 and Task 7
Steps 8–9; 2 → Task 4 Step 2; 3 → Task 1 Step 4 and Task 4 Step 6; 4 → Task 1 Step 6
and Task 7 Step 9; 5 → Task 5 Step 8.

## SG3 — Plan approved

Pending owner review.
