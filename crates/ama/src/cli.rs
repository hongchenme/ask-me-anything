//! Argument parsing, dispatch, and the exit-code contract (NFR-05).

use clap::{Parser, Subcommand};
use std::io::Write;
use std::process::ExitCode;

use crate::spinner::Spinner;
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
    /// Create the `@@` alias, a starter config, and wire up your shell.
    ///
    /// This is what makes a `curl` install work with no git clone: cargo-dist
    /// ships only `ama`, because rustc rejects a crate named `@@`, so the
    /// alias has to be made after the fact.
    Setup {
        /// Print what would change and exit.
        #[arg(long)]
        dry_run: bool,
    },
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
        let joined = argv[1..].join(" ");
        let (question, no_context) = peel_no_context(&joined);
        return ask(question, no_context);
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
        Cmd::Setup { dry_run } => setup(dry_run),
    }
}

fn fail(code: u8, msg: &str) -> ExitCode {
    eprintln!("ama: {msg}");
    ExitCode::from(code)
}

/// R24/REQ-15: honour a leading `--no-context` on the `@@` trigger path.
///
/// `--no-context` is one of only two mitigations 04-design.md names for the
/// owner-accepted RISK-01, and the trigger path used to join all of argv
/// into the question -- so `@@ --no-context what is my key` sent the whole
/// pane *and* asked the agent a question starting "--no-context".
///
/// This peels the *joined* question, not an argv token, because argv does
/// not survive the hook: `ama.bash` rewrites the typed line to
/// `@@ '--no-context what is my key'`, a single quoted word, so by the time
/// this process starts `argv[1..]` is one element. Peeling after the join
/// is the only form that covers both that path and a script's
/// `@@ --no-context foo`. Everything after the flag is still the question
/// byte-for-byte.
///
/// Accepted cost: a question whose literal first word is `--no-context` is
/// no longer askable through the trigger. `ama ask -- --no-context …`
/// still is. Only a whole token counts, so `--no-contextual` is a question.
fn peel_no_context(question: &str) -> (&str, bool) {
    match question.trim_start().strip_prefix("--no-context") {
        Some(rest) if rest.is_empty() || rest.starts_with(char::is_whitespace) => (rest, true),
        _ => (question, false),
    }
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

    // REQ-35: the moon, from now until the answer starts. Not a moment
    // sooner: the pane has only just been captured, and a frame drawn before
    // that would have been sent to the agent as part of its own context
    // (F-15).
    let spinner = if cfg.spinner {
        Spinner::for_terminal()
    } else {
        None
    };

    // Tee the answer so it can be recorded for the transcript fallback.
    let mut captured: Vec<u8> = Vec::new();
    let mut sink = Tee {
        a: std::io::stdout(),
        b: &mut captured,
    };

    let result = agent::run(&spec, &composed, &mut sink, spinner.as_ref());
    // `run` has erased the moon on whichever path it took, spawn failures
    // included. Nothing below draws; let it go before anything prints.
    drop(spinner);

    match result {
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
        format!("not loaded -- run: ama setup   (wires {rc} for {name})")
    }
}

/// R30/REQ-25: `is_file()` is not the question `doctor` is asking. A
/// mode-644 file sitting on `$PATH` -- a downloaded-but-never-`chmod`ed
/// agent, the single most likely way for this check to matter -- made
/// `doctor` print `status ok`, and then the next turn failed at spawn with
/// a permission error. REQ-25 says "whether the agent is executable", so
/// test that.
#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}

fn which(program: &str) -> bool {
    if program.contains('/') {
        return is_executable(std::path::Path::new(program));
    }
    std::env::var("PATH")
        .is_ok_and(|paths| std::env::split_paths(&paths).any(|d| is_executable(&d.join(program))))
}

