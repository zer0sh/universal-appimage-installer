use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    pub version: String,
    pub appimage_path: PathBuf,
    pub desktop_path: PathBuf,
    pub icon_path: Option<PathBuf>,
    pub installed_at: String,
}

pub fn load(registry_file: &Path) -> Result<Vec<InstalledApp>> {
    let raw = fs::read_to_string(registry_file)
        .with_context(|| format!("reading registry {}", registry_file.display()))?;
    let apps: Vec<InstalledApp> = serde_json::from_str(&raw)
        .with_context(|| format!("parsing registry {}", registry_file.display()))?;
    Ok(apps)
}

pub fn save(registry_file: &Path, apps: &[InstalledApp]) -> Result<()> {
    let raw = serde_json::to_string_pretty(apps)?;
    fs::write(registry_file, raw)
        .with_context(|| format!("writing registry {}", registry_file.display()))?;
    Ok(())
}

pub fn add(registry_file: &Path, app: InstalledApp) -> Result<()> {
    let mut apps = load(registry_file)?;
    apps.retain(|a| a.appimage_path != app.appimage_path);
    apps.push(app);
    save(registry_file, &apps)
}

pub fn remove(registry_file: &Path, index: usize) -> Result<InstalledApp> {
    let mut apps = load(registry_file)?;
    if index >= apps.len() {
        anyhow::bail!("invalid registry index {index}");
    }
    let removed = apps.remove(index);
    save(registry_file, &apps)?;
    Ok(removed)
}
