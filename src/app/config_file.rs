//! Saving settings into the config file, keeping everything else in it as the user wrote it:
//! other keys, comments, order and spacing.

use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::{Array, DocumentMut, Item, Table, TableLike, Value};

use crate::app::config::{Config, DEFAULT_CONFIG};
use crate::app::files::write_atomically;
use crate::app::settings::Setting;

/// Why settings could not be saved. The file is left as it was.
#[derive(Debug, Error)]
pub enum SaveError {
    /// The file exists but cannot be read.
    #[error("cannot read {}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The I/O failure.
        source: io::Error,
    },
    /// The file is not valid TOML.
    #[error("{}: {source}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// The parse failure.
        source: Box<toml_edit::TomlError>,
    },
    /// The file holds something where a setting's table should be, or would not load back
    /// as the settings to save.
    #[error("{}: {message}", path.display())]
    Conflict {
        /// The file.
        path: PathBuf,
        /// What is in the way.
        message: String,
    },
    /// The file cannot be written.
    #[error("cannot write {}: {source}", path.display())]
    Write {
        /// The file.
        path: PathBuf,
        /// The I/O failure.
        source: io::Error,
    },
}

/// Writes the values `config` holds for `settings` into the config file at `path`. A missing
/// file starts as [`DEFAULT_CONFIG`], so it documents every setting. A missing table is added,
/// and a changed value keeps the comments and spacing around it. The new text must load back
/// with these values before it replaces the file, atomically.
///
/// # Errors
/// Returns a [`SaveError`] when the file cannot be read, parsed or written, or when its
/// contents leave no room for a setting; nothing is written then.
pub fn save(path: &Path, config: &Config, settings: &[Setting]) -> Result<(), SaveError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => DEFAULT_CONFIG.to_owned(),
        Err(source) => {
            return Err(SaveError::Read {
                path: path.to_owned(),
                source,
            });
        }
    };
    let mut document: DocumentMut = text.parse().map_err(|source| SaveError::Parse {
        path: path.to_owned(),
        source: Box::new(source),
    })?;
    let conflict = |message: String| SaveError::Conflict {
        path: path.to_owned(),
        message,
    };
    for setting in settings {
        write_setting(&mut document, *setting, config).map_err(conflict)?;
    }
    let text = document.to_string();
    let loaded = Config::parse(&text, path).map_err(|err| conflict(err.to_string()))?;
    if let Some(setting) = settings
        .iter()
        .find(|s| s.value(&loaded) != s.value(config))
    {
        let (table, key) = setting.path();
        return Err(conflict(format!(
            "`{table}.{key}` would not load back as saved"
        )));
    }
    write_atomically(path, text.as_bytes()).map_err(|source| SaveError::Write {
        path: path.to_owned(),
        source,
    })
}

/// Sets `setting` in `document` to its value in `config`, adding its tables when missing.
fn write_setting(
    document: &mut DocumentMut,
    setting: Setting,
    config: &Config,
) -> Result<(), String> {
    let (table_path, key) = setting.path();
    let parts: Vec<&str> = table_path.split('.').collect();
    let mut table: &mut dyn TableLike = document.as_table_mut();
    for (index, part) in parts.iter().enumerate() {
        if table.get(part).is_none() {
            let mut new = Table::new();
            // A parent that only holds the new table gets no header of its own.
            new.set_implicit(index + 1 < parts.len());
            table.insert(part, Item::Table(new));
        }
        let name = parts[..=index].join(".");
        table = table
            .get_mut(part)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| format!("`{name}` is not a table"))?;
    }
    let mut value = toml_value(setting, config);
    match table.get_mut(key) {
        Some(item) => {
            if let Some(old) = item.as_value() {
                *value.decor_mut() = old.decor().clone();
            }
            *item = Item::Value(value);
        }
        None => {
            table.insert(key, Item::Value(value));
        }
    }
    Ok(())
}

