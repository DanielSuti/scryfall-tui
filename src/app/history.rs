//! The search history, newest first, saved between sessions.

use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One past search. In the file each entry is a JSON object on its own line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// The query as typed.
    pub query: String,
    /// Matching cards on the last run, or `null` when unknown, for example after a failure.
    pub total_cards: Option<u32>,
}

/// Why the history could not be read or written.
#[derive(Debug, Error)]
pub enum HistoryError {
    /// The file exists but cannot be read.
    #[error("cannot read {}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// The I/O failure.
        source: io::Error,
    },
    /// A line is not a history entry.
    #[error("{}:{line}: not a history entry: {source}", path.display())]
    Corrupt {
        /// The file.
        path: PathBuf,
        /// The line number, starting at 1.
        line: usize,
        /// The JSON failure.
        source: serde_json::Error,
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

/// Past searches, newest first, without duplicate queries and at most `max_entries` long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    entries: Vec<HistoryEntry>,
    max_entries: NonZeroUsize,
}

impl History {
    /// Creates an empty history keeping at most `max_entries`.
    pub fn new(max_entries: NonZeroUsize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Returns the entries, newest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Puts `query` at the top. An existing entry with the same text moves there and keeps
    /// its count; a new one starts with an unknown count. The oldest entries beyond
    /// `max_entries` are dropped.
    pub fn record(&mut self, query: &str) {
        let entry = match self.entries.iter().position(|e| e.query == query) {
            Some(index) => self.entries.remove(index),
            None => HistoryEntry {
                query: query.to_owned(),
                total_cards: None,
            },
        };
        self.entries.insert(0, entry);
        self.entries.truncate(self.max_entries.get());
    }

    /// Sets the count of `query`'s entry. Does nothing when the query is not in the history.
    pub fn set_total(&mut self, query: &str, total_cards: Option<u32>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.query == query) {
            entry.total_cards = total_cards;
        }
    }

    /// Removes and returns the entry at `index`, or returns `None` when there is none.
    pub fn delete(&mut self, index: usize) -> Option<HistoryEntry> {
        (index < self.entries.len()).then(|| self.entries.remove(index))
    }

    /// Reads the history from `path`, keeping at most `max_entries`. A missing file gives an
    /// empty history. Empty lines are skipped.
    ///
    /// # Errors
    /// Returns [`HistoryError::Read`] when the file cannot be read, and
    /// [`HistoryError::Corrupt`] for a line that is not an entry.
    pub fn load(path: &Path, max_entries: NonZeroUsize) -> Result<Self, HistoryError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::new(max_entries));
            }
            Err(source) => {
                return Err(HistoryError::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let mut history = Self::new(max_entries);
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry = serde_json::from_str(line).map_err(|source| HistoryError::Corrupt {
                path: path.to_owned(),
                line: index + 1,
                source,
            })?;
            history.entries.push(entry);
        }
        history.entries.truncate(max_entries.get());
        Ok(history)
    }

    /// Writes the history to `path`, creating its directory. The file is written next to its
    /// final name and then renamed, so a crash never leaves a half-written history.
    ///
    /// # Errors
    /// Returns [`HistoryError::Write`] when the directory or file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), HistoryError> {
        let write_error = |source| HistoryError::Write {
            path: path.to_owned(),
            source,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(write_error)?;
        }
        let temporary = path.with_extension("jsonl.tmp");
        let mut file = std::fs::File::create(&temporary).map_err(write_error)?;
        for entry in &self.entries {
            let line = serde_json::to_string(entry).map_err(|err| write_error(err.into()))?;
            writeln!(file, "{line}").map_err(write_error)?;
        }
        file.sync_all().map_err(write_error)?;
        std::fs::rename(&temporary, path).map_err(write_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).unwrap()
    }

    fn queries(history: &History) -> Vec<&str> {
        history.entries().iter().map(|e| e.query.as_str()).collect()
    }

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("scry-history-test-{}-{name}", std::process::id()))
            .join("history.jsonl")
    }

    #[test]
    fn rerunning_moves_the_entry_to_the_top_without_a_duplicate() {
        let mut history = History::new(max(10));
        history.record("t:dragon");
        history.set_total("t:dragon", Some(444));
        history.record("o:draw");
        history.record("t:dragon");
        assert_eq!(queries(&history), ["t:dragon", "o:draw"]);
        assert_eq!(
            history.entries()[0].total_cards,
            Some(444),
            "the count is kept"
        );
    }

    #[test]
    fn the_oldest_entry_drops_at_the_cap() {
        let mut history = History::new(max(2));
        for q in ["a", "b", "c"] {
            history.record(q);
        }
        assert_eq!(queries(&history), ["c", "b"]);
    }

    #[test]
    fn delete_removes_the_entry() {
        let mut history = History::new(max(5));
        for q in ["a", "b", "c"] {
            history.record(q);
        }
        assert_eq!(history.delete(1).unwrap().query, "b");
        assert_eq!(queries(&history), ["c", "a"]);
        assert_eq!(history.delete(7), None);
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = temp_file("round-trip");
        let mut history = History::new(max(5));
        history.record("t:dragon");
        history.set_total("t:dragon", Some(444));
        history.record("o:\"draw, then discard\"");
        history.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "{\"query\":\"o:\\\"draw, then discard\\\"\",\"total_cards\":null}\n\
             {\"query\":\"t:dragon\",\"total_cards\":444}\n"
        );
        assert_eq!(History::load(&path, max(5)).unwrap(), history);
        assert_eq!(
            queries(&History::load(&path, max(1)).unwrap()),
            ["o:\"draw, then discard\""]
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_file_is_empty_and_a_corrupt_line_is_reported() {
        let missing = History::load(Path::new("/nonexistent/scry/history.jsonl"), max(5)).unwrap();
        assert!(missing.entries().is_empty());
        let path = temp_file("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{\"query\":\"a\",\"total_cards\":1}\n\nnot json\n").unwrap();
        let err = History::load(&path, max(5)).unwrap_err();
        assert!(
            matches!(err, HistoryError::Corrupt { line: 3, .. }),
            "{err}"
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
