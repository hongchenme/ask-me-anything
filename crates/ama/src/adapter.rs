//! Known-agent adapters (ADR-003, as amended by A-06 and ADR-007).
//!
//! An adapter may only *insert a missing token*. It never removes, reorders,
//! or rewrites a user's arguments, so a config that already works keeps
//! working when this table changes.
//!
//! There are two kinds of token. The first makes the agent run **one-shot**
//! at all; without it `ama` would hang against an interactive session:
//!
//! | Agent    | One-shot form        | Inserted when                          | Prompt via |
//! |----------|----------------------|----------------------------------------|------------|
//! | `claude` | `claude -p`          | neither `-p` nor `--print` present      | stdin |
//! | `codex`  | `codex exec`         | no `exec` token present                 | stdin |
//! | `ollama` | `ollama run`         | the first argument is not `run`         | stdin |
//! | `agy`    | `agy … -p {prompt}`  | no `-p`/`--print`/`--prompt`/`{prompt}` | argument |
//!
//! Two things this table encodes that are easy to get wrong, both established
//! by running the real tools rather than reading their help:
//!
//! - **`codex`'s one-shot mode is a subcommand, not a flag** — and `codex exec
//!   -p` means `--profile`, not `--print`. Inserting a flag there would
//!   silently change the invocation, which is why this inserts a *token*.
//! - **`agy`'s `-p` takes the prompt as its value** and does not read stdin;
//!   `agy -p` alone fails with `flag needs an argument: -p`. So its adapter
//!   also appends a `{prompt}` placeholder, which `Config::agent_spec` then
//!   resolves to `PromptVia::Arg`. It is appended rather than inserted after
//!   the program so a flag-value pair cannot be split by later arguments.
//!
//! The second kind grants the agent **authority to search the web**
//! (ADR-007), and is the whole of [`Tools`]:
//!
//! | Agent    | Grant                        | Inserted when                     |
//! |----------|------------------------------|-----------------------------------|
//! | `claude` | `--allowedTools WebSearch`   | no `--allowedTools`/`--allowed-tools` |
//! | `codex`  | `--search`                   | no `--search` present             |
//! | others   | nothing                      | — |
//!
//! This exists because a one-shot agent has nobody to answer a permission
//! prompt, so every tool needing approval is denied and the agent reports
//! itself incapable — `claude -p` answers a weather question with "I don't
//! have permission to use WebSearch right now" (F-07). Read-only tools are
//! already auto-allowed, which is why file questions were never affected.
//!
//! **WebSearch, never WebFetch.** A search query goes to a search engine; a
//! fetch goes to a URL the *prompt* can choose, and the prompt carries
//! unfiltered terminal content (RISK-01). That is an exfiltration channel,
//! so RISK-10 withholds it. `--permission-mode auto` was rejected for the
//! same reason: it grants the whole permission system, not one tool.

use serde::Deserialize;

/// How much authority the adapter grants a known agent (REQ-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tools {
    /// Let the agent search the web. The default: without it, a question
    /// whose answer is not already in the model comes back as a refusal.
    #[default]
    Research,
    /// Grant nothing. The one-shot token is still inserted — that is what
    /// makes the agent runnable, not part of the grant.
    None,
}

/// Insert the tokens a known agent needs, if they are absent.
pub fn normalize(argv: &[String], tools: Tools) -> Vec<String> {
    let mut out = argv.to_vec();
    let Some(program) = argv.first() else {
        return out;
    };
    // Match on the file name so an absolute path still resolves.
    let name = program.rsplit('/').next().unwrap_or(program);
    let rest = &argv[1..];
    let has = |flag: &str| rest.iter().any(|a| a == flag);
    let research = tools == Tools::Research;

    match name {
        "claude" => {
            // `at` tracks where the next insertion goes, so the grant lands
            // after a `-p` this call just inserted rather than before it.
            let mut at = 1;
            if !has("-p") && !has("--print") {
                out.insert(at, "-p".to_string());
                at += 1;
            }
            if research && !has("--allowedTools") && !has("--allowed-tools") {
                out.insert(at, "--allowedTools".to_string());
                out.insert(at + 1, "WebSearch".to_string());
            }
        }
        "codex" => {
            // `--search` is a *top-level* flag: `codex --search exec` is
            // accepted and `codex exec --search` is rejected outright
            // (exit 2, codex-cli 0.155.1). So it must precede the
            // subcommand -- including when the user supplied the flag and
            // this call supplies `exec`, which is why the insert position
            // is computed from the user's own argv rather than fixed at 1.
            let mut exec_at = 1;
            if let Some(i) = rest.iter().position(|a| a == "--search") {
                exec_at = i + 2;
            } else if research {
                out.insert(1, "--search".to_string());
                exec_at = 2;
            }
            if !has("exec") {
                out.insert(exec_at, "exec".to_string());
            }
        }
        "ollama" if rest.first().map(String::as_str) != Some("run") => {
            out.insert(1, "run".to_string())
        }
        "agy"
            if !rest.iter().any(|a| {
                a == "-p" || a == "--print" || a == "--prompt" || a.contains("{prompt}")
            }) =>
        {
            out.push("-p".to_string());
            out.push("{prompt}".to_string());
        }
        _ => {}
    }
    out
}
