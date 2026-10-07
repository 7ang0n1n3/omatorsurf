mod cli;
mod command;
mod config;
mod controller;
mod dns;
mod error;
mod firewall;
mod health;
mod network;
mod privilege;
mod runtime;
mod status;
mod tor;

use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    match cli::Cli::parse().dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ERROR: {error}");
            ExitCode::FAILURE
        }
    }
}
