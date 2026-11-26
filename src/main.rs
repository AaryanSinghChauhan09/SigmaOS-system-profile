//! CLI for sigmaos-system-profile.

use clap::Parser;
use sigmaos_system_profile::{default_profile_path, to_json, SystemProfile};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sigmaos-system-profile",
    about = "SigmaOS declarative system profile"
)]
struct Cli {
    /// Profile file path (default: ~/.config/sigmaos/system.toml).
    #[arg(short, long)]
    profile: Option<PathBuf>,
    /// Print the resolved system view as JSON.
    #[arg(long)]
    json: bool,
    /// Validate the profile and print warnings.
    #[arg(long)]
    validate: bool,
    /// List enabled components.
    #[arg(long)]
    list: bool,
    /// Write a default desktop profile if none exists.
    #[arg(long)]
    init: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();
    let path = cli.profile.unwrap_or_else(default_profile_path);

    if cli.init {
        if path.exists() {
            eprintln!("profile already exists: {}", path.display());
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        SystemProfile::default_desktop().save(&path)?;
        println!("wrote default profile to {}", path.display());
        return Ok(());
    }

    if cli.list {
        let p = SystemProfile::load(&path)?;
        for c in p.enabled_components() {
            println!("{c}");
        }
        return Ok(());
    }

    let profile = SystemProfile::load(&path)?;

    if cli.validate {
        let warnings = profile.validate()?;
        for w in &warnings {
            eprintln!("warning: {w}");
        }
        println!("profile '{}' valid", profile.name);
        return Ok(());
    }

    let resolved = profile.resolve()?;
    if cli.json {
        println!("{}", to_json(&resolved)?);
    } else {
        println!("profile: {} v{}", resolved.name, resolved.version);
        for c in &resolved.components {
            let state = if c.enabled { "on " } else { "off" };
            let cfg = c
                .config_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".into());
            println!("  [{state}] {:<28} {}", c.name, cfg);
        }
    }
    Ok(())
}
