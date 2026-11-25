// No console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod battery;
mod commands;
mod indicators;
mod logging;
mod notifications;
mod settings;
mod shell;
mod system;
mod tray;
mod window;

fn main() {
    app::run();
}