/// `ama setup` — everything a fresh install still needs after the binary
/// lands on disk (A-07).
///
/// `cargo dist` ships only `ama`: rustc rejects a crate named `@@`, so the
/// trigger alias cannot be a second `[[bin]]` and has to be created here.
/// That is also why this exists as a subcommand rather than a script — after
/// `curl … | sh` there is no repository to run a script from.
///
/// Every step is idempotent, so re-running after an upgrade is safe.
fn setup(dry_run: bool) -> ExitCode {
    let mut plan: Vec<String> = Vec::new();
    let mut unsupported_shell = false;
    let note = |plan: &mut Vec<String>, s: String| plan.push(s);

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return fail(EXIT_CONFIG, &format!("cannot locate the ama binary: {e}")),
    };
    let Some(bin_dir) = exe.parent().map(std::path::Path::to_path_buf) else {
        return fail(EXIT_CONFIG, "the ama binary has no parent directory");
    };
    let trigger = bin_dir.join("@@");

    // 1. The `@@` alias.
    let alias_ok = trigger.read_link().map(|t| t == exe).unwrap_or(false);
    if alias_ok {
        note(
            &mut plan,
            format!("ok    {} already points at ama", trigger.display()),
        );
    } else if dry_run {
        note(
            &mut plan,
            format!("would symlink {} -> {}", trigger.display(), exe.display()),
        );
    } else {
        match link_trigger(&exe, &trigger) {
            Ok(()) => note(
                &mut plan,
                format!("made  {} -> {}", trigger.display(), exe.display()),
            ),
            Err(e) => {
                return fail(
                    EXIT_CONFIG,
                    &format!(
                        "could not create {}: {e}\n\nThe `@@` alias lives beside the \
                         ama binary, so that directory must be writable. Install \
                         somewhere you own and re-run:\n  AMA_PREFIX=~/.local/bin \
                         ./install.sh",
                        trigger.display()
                    ),
                );
            }
        }
    }

    // 2. A starter config, never overwriting one that exists.
    let cfg = config::config_path();
    if cfg.exists() {
        note(
            &mut plan,
            format!("ok    {} already exists, left alone", cfg.display()),
        );
    } else if dry_run {
        note(
            &mut plan,
            format!("would write a starter config to {}", cfg.display()),
        );
    } else {
        match write_starter_config(&cfg) {
            Ok(()) => note(&mut plan, format!("wrote {}", cfg.display())),
            Err(e) => {
                return fail(
                    EXIT_CONFIG,
                    &format!("could not write {}: {e}", cfg.display()),
                );
            }
        }
    }

    // 3. The shell rc: PATH first, then the integration.
    let shell = shellinit::Shell::parse(&std::env::var("SHELL").unwrap_or_default());
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    match shell {
        Some(sh) => {
            let (rc, name) = match sh {
                shellinit::Shell::Bash => (std::path::PathBuf::from(&home).join(".bashrc"), "bash"),
                shellinit::Shell::Zsh => (std::path::PathBuf::from(&home).join(".zshrc"), "zsh"),
            };
            let existing = std::fs::read_to_string(&rc).unwrap_or_default();

            // Absolute path, not a bare `ama`. An rc runs before PATH is
            // necessarily set up: Debian/Ubuntu's ~/.profile sources
            // ~/.bashrc *before* it adds ~/.local/bin, so a bare `ama` here
            // makes `eval` a silent no-op in every login shell and the hook
            // never loads. The old install.sh used an absolute path; losing
            // it was a regression.
            let eval_line = format!("eval \"$({} init {name})\"", sh_quote(&exe));

            // The PATH line is decided from the rc's contents alone, never
            // from this process's PATH -- those are different moments, and
            // the hook needs bare `@@` and `ama session reset` resolvable at
            // runtime (see ama.bash) regardless of how setup was invoked.
            let path_line = format!("export PATH={}:\"$PATH\"", sh_quote(&bin_dir));

            let mut add: Vec<String> = Vec::new();
            if !existing.contains(&path_line) {
                add.push(path_line);
            }
            if !existing.contains(&format!("init {name})")) {
                add.push(eval_line);
            }
            // Emit as one block or not at all: appending them in separate
            // runs could land PATH *below* the eval that needs it, which no
            // later run would repair.
            if add.len() == 1 && !existing.is_empty() {
                add.clear();
            }

            if add.is_empty() {
                note(
                    &mut plan,
                    format!("ok    {} already wired for {name}", rc.display()),
                );
            } else if dry_run {
                for l in &add {
                    note(&mut plan, format!("would add to {}: {l}", rc.display()));
                }
            } else {
                match append_rc(&rc, &add) {
                    Ok(()) => {
                        for l in &add {
                            note(&mut plan, format!("added to {}: {l}", rc.display()));
                        }
                    }
                    Err(e) => {
                        return fail(
                            EXIT_CONFIG,
                            &format!("could not update {}: {e}", rc.display()),
                        );
                    }
                }
            }
        }
        // The Enter trigger is a bash/zsh readline/ZLE hook: there is no
        // fish or nushell equivalent to hand out, and pasting
        // `eval "$(...)"` into config.fish is a syntax error. Saying so and
        // pointing at the shell-neutral command beats promising `@@` will
        // work and letting the user find out otherwise.
        None => {
            unsupported_shell = true;
            note(
                &mut plan,
                format!(
                    "skip  $SHELL is not bash or zsh, so no rc file was touched.\n      \
                     The `@@` Enter trigger is bash/zsh only -- it is a readline/ZLE\n      \
                     hook with no equivalent in other shells. `ama ask -- <question>`\n      \
                     works everywhere; put {} on your PATH to use it.",
                    bin_dir.display()
                ),
            )
        }
    }

    for line in &plan {
        println!("{line}");
    }
    if dry_run {
        println!("\nDry run — nothing was changed.");
    } else {
        println!();
        if unsupported_shell {
            println!("Done. `ama ask -- what is this project about` works in any shell.");
        } else {
            println!("Done. Open a new shell, then try:  @@ what is this project about");
        }
        println!("`ama doctor` will confirm the integration is live.");
    }
    ExitCode::from(EXIT_OK)
}

