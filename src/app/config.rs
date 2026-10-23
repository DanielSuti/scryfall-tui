//! The user's configuration file.

use std::io;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

/// Terms added to every search when `search.base` is not set: paper cards only, without
/// joke cards.
pub const DEFAULT_BASE: &str = "game:paper -is:funny";
/// Pages loaded per batch when `search.max_pages` is not set.
pub const DEFAULT_MAX_PAGES: NonZeroU32 = NonZeroU32::new(6).expect("non-zero");
/// History entries kept when `history.max_entries` is not set.
pub const DEFAULT_MAX_ENTRIES: NonZeroUsize = NonZeroUsize::new(100).expect("non-zero");
/// Formats whose legality the detail view shows when `detail.formats` is not set.
pub const DEFAULT_FORMATS: &[&str] = &[
    "standard",
    "pioneer",
    "modern",
    "commander",
    "legacy",
    "historic",
    "alchemy",
    "brawl",
    "timeless",
];

/// How long the cursor rests on a card before its rulings or image are fetched in `auto` mode,
/// when `pause_ms` is not set.
pub const DEFAULT_PAUSE_MS: u64 = 300;

/// The whole configuration. Every key is optional, and an unknown key is an error.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// `[search]`
    pub search: SearchConfig,
    /// `[history]`
    pub history: HistoryConfig,
    /// `[detail]`
    pub detail: DetailConfig,
}

/// The `[search]` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    /// Scryfall terms added to every search, or an empty string for none.
    pub base: String,
    /// Result pages loaded per batch, at least 1.
    pub max_pages: NonZeroU32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            base: DEFAULT_BASE.to_owned(),
            max_pages: DEFAULT_MAX_PAGES,
        }
    }
}

/// The `[history]` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    /// History entries kept, at least 1. Older entries are dropped.
    pub max_entries: NonZeroUsize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }
}

/// The `[detail]` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DetailConfig {
    /// Format names whose legality the detail view shows, in this order. An empty list hides
    /// the legalities.
    pub formats: Vec<String>,
    /// `[detail.rulings]`: when the card's rulings are fetched.
    pub rulings: LoadConfig,
    /// `[detail.images]`: when the card's image is fetched. Images are shown only in terminals
    /// with a graphics protocol.
    pub images: LoadConfig,
}

impl Default for DetailConfig {
    fn default() -> Self {
        Self {
            formats: DEFAULT_FORMATS.iter().map(|f| (*f).to_owned()).collect(),
            rulings: LoadConfig::default(),
            images: LoadConfig::default(),
        }
    }
}

/// When one view of the detail loads data beyond the card itself, such as its rulings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadMode {
    /// Once the cursor has rested on the card for `pause_ms`.
    #[default]
    Auto,
    /// Never. The view doesn't show the data, even when it was loaded in the other view.
    Never,
}

/// A table saying when the detail loads one kind of data, in the split and in the full detail.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoadConfig {
    /// When the data is loaded in the split.
    pub split: LoadMode,
    /// When the data is loaded in the full detail.
    pub full_detail: LoadMode,
    /// In `auto` mode, how long the cursor rests on a card before the data is fetched, in
    /// milliseconds. 0 fetches it at once.
    pub pause_ms: u64,
}

impl LoadConfig {
    /// Returns the mode of the full detail when `full_detail` is true, else of the split.
    pub fn mode_in(&self, full_detail: bool) -> LoadMode {
        if full_detail {
            self.full_detail
        } else {
            self.split
        }
    }

    /// Returns true when neither view ever loads the data.
    pub fn is_never(&self) -> bool {
        self.split == LoadMode::Never && self.full_detail == LoadMode::Never
    }

    /// Returns `pause_ms` as a duration.
    pub fn pause(&self) -> Duration {
        Duration::from_millis(self.pause_ms)
    }
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            split: LoadMode::Auto,
            full_detail: LoadMode::Auto,
            pause_ms: DEFAULT_PAUSE_MS,
        }
    }
}

/// Why the configuration could not be loaded.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The file exists but cannot be read.
    #[error("cannot read {}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The I/O failure.
        source: io::Error,
    },
    /// The file is not valid TOML or does not match the documented keys.
    #[error("{}: {source}", path.display())]
    Parse {
        /// The file.
        path: PathBuf,
        /// The parse failure, which names the key and its position.
        source: Box<toml::de::Error>,
    },
    /// An entry of `detail.formats` is empty.
    #[error("{}: `detail.formats` entry {index} is empty", path.display())]
    EmptyFormat {
        /// The file.
        path: PathBuf,
        /// The position of the entry, starting at 1.
        index: usize,
    },
}

