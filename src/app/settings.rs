//! The settings the config window edits: their labels, where they live in the config file,
//! their values as text, and the presets Tab cycles through.

use std::num::{NonZeroU32, NonZeroUsize};

use thiserror::Error;

use crate::app::config::{Config, DEFAULT_FORMATS, LoadMode};

/// Joins the formats of `detail.formats` when they are shown or edited as text.
pub const FORMAT_SEPARATOR: &str = ", ";

/// A setting the config window edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setting {
    /// `search.base`
    Base,
    /// `search.max_pages`
    MaxPages,
    /// `history.max_entries`
    MaxEntries,
    /// `detail.formats`
    Formats,
    /// `detail.rulings.split`
    RulingsSplit,
    /// `detail.rulings.full_detail`
    RulingsFullDetail,
    /// `detail.rulings.pause_ms`
    RulingsPause,
    /// `detail.images.split`
    ImagesSplit,
    /// `detail.images.full_detail`
    ImagesFullDetail,
    /// `detail.images.pause_ms`
    ImagesPause,
}

/// A value that the setting doesn't accept.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct SettingError(String);

impl Setting {
    /// Every setting, in the order the config window lists them.
    pub const ALL: [Setting; 10] = [
        Setting::Base,
        Setting::MaxPages,
        Setting::MaxEntries,
        Setting::Formats,
        Setting::RulingsSplit,
        Setting::RulingsFullDetail,
        Setting::RulingsPause,
        Setting::ImagesSplit,
        Setting::ImagesFullDetail,
        Setting::ImagesPause,
    ];

