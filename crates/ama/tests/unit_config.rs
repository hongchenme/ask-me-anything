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
    assert_eq!(
        adapter::normalize(&v("claude -p --model opus")),
        v("claude -p --model opus")
    );
    assert_eq!(
        adapter::normalize(&v("claude --print")),
        v("claude --print")
    );
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
    assert_eq!(
        adapter::normalize(&v("ollama llama3")),
        v("ollama run llama3")
    );
    assert_eq!(
        adapter::normalize(&v("ollama run llama3")),
        v("ollama run llama3")
    );
}

/// codex's one-shot mode is a *subcommand*, and `codex exec -p` means
/// `--profile`, not `--print` -- inserting a flag would silently change the
/// invocation. Verified against codex 0.155: `codex exec` reads the prompt
/// from stdin and writes only the answer to stdout.
#[test]
fn codex_gains_its_exec_subcommand() {
    assert_eq!(adapter::normalize(&v("codex")), v("codex exec"));
    assert_eq!(
        adapter::normalize(&v("codex -m gpt-5")),
        v("codex exec -m gpt-5")
    );
    assert_eq!(
        adapter::normalize(&v("codex exec")),
        v("codex exec"),
        "already one-shot"
    );
}

/// agy is the one agent that does not read the prompt from stdin: its `-p`
/// takes the prompt as its *value*, and `agy -p` alone fails with
/// `flag needs an argument: -p`. The adapter therefore appends a `{prompt}`
/// placeholder too, which `agent_spec` resolves to `PromptVia::Arg`.
/// Appended rather than inserted after the program, so the flag and its
/// value cannot be separated by the user's own arguments.
#[test]
fn agy_gains_a_print_flag_and_a_prompt_placeholder() {
    assert_eq!(
        adapter::normalize(&v("agy --effort low")),
        v("agy --effort low -p {prompt}")
    );
    assert_eq!(adapter::normalize(&v("agy")), v("agy -p {prompt}"));
}

#[test]
fn agy_is_left_alone_when_the_user_already_placed_the_prompt() {
    for already in [
        "agy -p {prompt}",
        "agy --print {prompt}",
        "agy --prompt {prompt}",
    ] {
        assert_eq!(adapter::normalize(&v(already)), v(already), "{already}");
    }
}

#[test]
fn an_adapted_agy_takes_its_prompt_as_an_argument_not_stdin() {
    let (_d, p) = write_cfg("agent:\n  command: [agy]\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("agy -p {prompt}"));
    assert_eq!(
        spec.prompt_via,
        PromptVia::Arg(2),
        "agy does not read stdin; the prompt must land in argv"
    );
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

use ama::config::{Config, ConfigError, PromptVia};
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

/// A-06: the structured form is the *documented* form, so it must not be the
/// one that hangs. `command: [claude]` with no adapter would launch an
/// interactive session and never return.
#[test]
fn a_structured_command_gets_adapters_too() {
    let (_d, p) = write_cfg("agent:\n  command: [claude, --model, opus]\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude -p --model opus"));
    assert_eq!(spec.prompt_via, PromptVia::Stdin);
}

#[test]
fn adapter_false_keeps_the_command_literal() {
    let (_d, p) = write_cfg("agent:\n  command: [claude, --model, opus]\n  adapter: false\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude --model opus"), "nothing inserted");
}

/// The placeholder index is resolved *after* normalisation. Inserting a
/// one-shot token shifts every later argument, so computing it first would
/// substitute the prompt into the wrong slot.
#[test]
fn a_placeholder_index_survives_an_inserted_one_shot_token() {
    let (_d, p) = write_cfg("agent:\n  command: [claude, --ask, \"{prompt}\"]\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude -p --ask {prompt}"));
    assert_eq!(
        spec.prompt_via,
        PromptVia::Arg(3),
        "shifted by the inserted -p"
    );
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
fn a_missing_file_remedy_names_the_path_it_actually_checked() {
    let err = Config::load(std::path::Path::new("/nonexistent/custom/ama.yml")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("/nonexistent/custom/ama.yml"),
        "remedy must name the real path: {msg}"
    );
    assert!(
        !msg.contains(".ama/config.yml"),
        "remedy must not point elsewhere: {msg}"
    );
}

#[test]
fn malformed_yaml_names_the_problem_and_does_not_panic() {
    let (_d, p) = write_cfg("agent: [unclosed\n");
    assert!(matches!(
        Config::load(&p).unwrap_err(),
        ConfigError::Parse { .. }
    ));
}

#[test]
fn an_unknown_key_is_rejected_rather_than_ignored() {
    let (_d, p) = write_cfg("agent: llm\nmax_contxt_lines: 40\n");
    let err = Config::load(&p).unwrap_err();
    assert!(
        err.to_string().contains("max_contxt_lines"),
        "typo must be named: {err}"
    );
}

#[test]
fn an_empty_agent_is_a_configuration_error() {
    let (_d, p) = write_cfg("agent: \"\"\n");
    assert!(matches!(
        Config::load(&p).unwrap_err(),
        ConfigError::EmptyAgent
    ));
}

#[test]
fn an_empty_file_is_a_configuration_error_not_a_panic() {
    let (_d, p) = write_cfg("");
    assert!(Config::load(&p).is_err());
}