impl Config {
    /// Loads the configuration from `path`. A missing file gives the defaults.
    ///
    /// # Errors
    /// Returns [`ConfigError`] when the file cannot be read, is not valid TOML, contains an
    /// unknown key or an invalid value, or lists an empty format name.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Read {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Parses configuration `text`. `path` is used only in error messages.
    ///
    /// # Errors
    /// Returns [`ConfigError::Parse`] or [`ConfigError::EmptyFormat`] as described for
    /// [`Config::load`].
    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(text).map_err(|source| ConfigError::Parse {
            path: path.to_owned(),
            source: Box::new(source),
        })?;
        if let Some(index) = config
            .detail
            .formats
            .iter()
            .position(|f| f.trim().is_empty())
        {
            return Err(ConfigError::EmptyFormat {
                path: path.to_owned(),
                index: index + 1,
            });
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new("config.toml"))
    }

    #[test]
    fn missing_file_and_empty_file_give_the_defaults() {
        let missing = Config::load(Path::new("/nonexistent/scry/config.toml")).unwrap();
        assert_eq!(missing, Config::default());
        assert_eq!(parse("").unwrap(), Config::default());
        assert_eq!(Config::default().search.max_pages.get(), 6);
        assert_eq!(Config::default().search.base, "game:paper -is:funny");
        assert_eq!(Config::default().history.max_entries.get(), 100);
        assert_eq!(Config::default().detail.formats.len(), 9);
        for load in [
            Config::default().detail.rulings,
            Config::default().detail.images,
        ] {
            assert_eq!(
                (load.split, load.full_detail),
                (LoadMode::Auto, LoadMode::Auto)
            );
            assert_eq!(load.pause(), Duration::from_millis(300));
        }
    }

    #[test]
    fn set_values_are_read() {
        let config = parse(
            "[search]\nmax_pages = 2\n[history]\nmax_entries = 5\n[detail]\nformats = [\"modern\"]\n",
        )
        .unwrap();
        assert_eq!(config.search.max_pages.get(), 2);
        assert_eq!(
            config.search.base, "game:paper -is:funny",
            "unset keys keep defaults"
        );
        let no_base = parse("[search]\nbase = \"\"").unwrap();
        assert_eq!(no_base.search.base, "");
        let custom = parse("[search]\nbase = \"f:commander\"").unwrap();
        assert_eq!(custom.search.base, "f:commander");
        assert_eq!(config.history.max_entries.get(), 5);
        assert_eq!(config.detail.formats, ["modern"]);
        let rulings = parse("[detail.rulings]\nsplit = \"never\"\npause_ms = 0").unwrap();
        let load = &rulings.detail.rulings;
        assert_eq!(
            (load.split, load.full_detail),
            (LoadMode::Never, LoadMode::Auto)
        );
        assert_eq!(load.mode_in(false), LoadMode::Never);
        assert_eq!(load.mode_in(true), LoadMode::Auto);
        assert!(!load.is_never());
        assert_eq!(load.pause(), Duration::ZERO);
        assert_eq!(
            rulings.detail.formats.len(),
            9,
            "the formats keep their default"
        );
        let images =
            parse("[detail.images]\nsplit = \"never\"\nfull_detail = \"never\"\npause_ms = 50")
                .unwrap();
        assert!(images.detail.images.is_never());
        assert_eq!(images.detail.images.pause(), Duration::from_millis(50));
        assert_eq!(images.detail.rulings, LoadConfig::default());
    }

    #[test]
    fn invalid_values_are_rejected_with_the_key() {
        for (text, key) in [
            ("[search]\nmax_pages = 0", "max_pages"),
            ("[search]\nmax_pages = -1", "max_pages"),
            ("[search]\nmax_pages = 1.5", "max_pages"),
            ("[search]\nmax_pages = \"\"", "max_pages"),
            ("[history]\nmax_entries = 0", "max_entries"),
            ("[search]\nbase = 3", "base"),
            ("[search]\nmax_page = 2", "max_page"),
            ("colour = true", "colour"),
            ("[detail.rulings]\nsplit = \"sometimes\"", "never"),
            ("[detail.rulings]\nfull_detail = \"split\"", "auto"),
            ("[detail.rulings]\nmode = \"auto\"", "mode"),
            ("[detail.rulings]\npause_ms = -1", "pause_ms"),
            ("[detail.rulings]\ndelay = 1", "delay"),
            ("[detail.images]\nsplit = \"full_detail\"", "never"),
            ("[detail.images]\npause_ms = -1", "pause_ms"),
            ("[detail.images]\nsize = \"large\"", "size"),
        ] {
            let err = parse(text).unwrap_err().to_string();
            assert!(err.contains(key), "{text:?} gave {err}");
        }
    }

    #[test]
    fn empty_format_name_is_rejected() {
        let err = parse("[detail]\nformats = [\"modern\", \" \"]").unwrap_err();
        assert!(
            matches!(err, ConfigError::EmptyFormat { index: 2, .. }),
            "{err}"
        );
    }
}
