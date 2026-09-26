use ama::adapter::{self, Tools};

fn v(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

#[test]
fn claude_gains_the_one_shot_flag() {
    assert_eq!(
        adapter::normalize(&v("claude --model opus --effort high"), Tools::None),
        v("claude -p --model opus --effort high")
    );
}

#[test]
fn claude_keeps_an_existing_one_shot_flag() {
    assert_eq!(
        adapter::normalize(&v("claude -p --model opus"), Tools::None),
        v("claude -p --model opus")
    );
    assert_eq!(
        adapter::normalize(&v("claude --print"), Tools::None),
        v("claude --print")
    );
}

#[test]
fn adapters_match_on_the_file_name_not_the_whole_path() {
    assert_eq!(
        adapter::normalize(&v("/home/u/.local/bin/claude --model opus"), Tools::None),
        v("/home/u/.local/bin/claude -p --model opus")
    );
}

#[test]
fn ollama_gains_its_subcommand() {
    assert_eq!(
        adapter::normalize(&v("ollama llama3"), Tools::None),
        v("ollama run llama3")
    );
    assert_eq!(
        adapter::normalize(&v("ollama run llama3"), Tools::None),
        v("ollama run llama3")
    );
}

/// codex's one-shot mode is a *subcommand*, and `codex exec -p` means
/// `--profile`, not `--print` -- inserting a flag would silently change the
/// invocation. Verified against codex 0.155: `codex exec` reads the prompt
/// from stdin and writes only the answer to stdout.
#[test]
fn codex_gains_its_exec_subcommand() {
    assert_eq!(
        adapter::normalize(&v("codex"), Tools::None),
        v("codex exec")
    );
    assert_eq!(
        adapter::normalize(&v("codex -m gpt-5"), Tools::None),
        v("codex exec -m gpt-5")
    );
    assert_eq!(
        adapter::normalize(&v("codex exec"), Tools::None),
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
        adapter::normalize(&v("agy --effort low"), Tools::None),
        v("agy --effort low -p {prompt}")
    );
    assert_eq!(
        adapter::normalize(&v("agy"), Tools::None),
        v("agy -p {prompt}")
    );
}

#[test]
fn agy_is_left_alone_when_the_user_already_placed_the_prompt() {
    for already in [
        "agy -p {prompt}",
        "agy --print {prompt}",
        "agy --prompt {prompt}",
    ] {
        assert_eq!(
            adapter::normalize(&v(already), Tools::None),
            v(already),
            "{already}"
        );
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
    assert_eq!(
        adapter::normalize(&v("llm -m gpt-4o"), Tools::None),
        v("llm -m gpt-4o")
    );
    assert_eq!(adapter::normalize(&v("my-bot"), Tools::None), v("my-bot"));
}

#[test]
fn an_empty_argv_is_returned_unchanged() {
    assert_eq!(adapter::normalize(&[], Tools::None), Vec::<String>::new());
}

// ---- REQ-31: research authority (0.1.1) ------------------------------------
//
// A one-shot agent cannot answer a permission prompt, so anything needing
// approval is denied and the agent reports itself incapable -- `claude -p`
// says verbatim "I don't have permission to use WebSearch right now"
// (F-07). These pin the grant, and pin just as hard that it is *narrow*:
// WebSearch only, never WebFetch, which is the attacker-chosen-URL
// exfiltration primitive RISK-10 withholds.

#[test]
fn claude_is_granted_web_search() {
    assert_eq!(
        adapter::normalize(&v("claude --model opus"), Tools::Research),
        v("claude -p --allowedTools WebSearch --model opus")
    );
}

#[test]
fn the_grant_never_includes_web_fetch() {
    for agent in ["claude", "codex", "ollama", "agy"] {
        let got = adapter::normalize(&v(agent), Tools::Research).join(" ");
        assert!(
            !got.contains("WebFetch"),
            "RISK-10 withholds WebFetch; {agent} resolved to `{got}`"
        );
    }
}

/// The adapter may only insert what is *missing*. A user who names their own
/// tools has decided the question -- in either direction, wider or narrower
/// than ours -- and must not be second-guessed.
#[test]
fn a_users_own_tool_grant_is_left_alone() {
    for (given, want) in [
        (
            "claude --allowedTools WebSearch WebFetch",
            "claude -p --allowedTools WebSearch WebFetch",
        ),
        (
            "claude --allowed-tools Bash",
            "claude -p --allowed-tools Bash",
        ),
    ] {
        assert_eq!(
            adapter::normalize(&v(given), Tools::Research),
            v(want),
            "{given}"
        );
    }
}

/// `codex exec --search` is rejected by codex's own parser (exit 2), while
/// `codex --search exec` is accepted (exit 0) -- verified against
/// codex-cli 0.155.1. So the flag is top-level and must land *before* the
/// subcommand, including when the user wrote the flag and we supply `exec`.
#[test]
fn codex_gains_a_search_flag_before_its_subcommand() {
    assert_eq!(
        adapter::normalize(&v("codex"), Tools::Research),
        v("codex --search exec")
    );
    assert_eq!(
        adapter::normalize(&v("codex exec -m gpt-5"), Tools::Research),
        v("codex --search exec -m gpt-5")
    );
}

#[test]
fn a_users_own_codex_search_flag_still_precedes_an_inserted_exec() {
    assert_eq!(
        adapter::normalize(&v("codex --search"), Tools::Research),
        v("codex --search exec"),
        "exec must not be inserted ahead of a top-level flag"
    );
    assert_eq!(
        adapter::normalize(&v("codex --search exec"), Tools::Research),
        v("codex --search exec"),
        "nothing to do"
    );
}

/// ollama and agy have no web-search flag this project has verified, and an
/// unknown agent is never touched at all. Inserting a flag an agent does not
/// accept would break a config that works today (RISK-11).
#[test]
fn agents_with_no_verified_search_flag_gain_nothing() {
    assert_eq!(
        adapter::normalize(&v("ollama llama3"), Tools::Research),
        v("ollama run llama3")
    );
    assert_eq!(
        adapter::normalize(&v("agy"), Tools::Research),
        v("agy -p {prompt}")
    );
    assert_eq!(
        adapter::normalize(&v("my-bot --ask"), Tools::Research),
        v("my-bot --ask")
    );
}

/// The one-shot token is not part of the grant: `tools: none` must still
/// produce a runnable agent, or turning the grant off would hang the shell.
#[test]
fn turning_the_grant_off_still_leaves_the_agent_one_shot() {
    assert_eq!(
        adapter::normalize(&v("claude"), Tools::None),
        v("claude -p")
    );
    assert_eq!(
        adapter::normalize(&v("codex"), Tools::None),
        v("codex exec")
    );
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
fn a_bare_string_line_still_gains_the_one_shot_flag() {
    let (_d, p) = write_cfg("agent: claude --model opus --effort high\n");
    let cfg = Config::load(&p).expect("load");
    assert_eq!(cfg.max_context_lines, 200, "default cap");
    let spec = cfg.agent_spec().expect("spec");
    assert_eq!(
        spec.argv,
        v("claude -p --allowedTools WebSearch --model opus --effort high"),
        "REQ-32: an absent `tools:` means `research`"
    );
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
    assert_eq!(
        spec.argv,
        v("claude -p --allowedTools WebSearch --model opus")
    );
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
    assert_eq!(
        spec.argv,
        v("claude -p --allowedTools WebSearch --ask {prompt}")
    );
    assert_eq!(
        spec.prompt_via,
        PromptVia::Arg(5),
        "shifted by the inserted -p and the two-token grant"
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

/// C2: `normalize` can *create* a `{prompt}` placeholder -- agy's adapter
/// does -- so the bare-string form must scan for it too. It used to hardcode
/// `Stdin`, which sent agy the literal text `{prompt}` and discarded the
/// user's question.
#[test]
fn a_bare_string_agy_also_takes_its_prompt_as_an_argument() {
    let (_d, p) = write_cfg("agent: agy\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("agy -p {prompt}"));
    assert_eq!(
        spec.prompt_via,
        PromptVia::Arg(2),
        "the bare-string form must resolve the placeholder exactly like command:"
    );
}

#[test]
fn both_config_forms_resolve_an_agent_identically() {
    for text in [
        "agent: agy --effort low\n",
        "agent:\n  command: [agy, --effort, low]\n",
    ] {
        let (_d, p) = write_cfg(text);
        let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
        assert_eq!(spec.argv, v("agy --effort low -p {prompt}"), "{text}");
        assert_eq!(spec.prompt_via, PromptVia::Arg(4), "{text}");
    }
}

#[test]
fn adapter_false_still_honours_a_hand_written_placeholder() {
    let (_d, p) = write_cfg("agent:\n  command: [agy, -p, \"{prompt}\"]\n  adapter: false\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("agy -p {prompt}"), "nothing inserted");
    assert_eq!(
        spec.prompt_via,
        PromptVia::Arg(2),
        "the user's own placeholder still resolves"
    );
}

// ---- REQ-32: the grant is configurable (0.1.1) ------------------------------

/// `tools: none` is the documented way back to 0.1.0's argv, and it is the
/// rollback path named in the plan. It must reproduce that argv exactly --
/// including the one-shot token, which is not part of the grant.
#[test]
fn tools_none_reproduces_the_0_1_0_invocation() {
    let (_d, p) = write_cfg("agent:\n  command: [claude, --model, opus]\n  tools: none\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude -p --model opus"));
}

#[test]
fn tools_research_is_the_same_as_omitting_the_key() {
    let (_d, a) = write_cfg("agent:\n  command: [claude]\n  tools: research\n");
    let (_d2, b) = write_cfg("agent:\n  command: [claude]\n");
    assert_eq!(
        Config::load(&a).expect("load").agent_spec().expect("spec"),
        Config::load(&b).expect("load").agent_spec().expect("spec")
    );
}

/// Silently treating a typo as `none` would leave the reported defect in
/// place while looking configured, which is the failure mode this whole
/// cycle exists to remove.
#[test]
fn an_unknown_tools_value_is_rejected_rather_than_ignored() {
    let (_d, p) = write_cfg("agent:\n  command: [claude]\n  tools: reserch\n");
    let err = Config::load(&p).expect_err("a typo must not load");
    let msg = err.to_string();
    assert!(msg.contains("reserch"), "must name the bad value: {msg}");
    assert!(
        msg.contains("research") && msg.contains("none"),
        "must name the accepted values: {msg}"
    );
}

/// `adapter: false` means "touch nothing at all" and outranks `tools:`;
/// 0.1.0 promised that and a patch release must not narrow it.
#[test]
fn adapter_false_outranks_the_tools_key() {
    let (_d, p) = write_cfg("agent:\n  command: [claude]\n  adapter: false\n  tools: research\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude"), "nothing inserted, not even -p");
}

/// The bare-string form is still supported, so it must get the same default
/// as the documented form -- the two resolving differently is exactly the
/// trap A-06 removed in 0.1.0.
#[test]
fn the_bare_string_form_gets_the_default_grant_too() {
    let (_d, p) = write_cfg("agent: claude\n");
    let spec = Config::load(&p).expect("load").agent_spec().expect("spec");
    assert_eq!(spec.argv, v("claude -p --allowedTools WebSearch"));
}

/// REQ-39: the moon is on unless a config says otherwise, so every 0.1.1
/// config gets it without an edit.
#[test]
fn the_spinner_is_on_unless_the_config_turns_it_off() {
    let (_d, p) = write_cfg("agent: llm\n");
    assert!(Config::load(&p).expect("load").spinner, "absent means on");
    let (_d, p) = write_cfg("agent: llm\nspinner: false\n");
    assert!(!Config::load(&p).expect("load").spinner);
}

/// REQ-39: like `tools:` (REQ-32), a value ama cannot read is an error that
/// names the key -- never quietly taken as one of the two.
#[test]
fn a_spinner_value_that_is_not_a_boolean_is_rejected() {
    let (_d, p) = write_cfg("agent: llm\nspinner: sometimes\n");
    let err = Config::load(&p).expect_err("not a boolean").to_string();
    assert!(err.contains("spinner") && err.contains("boolean"), "{err}");
}