    /// Returns the setting's name in the config window, such as `Pages per batch`.
    pub fn label(self) -> &'static str {
        match self {
            Setting::Base => "Terms added to every search",
            Setting::MaxPages => "Pages per batch",
            Setting::MaxEntries => "History entries kept",
            Setting::Formats => "Legality formats",
            Setting::RulingsSplit => "Rulings in the split",
            Setting::RulingsFullDetail => "Rulings in the full detail",
            Setting::RulingsPause => "Rulings pause (ms)",
            Setting::ImagesSplit => "Images in the split",
            Setting::ImagesFullDetail => "Images in the full detail",
            Setting::ImagesPause => "Images pause (ms)",
        }
    }

    /// Returns the setting's table and key in the config file, such as
    /// `("detail.rulings", "split")`.
    pub fn path(self) -> (&'static str, &'static str) {
        match self {
            Setting::Base => ("search", "base"),
            Setting::MaxPages => ("search", "max_pages"),
            Setting::MaxEntries => ("history", "max_entries"),
            Setting::Formats => ("detail", "formats"),
            Setting::RulingsSplit => ("detail.rulings", "split"),
            Setting::RulingsFullDetail => ("detail.rulings", "full_detail"),
            Setting::RulingsPause => ("detail.rulings", "pause_ms"),
            Setting::ImagesSplit => ("detail.images", "split"),
            Setting::ImagesFullDetail => ("detail.images", "full_detail"),
            Setting::ImagesPause => ("detail.images", "pause_ms"),
        }
    }

    /// Returns the values Tab cycles through, as text.
    pub fn presets(self) -> Vec<String> {
        match self {
            Setting::Base => vec![
                "game:paper -is:funny".into(),
                "game:paper".into(),
                String::new(),
            ],
            Setting::MaxPages => ["1", "2", "4", "6", "10"].map(String::from).to_vec(),
            Setting::MaxEntries => ["20", "40", "100", "500"].map(String::from).to_vec(),
            Setting::Formats => vec![
                DEFAULT_FORMATS.join(FORMAT_SEPARATOR),
                "commander".into(),
                String::new(),
            ],
            Setting::RulingsPause | Setting::ImagesPause => {
                ["0", "50", "150", "300", "500"].map(String::from).to_vec()
            }
            Setting::RulingsSplit
            | Setting::RulingsFullDetail
            | Setting::ImagesSplit
            | Setting::ImagesFullDetail => LoadMode::ALL.iter().map(|m| m.name().into()).collect(),
        }
    }

    /// Returns the setting's value in `config` as text: a number, a mode name, the formats
    /// joined with [`FORMAT_SEPARATOR`], or the terms as they are.
    pub fn value(self, config: &Config) -> String {
        match self {
            Setting::Base => config.search.base.clone(),
            Setting::MaxPages => config.search.max_pages.to_string(),
            Setting::MaxEntries => config.history.max_entries.to_string(),
            Setting::Formats => config.detail.formats.join(FORMAT_SEPARATOR),
            Setting::RulingsSplit => config.detail.rulings.split.name().to_owned(),
            Setting::RulingsFullDetail => config.detail.rulings.full_detail.name().to_owned(),
            Setting::RulingsPause => config.detail.rulings.pause_ms.to_string(),
            Setting::ImagesSplit => config.detail.images.split.name().to_owned(),
            Setting::ImagesFullDetail => config.detail.images.full_detail.name().to_owned(),
            Setting::ImagesPause => config.detail.images.pause_ms.to_string(),
        }
    }

    /// Sets the setting in `config` from `text`, as [`Setting::value`] writes it. Surrounding
    /// spaces are ignored; empty text clears the terms and hides the legalities.
    ///
    /// # Errors
    /// Returns a [`SettingError`] naming the setting when `text` is not a value it accepts.
    /// `config` is unchanged then.
    pub fn set(self, config: &mut Config, text: &str) -> Result<(), SettingError> {
        let text = text.trim();
        match self {
            Setting::Base => config.search.base = text.to_owned(),
            Setting::MaxPages => config.search.max_pages = self.positive::<NonZeroU32>(text)?,
            Setting::MaxEntries => {
                config.history.max_entries = self.positive::<NonZeroUsize>(text)?;
            }
            Setting::Formats => config.detail.formats = parse_formats(text)?,
            Setting::RulingsSplit => config.detail.rulings.split = self.mode(text)?,
            Setting::RulingsFullDetail => config.detail.rulings.full_detail = self.mode(text)?,
            Setting::RulingsPause => config.detail.rulings.pause_ms = self.millis(text)?,
            Setting::ImagesSplit => config.detail.images.split = self.mode(text)?,
            Setting::ImagesFullDetail => config.detail.images.full_detail = self.mode(text)?,
            Setting::ImagesPause => config.detail.images.pause_ms = self.millis(text)?,
        }
        Ok(())
    }

    /// Returns the value `step` places after `current` in the presets, wrapping around;
    /// a negative `step` goes back. `typed`, a value the user typed that is not a preset,
    /// takes its place in the cycle after the presets, and so does `current` when it is
    /// neither.
    pub fn cycle(self, current: &str, typed: Option<&str>, step: isize) -> String {
        let mut values = self.presets();
        for extra in [typed, Some(current)].into_iter().flatten() {
            if !values.iter().any(|v| v == extra) {
                values.push(extra.to_owned());
            }
        }
        let index = values.iter().position(|v| v == current).unwrap_or(0);
        let len = isize::try_from(values.len()).unwrap_or(isize::MAX);
        let next = (isize::try_from(index).unwrap_or(0) + step).rem_euclid(len);
        values[usize::try_from(next).unwrap_or(0)].clone()
    }

    fn positive<T: std::str::FromStr>(self, text: &str) -> Result<T, SettingError> {
        text.parse().map_err(|_| {
            SettingError(format!(
                "{} must be a whole number of 1 or more",
                self.label()
            ))
        })
    }

    fn millis(self, text: &str) -> Result<u64, SettingError> {
        text.parse().map_err(|_| {
            SettingError(format!(
                "{} must be a whole number of milliseconds, 0 or more",
                self.label().trim_end_matches(" (ms)")
            ))
        })
    }

    fn mode(self, text: &str) -> Result<LoadMode, SettingError> {
        LoadMode::from_name(&text.to_lowercase())
            .ok_or_else(|| SettingError(format!("{} must be auto, trigger or never", self.label())))
    }
}

