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
