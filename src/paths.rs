//! Where Cerno keeps its own data: the index database, downloaded models and tools.

use std::path::PathBuf;

use anyhow::{Context as _, Result};

/// `%LOCALAPPDATA%\Cerno\data` on Windows, `~/.local/share/cerno` on Linux – or `CERNO_DATA_DIR`
/// when set: another index, settings and crash log, so scripted tests never write into the
/// user's index (a mark there teaches the prediction). The models stay shared ([`models_dir`]).
pub fn data_dir() -> Result<PathBuf> {
    match std::env::var_os("CERNO_DATA_DIR").filter(|dir| !dir.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => default_data_dir(),
    }
}

/// Local rather than roaming on purpose: the aesthetics model alone is 1.2 GB.
fn default_data_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "Cerno").context("no home directory")?;
    Ok(dirs.data_local_dir().to_path_buf())
}

/// Always in the default data folder: a test with its own `CERNO_DATA_DIR` uses the models
/// that are already downloaded.
pub fn models_dir() -> Result<PathBuf> {
    Ok(default_data_dir()?.join("models"))
}

/// The programs Cerno downloads itself (ExifTool on Windows). It follows `CERNO_DATA_DIR`, so
/// a scripted test can start without ExifTool and fetch it into its own folder.
pub fn tools_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("tools"))
}

pub fn database_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("cerno.db"))
}