/// Parses formats separated by commas. Empty text gives none.
fn parse_formats(text: &str) -> Result<Vec<String>, SettingError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let formats: Vec<String> = text.split(',').map(|f| f.trim().to_owned()).collect();
    if formats.iter().any(String::is_empty) {
        return Err(SettingError(
            "Legality formats can't have an empty name between commas".to_owned(),
        ));
    }
    Ok(formats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_takes_back_its_own_value() {
        let mut config = Config::default();
        for setting in Setting::ALL {
            let value = setting.value(&config);
            setting.set(&mut config, &value).unwrap();
            assert_eq!(config, Config::default(), "{setting:?}");
        }
    }

    #[test]
    fn values_are_set_and_shown_as_text() {
        let mut config = Config::default();
        Setting::MaxPages.set(&mut config, " 2 ").unwrap();
        assert_eq!(config.search.max_pages.get(), 2);
        Setting::RulingsSplit.set(&mut config, "Auto").unwrap();
        assert_eq!(config.detail.rulings.split, LoadMode::Auto);
        Setting::ImagesPause.set(&mut config, "0").unwrap();
        assert_eq!(Setting::ImagesPause.value(&config), "0");
        Setting::Formats
            .set(&mut config, "modern, legacy ,vintage")
            .unwrap();
        assert_eq!(config.detail.formats, ["modern", "legacy", "vintage"]);
        assert_eq!(Setting::Formats.value(&config), "modern, legacy, vintage");
        Setting::Formats.set(&mut config, "  ").unwrap();
        assert!(config.detail.formats.is_empty(), "empty text hides them");
        Setting::Base.set(&mut config, "").unwrap();
        assert_eq!(config.search.base, "");
    }

    #[test]
    fn invalid_values_are_refused_with_the_settings_name() {
        let mut config = Config::default();
        for (setting, text, message) in [
            (
                Setting::MaxPages,
                "0",
                "Pages per batch must be a whole number of 1 or more",
            ),
            (
                Setting::MaxEntries,
                "lots",
                "History entries kept must be a whole number",
            ),
            (
                Setting::RulingsPause,
                "-5",
                "Rulings pause must be a whole number of milliseconds",
            ),
            (
                Setting::ImagesSplit,
                "sometimes",
                "Images in the split must be auto, trigger or never",
            ),
            (
                Setting::Formats,
                "modern,,legacy",
                "can't have an empty name",
            ),
        ] {
            let err = setting.set(&mut config, text).unwrap_err().to_string();
            assert!(err.contains(message), "{setting:?} {text:?}: {err}");
        }
        assert_eq!(config, Config::default(), "a refused value changes nothing");
    }

    #[test]
    fn tab_cycles_the_presets_both_ways() {
        let modes = Setting::RulingsSplit;
        assert_eq!(modes.cycle("trigger", None, 1), "never");
        assert_eq!(modes.cycle("never", None, 1), "auto");
        assert_eq!(modes.cycle("auto", None, -1), "never");
        let pages = Setting::MaxPages;
        assert_eq!(pages.cycle("4", None, 1), "6");
        assert_eq!(pages.cycle("10", None, 1), "1");
    }

    #[test]
    fn a_typed_value_takes_a_place_in_the_cycle() {
        let pages = Setting::MaxPages;
        assert_eq!(
            pages.cycle("3", None, 1),
            "1",
            "after the presets comes the start"
        );
        assert_eq!(pages.cycle("10", Some("3"), 1), "3");
        assert_eq!(pages.cycle("3", Some("3"), -1), "10");
        assert_eq!(pages.cycle("1", Some("3"), -1), "3");
    }

    #[test]
    fn the_paths_name_keys_of_the_default_config() {
        let defaults: toml::Table = toml::from_str(crate::app::config::DEFAULT_CONFIG).unwrap();
        for setting in Setting::ALL {
            let (table, key) = setting.path();
            let mut value = &toml::Value::Table(defaults.clone());
            for part in table.split('.').chain([key]) {
                value = &value[part];
            }
            assert!(!value.is_table(), "{setting:?}");
        }
    }
}
