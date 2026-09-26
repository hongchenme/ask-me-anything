#![forbid(unsafe_code)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

pub mod adapter;
pub mod agent;
pub mod cli;
pub mod config;
pub mod context;
pub mod prompt;
pub mod render;
pub mod session;
pub mod shellinit;
pub mod spinner;
