use std::process::ExitCode;

use clap::Parser;
use pskursor::{daemon::start_daemon, error::Result, tui::setup_tui};
use tracing::error;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "pskursor",
    version,
    about = "Drive your mouse cursor with a gamepad",
    long_about = None
)]
struct Cli {
    /// Open the configuration TUI to pick and activate a controller.
    #[arg(short, long)]
    config: bool,

    /// Log every raw evdev event. Repeat is not needed; `RUST_LOG` overrides this.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // The TUI owns the alternate screen, so its logs would be painted over and
    // then wiped on exit. Only the daemon installs a subscriber.
    if !cli.config {
        init_tracing(cli.verbose);
    }

    let outcome: Result<()> = if cli.config {
        setup_tui()
    } else {
        start_daemon()
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report(&err, cli.config);
            ExitCode::FAILURE
        }
    }
}

/// Install the tracing subscriber.
///
/// The default filter is `pskursor=debug` rather than `info`: controller events
/// are logged at `debug`, and an `info` default would silently discard exactly
/// the output the daemon exists to produce. `RUST_LOG` still wins when set.
fn init_tracing(verbose: bool) {
    let default_level = if verbose {
        "pskursor=trace"
    } else {
        "pskursor=debug"
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

/// Print an error chain. Falls back to stderr when no subscriber is installed.
fn report(err: &dyn std::error::Error, plain: bool) {
    let mut chain = format!("{err}");
    let mut source = err.source();
    while let Some(cause) = source {
        chain.push_str(&format!(": {cause}"));
        source = cause.source();
    }

    if plain {
        eprintln!("pskursor: {chain}");
    } else {
        error!("{chain}");
    }
}
