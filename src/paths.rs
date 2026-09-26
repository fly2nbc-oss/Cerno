//! Where Cerno keeps its own data: the index database and downloaded models.

use std::path::PathBuf;

use anyhow::{Context as _, Result};

/// `%LOCALAPPDATA%\Cerno\data` on Windows, `~/.local/share/cerno` on Linux. Local rather than
/// roaming on purpose: the aesthetics model alone is 1.2 GB.
pub fn data_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "Cerno").context("no home directory")?;
    Ok(dirs.data_local_dir().to_path_buf())
}

pub fn models_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("models"))
}

pub fn database_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("cerno.db"))
}
