#![cfg_attr(windows, windows_subsystem = "windows")]

mod canvas;
mod config;
mod crosshair;
mod platform;
mod profiles;

fn main() {
    platform::run();
}
