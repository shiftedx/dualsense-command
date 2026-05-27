#![cfg_attr(windows, windows_subsystem = "windows")]

use anyhow::Result;

#[cfg(windows)]
fn main() -> Result<()> {
    windows_tray::run()
}

#[cfg(not(windows))]
fn main() -> Result<()> {
    println!("DualSense Command Center tray is currently implemented for Windows.");
    Ok(())
}

#[cfg(windows)]
mod windows_tray;
