//! # Prekursor
//!
//! <div align="center">
//!     <img src="../assets/banner.png" alt="" />
//! </div>
//!
//! <hr />
//!
//! **pskursor** is a simple Linux device driver written in Rust. It lets you use your gamepads and controllers to control your mouse cursor, making it fun to navigate your desktop without a mouse.

use pskursor::{daemon::start_daemon, tui::setup_tui};
use std::env;

fn main() {
    let args = env::args().collect::<Vec<String>>();

    if args.iter().any(|arg| arg == "--config" || arg == "-c") {
        setup_tui().unwrap();
    } else {
        start_daemon();
    }
}
