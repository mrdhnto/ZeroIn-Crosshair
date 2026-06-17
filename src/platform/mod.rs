#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(windows)]
pub use windows::run;
#[cfg(target_os = "linux")]
pub use linux::run;