/// Single-quote a path for a shell rc line.
///
/// The same reasoning as `__ama_sq`: inside `'…'` every byte is literal and
/// `'` is the only character that can end the quote. A prefix containing a
/// space, `$`, a backtick or a quote would otherwise expand or break on
/// every shell start.
fn sh_quote(p: &std::path::Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))
}

#[cfg(unix)]
fn link_trigger(exe: &std::path::Path, trigger: &std::path::Path) -> std::io::Result<()> {
    // Replace rather than fail: an upgrade moves the binary, and a stale
    // symlink is worse than no symlink.
    match std::fs::remove_file(trigger) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    std::os::unix::fs::symlink(exe, trigger)
}

#[cfg(not(unix))]
fn link_trigger(exe: &std::path::Path, trigger: &std::path::Path) -> std::io::Result<()> {
    std::fs::copy(exe, trigger).map(|_| ())
}

fn write_starter_config(cfg: &std::path::Path) -> std::io::Result<()> {
    if let Some(dir) = cfg.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        cfg,
        "# Your agent. Any CLI that reads a prompt on stdin and writes an\n\
         # answer to stdout works. ama inserts the one-shot flag for agents\n\
         # it knows (claude, codex, agy, ollama), plus a web-search grant --\n\
         # without it a one-shot agent has nobody to approve the tool and\n\
         # answers \"I don't have internet access\" instead of looking.\n\
         #\n\
         # `tools: none` drops the grant; `adapter: false` stops ama\n\
         # touching your argv at all. `ama doctor` prints what will run.\n\
         agent:\n  command: [claude]\n\n\
         # Lines of terminal scrollback sent as context.\n\
         max_context_lines: 200\n",
    )
}

fn append_rc(rc: &std::path::Path, lines: &[String]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc)?;
    writeln!(f, "\n# ama -- ask me anything")?;
    for l in lines {
        writeln!(f, "{l}")?;
    }
    Ok(())
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

    // R24: the token boundary is the whole of the risk here -- peel too
    // eagerly and a legitimate question silently loses its first word and
    // its context with it.
    #[test]
    fn a_leading_no_context_token_is_peeled_off_a_trigger_question() {
        assert_eq!(
            peel_no_context("--no-context what is my key"),
            (" what is my key", true)
        );
        assert_eq!(peel_no_context("--no-context"), ("", true));
    }

    #[test]
    fn no_context_is_only_peeled_as_a_whole_leading_token() {
        assert_eq!(
            peel_no_context("--no-contextual awareness"),
            ("--no-contextual awareness", false)
        );
        assert_eq!(
            peel_no_context("why is --no-context ignored"),
            ("why is --no-context ignored", false)
        );
        assert_eq!(
            peel_no_context("what does --no-context do"),
            ("what does --no-context do", false)
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
