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
}

/// `$AMA_CONFIG` when set, else `~/.qmx2/config.yml`.
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("AMA_CONFIG")
        && !p.is_empty()
    {
        return PathBuf::from(p);
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
                let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
                if argv.is_empty() {
                    return Err(ConfigError::EmptyAgent);
                }
                Ok(AgentSpec {
                    argv: adapter::normalize(&argv),
                    prompt_via: PromptVia::Stdin,
                })
            }
            AgentConfig::Structured { command } => {
                let via = command
                    .iter()
                    .position(|a| a.contains("{prompt}"))
                    .map_or(PromptVia::Stdin, PromptVia::Arg);
                Ok(AgentSpec {
                    argv: command.clone(),
                    prompt_via: via,
                })
            }
        }
    }
}
