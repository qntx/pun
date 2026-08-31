//! P2P pipe: punch through NAT and send files with blake3-verified streaming.
//!
//! Adapted from n0-computer/sendme (Apache-2.0 OR MIT),
//! commit 8dda1e5383209e9027dd54430c77059ef51adc2e.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI binary writes human-readable progress and tickets to stdio"
)]

mod cli;
#[cfg(feature = "clipboard")]
mod clipboard;
mod endpoint;
mod error;
mod export;
mod import;
mod path;
mod progress;
mod receive;
mod secret;
mod send;
mod store;
mod ticket;

use std::process::ExitCode;

use clap::Parser;
#[cfg(test)]
use duct as _;
#[cfg(test)]
use tempfile as _;

use crate::cli::{Args, Commands};
use crate::error::to_exit_code;

fn main() -> ExitCode {
    tracing_subscriber::fmt::init();
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(err) => {
            drop(err.print());
            return ExitCode::from(u8::try_from(err.exit_code()).unwrap_or(1));
        }
    };
    tokio_main(args)
}

#[tokio::main]
async fn tokio_main(args: Args) -> ExitCode {
    let result = match args.command {
        Commands::Send(send) => send::run(send).await,
        Commands::Receive(recv) => receive::run(recv).await,
    };
    to_exit_code(result)
}
