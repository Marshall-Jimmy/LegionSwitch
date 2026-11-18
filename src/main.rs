#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(unsafe_op_in_unsafe_fn)]

mod autostart;
mod cli;
mod config;
mod console;
mod modes;
mod power;
mod ui;
mod wmi;
mod worker;

fn main() {
    let launch = cli::parse_args(std::env::args().skip(1));
    match launch {
        cli::Launch::Tray { show } => {
            if let Err(error) = ui::run(show) {
                ui::fatal_error(&error);
            }
        }
        other => {
            let code = cli::run_cli(other);
            std::process::exit(code);
        }
    }
}
