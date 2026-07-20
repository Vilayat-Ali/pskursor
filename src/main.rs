use pskursor::{daemon::start_daemon, tui::setup_tui};
use std::env;

fn main() {
    let args = env::args().collect::<Vec<String>>();

    if args.iter().any(|arg| arg == "--config" || arg == "-c") {
        setup_tui().unwrap();
    }

    start_daemon();
}
