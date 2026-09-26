//! Load and validate `~/.ama/config.yml` and resolve it to a runnable agent.

use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::adapter::{self, Tools};

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
    /// `agent: claude --model opus` — a bare string. Still supported, but the
    /// documented form is `command:` below.
    Line(String),
    /// `agent: { command: [...] }` — the documented form. Adapters apply here
    /// too (A-06), so `[claude]` becomes `claude -p` rather than hanging in
    /// interactive mode. Set `adapter: false` for a fully literal argv.
    Structured {
        command: Vec<String>,
        #[serde(default = "default_adapter")]
        adapter: bool,
        /// REQ-32. Held as a string and parsed by `parse_tools` rather than
        /// deserialised straight into `Tools`: `AgentConfig` is `untagged`,
        /// and an untagged enum reports a failed variant as "data did not
        /// match any variant", throwing away the one detail a user needs --
        /// which value was wrong and what the alternatives are.
        #[serde(default = "default_tools")]
        tools: String,
    },
}

fn default_adapter() -> bool {
    true
}

fn default_tools() -> String {
    "research".to_string()
}

fn parse_tools(s: &str) -> Result<Tools, ConfigError> {
    match s.trim() {
        "research" => Ok(Tools::Research),
        "none" => Ok(Tools::None),
        other => Err(ConfigError::UnknownTools {
            value: other.to_string(),
        }),
    }
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
        "no config at {0}\n\nRun `ama setup` to create one, or write it yourself:\n\n  \
         mkdir -p \"$(dirname {0})\"\n  \
         printf 'agent:\\n  command: [claude]\\n' > {0}"
    )]
    Missing(PathBuf),
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not valid: {source}")]
    Parse {
        path: PathBuf,
        source: serde_yaml_ng::Error,
    },
    #[error("`agent:` is empty; it must name a command, e.g. `agent: claude`")]
    EmptyAgent,
    #[error(
        "`tools: {value}` is not a setting ama knows.\n\nUse `research` -- the \
         default, which lets the agent search the web -- or `none` to grant \
         nothing."
    )]
    UnknownTools { value: String },
}

/// `$AMA_CONFIG` when set, else `~/.ama/config.yml`.
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("AMA_CONFIG")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".ama").join("config.yml")
}

/// Build a spec from an already-normalised argv.
///
/// The placeholder is located *after* normalisation, and this is shared by
/// both config forms rather than duplicated. Both details are load-bearing:
/// inserting a one-shot token shifts every later index, and an adapter can
/// *create* the placeholder -- `agy` does, because its `-p` takes the prompt
/// as a value and it never reads stdin. When only the structured arm scanned
/// for it, `agent: agy` as a bare string sent the agent the literal text
/// `{prompt}` and threw the user's question away.
fn spec_from(argv: Vec<String>) -> AgentSpec {
    let prompt_via = argv
        .iter()
        .position(|a| a.contains("{prompt}"))
        .map_or(PromptVia::Stdin, PromptVia::Arg);
    AgentSpec { argv, prompt_via }
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
        let cfg: Config = serde_yaml_ng::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        match &self.agent {
            AgentConfig::Line(s) if s.trim().is_empty() => Err(ConfigError::EmptyAgent),
            AgentConfig::Structured { command, .. } if command.is_empty() => {
                Err(ConfigError::EmptyAgent)
            }
            // Rejected at load, not at first use: a typo that only surfaced
            // when the agent ran would look like the very defect `tools:`
            // exists to fix.
            AgentConfig::Structured { tools, .. } => parse_tools(tools).map(|_| ()),
            _ => Ok(()),
        }
    }

    /// Resolve the configuration into an invocation. Adapters apply to both
    /// forms (A-06) unless the structured form sets `adapter: false`.
    pub fn agent_spec(&self) -> Result<AgentSpec, ConfigError> {
        match &self.agent {
            AgentConfig::Line(line) => {
                let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
                if argv.is_empty() {
                    return Err(ConfigError::EmptyAgent);
                }
                Ok(spec_from(adapter::normalize(&argv, Tools::default())))
            }
            AgentConfig::Structured {
                command,
                adapter,
                tools,
            } => Ok(spec_from(if *adapter {
                // `adapter: false` outranks `tools:` -- 0.1.0 promised it
                // means "touch nothing at all", and a patch release does
                // not narrow that.
                adapter::normalize(command, parse_tools(tools)?)
            } else {
                command.clone()
            })),
        }
    }
}
