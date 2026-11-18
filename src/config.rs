use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Default catalog bundled into the binary; copied out to the user's config
/// directory on first run so it can be edited/extended like a real repo.
const DEFAULT_CATALOG: &str = include_str!("../data/apps.json");

pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub catalog_file: PathBuf,
    pub registry_file: PathBuf,
    pub appimages_dir: PathBuf,
    pub install_dir: PathBuf,
    pub desktop_dir: PathBuf,
    pub icons_dir: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("universal-appimage-installer");
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| home.join(".local/share"))
            .join("universal-appimage-installer");

        let paths = Self {
            catalog_file: config_dir.join("apps.json"),
            registry_file: data_dir.join("installed.json"),
            // Downloads land directly in ~/.applications and installed copies
            // live in ~/.applications/installed. NOTE: deliberately NOT
            // ~/Applications: AppImageLauncher (if installed) watches that
            // folder via appimagelauncherd and auto-integrates anything
            // dropped into it, creating its own competing .desktop entry the
            // moment we copy a file there. The hidden ~/.applications isn't
            // watched, which avoids that race entirely.
            appimages_dir: home.join(".applications"),
            install_dir: home.join(".applications").join("installed"),
            desktop_dir: home.join(".local/share/applications"),
            icons_dir: home.join(".local/share/icons/hicolor"),
            config_dir,
            data_dir,
        };

        paths.ensure_dirs()?;
        Ok(paths)
    }

    fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.appimages_dir,
            &self.install_dir,
            &self.desktop_dir,
            &self.icons_dir,
        ] {
            fs::create_dir_all(dir)
                .with_context(|| format!("creating directory {}", dir.display()))?;
        }

        if !self.catalog_file.exists() {
            fs::write(&self.catalog_file, DEFAULT_CATALOG).with_context(|| {
                format!("writing default catalog to {}", self.catalog_file.display())
            })?;
        }

        if !self.registry_file.exists() {
            fs::write(&self.registry_file, "[]").with_context(|| {
                format!("initializing registry at {}", self.registry_file.display())
            })?;
        }

        Ok(())
    }
}
