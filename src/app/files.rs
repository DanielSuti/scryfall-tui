//! Writing the files `scry` keeps.

use std::io::{self, Write};
use std::path::Path;

/// Writes `contents` to `path`, creating its directory. The contents go to a file next to
/// `path` first, which is then renamed over it, so a crash never leaves a half-written file.
///
/// # Errors
/// Returns the I/O error when the directory or a file cannot be written.
pub fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".tmp");
    let temporary = path.with_file_name(name);
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)
}
