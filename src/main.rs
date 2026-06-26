#![cfg_attr(windows, windows_subsystem = "windows")]

pub mod canvas;
pub mod config;
pub mod crosshair;
mod platform;
pub mod profiles;

fn main() {
    platform::run();
}