/// Returns the value `config` holds for `setting`, as TOML.
fn toml_value(setting: Setting, config: &Config) -> Value {
    let number = |n: u64| Value::from(i64::try_from(n).unwrap_or(i64::MAX));
    match setting {
        Setting::MaxPages => number(u64::from(config.search.max_pages.get())),
        Setting::MaxEntries => number(config.history.max_entries.get() as u64),
        Setting::RulingsPause => number(config.detail.rulings.pause_ms),
        Setting::ImagesPause => number(config.detail.images.pause_ms),
        Setting::Formats => Value::Array(Array::from_iter(config.detail.formats.iter())),
        Setting::Base
        | Setting::RulingsSplit
        | Setting::RulingsFullDetail
        | Setting::ImagesSplit
        | Setting::ImagesFullDetail => Value::from(setting.value(config)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::config::LoadMode;

    fn temp_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("scry-config-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("scry").join("config.toml")
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn a_missing_file_starts_as_the_commented_defaults() {
        let path = temp_file("missing");
        let mut config = Config::default();
        config.search.max_pages = 2.try_into().unwrap();
        config.detail.rulings.split = LoadMode::Auto;
        save(&path, &config, &[Setting::MaxPages, Setting::RulingsSplit]).unwrap();
        let expected = DEFAULT_CONFIG
            .replace("max_pages = 4", "max_pages = 2")
            .replacen("split = \"trigger\"", "split = \"auto\"", 1);
        assert_eq!(read(&path), expected);
        assert_eq!(Config::load(&path).unwrap(), config);
    }

    #[test]
    fn an_existing_file_keeps_its_keys_comments_and_order() {
        let path = temp_file("existing");
        let original = "# my settings\n[search]\n# my terms\nbase = \"f:commander\"\nmax_pages = 6  # plenty\n\n[history]\nmax_entries = 10\n";
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, original).unwrap();
        let mut config = Config::load(&path).unwrap();
        config.search.max_pages = 2.try_into().unwrap();
        config.detail.images.split = LoadMode::Never;
        config.detail.formats = vec!["modern".into(), "legacy".into()];
        let changed = [Setting::MaxPages, Setting::ImagesSplit, Setting::Formats];
        save(&path, &config, &changed).unwrap();
        let text = read(&path);
        assert!(
            text.starts_with("# my settings\n[search]\n# my terms\nbase = \"f:commander\"\nmax_pages = 2  # plenty\n\n[history]\nmax_entries = 10\n"),
            "{text}"
        );
        assert!(
            text.contains("[detail]\nformats = [\"modern\", \"legacy\"]\n"),
            "{text}"
        );
        assert!(
            text.contains("[detail.images]\nsplit = \"never\"\n"),
            "{text}"
        );
        assert!(
            !text.contains("[detail.rulings]"),
            "no table for unsaved settings: {text}"
        );
        assert_eq!(Config::load(&path).unwrap(), config);
    }

    #[test]
    fn inline_tables_are_written_in_place() {
        let path = temp_file("inline");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[detail]\nrulings = { split = \"never\" }\n").unwrap();
        let mut config = Config::load(&path).unwrap();
        config.detail.rulings.split = LoadMode::Auto;
        save(&path, &config, &[Setting::RulingsSplit]).unwrap();
        assert_eq!(read(&path), "[detail]\nrulings = { split = \"auto\" }\n");
    }

    #[test]
    fn a_broken_file_is_reported_and_left_alone() {
        let path = temp_file("broken");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for original in ["[search\nmax_pages = 2\n", "search = 3\n"] {
            std::fs::write(&path, original).unwrap();
            let err = save(&path, &Config::default(), &[Setting::MaxPages]).unwrap_err();
            assert!(
                matches!(err, SaveError::Parse { .. } | SaveError::Conflict { .. }),
                "{err}"
            );
            assert_eq!(read(&path), original);
        }
    }
}
