use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// One entry of the app "repo". Keyed by a short id (e.g. "clickup") in the
/// JSON file so the catalog is a plain key/value map, easy to hand-edit or
/// extend with more apps later.
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    pub download_url: String,
}

#[derive(Debug, Clone)]
pub struct CatalogApp {
    pub id: String,
    pub entry: CatalogEntry,
}

pub fn load(catalog_file: &Path) -> Result<Vec<CatalogApp>> {
    let raw = fs::read_to_string(catalog_file)
        .with_context(|| format!("reading catalog file {}", catalog_file.display()))?;
    let map: BTreeMap<String, CatalogEntry> = serde_json::from_str(&raw)
        .with_context(|| format!("parsing catalog file {}", catalog_file.display()))?;

    Ok(map
        .into_iter()
        .map(|(id, entry)| CatalogApp { id, entry })
        .collect())
}
