// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use colored::Colorize;
use sui::client_commands::SuiClientCommands::{ProfileTransaction, ReplayBatch, ReplayTransaction};
use sui::sui_commands::SuiCommand;
use sui_types::exit_main;
use tracing::debug;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[clap(
    name = env!("CARGO_BIN_NAME"),
    about = "A Byzantine fault tolerant chain with low-latency finality and high throughput",
    rename_all = "kebab-case",
    author,
    version = VERSION,
    propagate_version = true,
)]
struct Args {
    #[clap(subcommand)]
    command: SuiCommand,
}

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();

    let args = Args::parse();
    let _guard = match args.command {
        SuiCommand::Console { .. } | SuiCommand::KeyTool { .. } | SuiCommand::Move { .. } => {
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("error")
                .with_env()
                .init()
        }

        SuiCommand::Client {
            cmd: Some(ReplayBatch { .. }),
            ..
        } => telemetry_subscribers::TelemetryConfig::new()
            .with_log_level("info")
            .with_env()
            .init(),

        SuiCommand::Client {
            cmd: Some(ReplayTransaction {
                gas_info, ptb_info, ..
            }),
            ..
        } => {
            let mut config = telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("info")
                .with_env();
            if gas_info {
                config = config.with_trace_target("replay_gas_info");
            }
            if ptb_info {
                config = config.with_trace_target("replay_ptb_info");
            }
            config.init()
        }

        SuiCommand::Client {
            cmd: Some(ProfileTransaction { .. }),
            ..
        } => {
            // enable full logging for ProfileTransaction and ReplayTransaction
            telemetry_subscribers::TelemetryConfig::new()
                .with_env()
                .init()
        }

        _ => telemetry_subscribers::TelemetryConfig::new()
            .with_log_level("error")
            .with_env()
            .init(),
    };
    debug!("Sui CLI version: {VERSION}");


    // Create a future for command execution
    let command_future = args.command.execute();

    // Create a shutdown signal future.
    // On Unix, we can listen for SIGTERM and SIGINT.
    #[cfg(unix)]
    let shutdown_signal = async {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to set up SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to set up SIGINT handler");
        tokio::select! {
            _ = sigterm.recv() => {},
            _ = sigint.recv() => {},
        }
        println!("Received shutdown signal (SIGTERM or SIGINT)");
    };

    // On non-Unix platforms, fallback to Ctrl+C handling.
    #[cfg(not(unix))]
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.expect("Failed to set up Ctrl+C handler");
        println!("Received shutdown signal (Ctrl+C)");
    };

    // Use tokio::select! to run the command or shutdown signal concurrently.
    tokio::select! {
        res = command_future => {
            // When the command finishes, exit with its result.
            exit_main!(res);
        },
        _ = shutdown_signal => {
            // Gracefully shutdown when a signal is received.
            println!("Shutting down gracefully...");
            // Here you can perform any cleanup if necessary.
            std::process::exit(0);
        }
    }
}
