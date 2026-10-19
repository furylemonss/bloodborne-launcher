use anyhow::{Context, Result};
use std::process::Command;

pub fn shutdown_pc() -> Result<()> {
    Command::new("shutdown")
        .args(["/s", "/t", "60", "/c", "Bloodborne Launcher"])
        .status()
        .context("shutdown baslatilamadi")?;
    Ok(())
}

pub fn restart_pc() -> Result<()> {
    Command::new("shutdown")
        .args(["/r", "/t", "60", "/c", "Bloodborne Launcher"])
        .status()
        .context("restart baslatilamadi")?;
    Ok(())
}
