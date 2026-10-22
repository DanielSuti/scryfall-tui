//! Where `scry` keeps its files, following the platform's conventions (XDG on Linux).

use std::path::PathBuf;

use etcetera::BaseStrategy;

/// The directory name used inside the platform's config, state and data directories.
pub const APP_DIR: &str = "scry";

/// Returns `<config dir>/scry/config.toml`, or `None` when the home directory is unknown.
pub fn config_file() -> Option<PathBuf> {
    let base = etcetera::choose_base_strategy().ok()?;
    Some(base.config_dir().join(APP_DIR).join("config.toml"))
}

/// Returns `<state dir>/scry/history.jsonl`. Platforms without a state directory use the data
/// directory instead. Returns `None` when the home directory is unknown.
pub fn history_file() -> Option<PathBuf> {
    let base = etcetera::choose_base_strategy().ok()?;
    let dir = base.state_dir().unwrap_or_else(|| base.data_dir());
    Some(dir.join(APP_DIR).join("history.jsonl"))
}
