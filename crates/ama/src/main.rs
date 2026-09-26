#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

fn main() -> std::process::ExitCode {
    ama::cli::run()
}
