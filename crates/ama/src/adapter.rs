//! Known-agent adapters (ADR-003, as amended by A-06).
//!
//! An adapter may only *insert a missing one-shot token*. It never removes,
//! reorders, or rewrites a user's arguments, so a config that already works
//! keeps working when this table changes.
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

/// Insert the one-shot token a known agent needs, if it is absent.
pub fn normalize(argv: &[String]) -> Vec<String> {
    let mut out = argv.to_vec();
    let Some(program) = argv.first() else {
        return out;
    };
    // Match on the file name so an absolute path still resolves.
    let name = program.rsplit('/').next().unwrap_or(program);
    let rest = &argv[1..];
    let has_print_flag = || rest.iter().any(|a| a == "-p" || a == "--print");

    match name {
        "claude" if !has_print_flag() => out.insert(1, "-p".to_string()),
        "codex" if !rest.iter().any(|a| a == "exec") => out.insert(1, "exec".to_string()),
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
