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
