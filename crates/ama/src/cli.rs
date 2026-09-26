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
        ///
        /// `allow_hyphen_values` is required alongside `trailing_var_arg`:
        /// without it clap still reads a leading `-` as an unknown flag, and
        /// a question is arbitrary text that must be free to start with one
        /// (R4).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
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
        Cmd::Ask {
            no_context,
            question,
        } => ask(&question.join(" "), no_context),
        Cmd::Init { shell } => init(&shell),
        Cmd::Session {
            action: SessionCmd::Reset,
        } => match session::reset(&session::current_key()) {
            Ok(()) => ExitCode::from(EXIT_OK),
            Err(e) => fail(EXIT_AGENT, &format!("could not reset session: {e}")),
        },
        Cmd::Doctor => doctor(),
    }
}

fn fail(code: u8, msg: &str) -> ExitCode {
    eprintln!("ama: {msg}");
    ExitCode::from(code)
}

/// R2: `NoCommand`/`Spawn` name something the user's `agent:` line cannot
/// even start -- a configuration problem. `Io` means the agent *did* start
/// and broke while ama was talking to it -- an agent failure, not a
/// misconfiguration.
fn agent_error_code(err: &agent::AgentError) -> u8 {
    match err {
        agent::AgentError::Io { .. } => EXIT_AGENT,
        agent::AgentError::NoCommand | agent::AgentError::Spawn { .. } => EXIT_CONFIG,
    }
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
    let mut sink = Tee {
        a: std::io::stdout(),
        b: &mut captured,
    };

    match agent::run(&spec, &composed, &mut sink) {
        Ok(0) => {
            let answer = String::from_utf8_lossy(&captured).into_owned();
            let _ = session::append_turn(
                &key,
                &session::Turn {
                    question: question.to_string(),
                    answer: strip_render(&answer),
                },
            );
            ExitCode::from(EXIT_OK)
        }
        // R1: the exit-code contract is applied here. Lower layers return
        // raw information (fake-agent-fail's `exit 7` comes back as
        // `Ok(7)`, correctly, from agent::run); this layer maps any
        // non-zero agent exit to the project's single agent-failure code.
        Ok(_) => ExitCode::from(EXIT_AGENT),
        Err(e) => fail(agent_error_code(&e), &e.to_string()),
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
        None => fail(
            EXIT_CONFIG,
            &format!("unknown shell `{shell}`; expected bash or zsh"),
        ),
    }
}

fn doctor() -> ExitCode {
    let path = config::config_path();
    println!("config       {}", path.display());
    println!("shell        {}", shell_name());
    println!("integration  {}", integration_status());
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

/// R19/REQ-25: the honest source for "which shell would `install.sh` wire
/// up" is `$SHELL`'s basename -- the same value `install.sh` itself reads
/// to pick a rc file. Anything other than exactly `bash`/`zsh` is reported
/// as `unknown` rather than guessed at, since `ama init` itself only knows
/// those two (see `Cmd::Init`'s own rejection of anything else).
fn shell_name() -> &'static str {
    let raw = std::env::var("SHELL").unwrap_or_default();
    match shellinit::Shell::parse(&raw) {
        Some(shellinit::Shell::Bash) => "bash",
        Some(shellinit::Shell::Zsh) => "zsh",
        None => "unknown",
    }
}

/// R19/REQ-25: `$AMA_SESSION` is exported only inside the interactive guard
/// in `ama.bash`/`ama.zsh` (`if [[ $- == *i* ]]` / `if [[ -o interactive ]]`),
/// so its presence in this process's environment is direct evidence the
/// Enter hook actually ran in *this* shell -- not just that `ama init` was
/// printed somewhere once. Its absence is not an error: REQ-27 requires
/// `@@`/`ama ask` to work standalone from a script with no hook at all, so
/// this is informational only, same as every other `doctor` line.
fn integration_status() -> String {
    if std::env::var("AMA_SESSION").is_ok_and(|v| !v.is_empty()) {
        format!("loaded ({} hook active)", shell_name())
    } else {
        let (name, rc) = match shell_name() {
            "zsh" => ("zsh", "~/.zshrc"),
            _ => ("bash", "~/.bashrc"),
        };
        format!("not loaded -- add: eval \"$(ama init {name})\" to {rc}")
    }
}

fn which(program: &str) -> bool {
    if program.contains('/') {
        return std::path::Path::new(program).is_file();
    }
    std::env::var("PATH")
        .is_ok_and(|paths| std::env::split_paths(&paths).any(|d| d.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // R2's mapping is otherwise only reachable through a live process
    // (a real unrunnable `agent:` covers Spawn/NoCommand in
    // integration_cli.rs) or a genuinely broken pipe (already covered for
    // `agent::run` itself in integration_agent.rs). `AgentError::Io` has no
    // deterministic, non-flaky trigger from a black-box subprocess test, so
    // the mapping is pinned directly here instead.

    #[test]
    fn no_command_and_spawn_are_configuration_errors() {
        assert_eq!(agent_error_code(&agent::AgentError::NoCommand), EXIT_CONFIG);
        assert_eq!(
            agent_error_code(&agent::AgentError::Spawn {
                program: "definitely-not-installed".into(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
            }),
            EXIT_CONFIG
        );
    }

    #[test]
    fn io_failures_are_agent_failures_not_configuration_errors() {
        assert_eq!(
            agent_error_code(&agent::AgentError::Io {
                program: "some-agent".into(),
                source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken"),
            }),
            EXIT_AGENT
        );
    }
}
